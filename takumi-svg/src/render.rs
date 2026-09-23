//! End-to-end SVG rendering: run takumi-core layout, walk the tree, emit SVG.

use std::{collections::HashMap, io, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  error::Result,
  geometry::{ComputedLayout as Layout, NodeId, Point, Size},
  layout::{
    border::{BorderProperties, BorderSide, PaintedSide},
    decoration::{ClipBox, OutlineGeometry},
    inline::{InlineBoxItem, VisualInlineBox},
    inline_box::{InlineBoxPaint, resolve_inline_box},
    node::{ImageData, Node, NodeKind},
    tree::{LayoutTree, RenderNode},
  },
  painter::{BoxPainter, FillShape, PaintDevice, StrokeStyle, paint_border},
  resources::image::ImageSource,
  scene::{SceneRequest, build_scene},
  style::{
    Affine, BackgroundClip, BackgroundImage, BackgroundOrigin, BasicShape, BlendMode, BorderStyle,
    Color, ComputedStyle, FillRule, FontFamily, Isolation, Lang, Overflow, ShapeRadius, Sides,
    SizingContext, SpacePair, StyleSheet, ToCss,
  },
  viewport::Viewport,
};
use typed_builder::TypedBuilder;

use crate::{
  APPROX_CHARS_PER_NUMBER, Frame, GroupToken, Num, Rgba, SvgDocument,
  box_model::{PathData, element_transform, path_data, rect_path_data},
  gradient::LayerEmitter,
  image::emit_image,
  scene_emit::SceneEmitter,
  text::{emit_inline_content, emit_text},
};

/// Inputs for [`render`], built with [`SvgOptions::builder`].
#[derive(TypedBuilder)]
pub struct SvgOptions<'g> {
  /// The viewport to render in.
  pub(crate) viewport: Viewport,
  /// The font context.
  pub(crate) fonts: &'g Fonts,
  /// The root node to render.
  pub(crate) node: Node,
  /// Resources fetched externally, keyed by URL.
  #[builder(default)]
  pub(crate) images: HashMap<Arc<str>, ImageSource>,
  /// CSS stylesheets to apply before layout.
  #[builder(default)]
  pub(crate) stylesheet: Arc<StyleSheet>,
  /// Global animation time in milliseconds.
  #[builder(default = 0)]
  pub(crate) time_ms: u64,
  /// Per-render font fallback chain (family names in order).
  #[builder(default)]
  pub(crate) font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root, inherited by nodes without their own `lang`.
  #[builder(default)]
  pub(crate) lang: Option<Lang>,
}

/// Renders a node tree to a vector SVG string.
pub fn render(options: SvgOptions<'_>) -> Result<String> {
  let viewport = options.viewport;

  let context = RenderContext::builder()
    .fonts(
      options
        .fonts
        .snapshot_with_fallbacks(options.font_families.as_ref()),
    )
    .sizing(SizingContext::builder().viewport(viewport).build())
    .images(Rc::new(options.images))
    .stylesheet(options.stylesheet)
    .time_ms(options.time_ms)
    .style(Box::new(ComputedStyle {
      lang: options.lang,
      font_family: options.font_families.unwrap_or_default(),
      ..Default::default()
    }))
    .build();

  let root = RenderNode::from_node(&context, options.node);
  let mut tree = LayoutTree::from_render_node(&root);

  tree.compute_layout(viewport.into());

  let results = tree.into_results();
  let root_layout = results.layout(NodeId::ROOT)?;
  let width = viewport
    .size
    .width
    .map_or(root_layout.size.width, |w| w as f32);
  let height = viewport
    .size
    .height
    .map_or(root_layout.size.height, |h| h as f32);
  let mut doc = SvgDocument::new(width, height)?;

  let contexts = build_scene(SceneRequest {
    root: &root,
    layout_results: &results,
    transform: Affine::IDENTITY,
    container_size: Size {
      width: Some(width),
      height: Some(height),
    },
    paint_bounds: true,
  })?;
  SceneEmitter {
    root: &root,
    contexts: &contexts,
    results: &results,
  }
  .emit(&mut doc)?;

  Ok(doc.render()?)
}

/// A box's open effect groups and deferred outline, closed after its content.
pub(crate) struct BoxChrome {
  /// The outline, painted when the box closes. CSS 2.1 Appendix E puts it
  /// above the box's own content, so it cannot go with the other decorations.
  outline: Option<PendingOutline>,
  blend: Option<GroupToken>,
  isolate: Option<GroupToken>,
  mask: Option<GroupToken>,
  filter_wrappers: Vec<GroupToken>,
  outer: Option<GroupToken>,
  clip_group: Option<GroupToken>,
  child_group: Option<GroupToken>,
}

impl BoxChrome {
  /// Emits a box's shared chrome and opens its child group.
  pub(crate) fn open(
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    group_transform: Affine,
    doc: &mut SvgDocument,
  ) -> io::Result<Self> {
    let style = &node.context.style;

    let blend = (style.mix_blend_mode != BlendMode::Normal)
      .then(|| doc.begin_blend_group(&style.mix_blend_mode.to_css_string()))
      .transpose()?;

    let isolate = (style.isolation == Isolation::Isolate)
      .then(|| doc.begin_isolate_group())
      .transpose()?;

    let mask = emit_mask_group(node, layout.size, x, y, doc)?;

    let opacity = style.opacity.0;
    let filter_refs = doc.filter(
      &style.filter,
      &node.context.sizing,
      node.context.current_color,
      layout.size,
      false,
    )?;
    let filter_wrappers = doc.begin_filter_wrappers(&filter_refs)?;
    let outer = (!group_transform.is_identity() || opacity < 1.0 || !filter_refs.is_empty())
      .then(|| {
        doc.begin_group(
          group_transform,
          opacity,
          None,
          filter_refs.first().map(String::as_str),
        )
      })
      .transpose()?;

    // Anchor the filter region to the border box: the raster backend filters the
    // element's full layer box, but an SVG filter's default objectBoundingBox
    // region collapses when nothing inside the group paints (e.g. an empty
    // overlay driving feTurbulence). The invisible rect only ever grows the bbox,
    // so painted content is unaffected.
    if !filter_refs.is_empty() {
      doc.rect(
        x,
        y,
        layout.size.width,
        layout.size.height,
        Rgba([0, 0, 0, 0]),
      )?;
    }

    let clip_group = emit_clip_path_group(node, layout.size, x, y, doc)?;
    let border = BorderProperties::from_context(&node.context, layout.size, layout.border);

    emit_box_shadows(node, &border, layout, x, y, doc)?;

    // `background-clip` picks the shape a background fills, never when it paints:
    // the border draws over the ring, as it does in Blink.
    emit_background(node, &border, layout, x, y, doc)?;
    emit_inset_box_shadows(node, &border, layout, x, y, doc)?;
    emit_borders(&border, x, y, layout.size, doc)?;

    // Children, clipped to the (rounded) padding box when overflow is not visible.
    // With border-radius present the raster backend clips both axes to the rounded
    // padding box regardless of the per-axis overflow values, so the rounded path is
    // used as-is. Without radius a two-value overflow (e.g. `overflow-x: hidden;
    // overflow-y: visible`) must leave the visible axis unbounded.
    let child_group = style
      .clips_overflow()
      .then(|| {
        let path = if border.is_zero() {
          overflow_clip_rect_data(style, layout, x, y)
        } else {
          padding_box_path_data(&border, layout, x, y)
        };

        doc.begin_clipped_group(&path)
      })
      .transpose()?;

    Ok(Self {
      outline: PendingOutline::new(node, layout, x, y),
      blend,
      isolate,
      mask,
      filter_wrappers,
      outer,
      clip_group,
      child_group,
    })
  }

  pub(crate) fn take_outline(&mut self) -> Option<PendingOutline> {
    self.outline.take()
  }

  /// Closes a box's groups and paints its deferred outline.
  pub(crate) fn close(self, doc: &mut SvgDocument) -> io::Result<()> {
    if let Some(group) = self.child_group {
      doc.end_group(group)?;
    }
    if let Some(pending) = self.outline {
      pending.paint(doc)?;
    }
    let groups = [self.clip_group, self.outer];
    for group in groups.into_iter().flatten() {
      doc.end_group(group)?;
    }
    for group in self.filter_wrappers.into_iter().rev() {
      doc.end_group(group)?;
    }
    let groups = [self.mask, self.isolate, self.blend];
    for group in groups.into_iter().flatten() {
      doc.end_group(group)?;
    }
    Ok(())
  }
}

/// The `background-origin` positioning area as an absolute frame within the box.
fn background_origin_frame(origin: BackgroundOrigin, layout: Layout, x: f32, y: f32) -> Frame {
  let b = layout.border;
  let p = layout.padding;
  let frame = |left: f32, right: f32, top: f32, bottom: f32| {
    Frame::new(
      x + left,
      y + top,
      (layout.size.width - left - right).max(0.0),
      (layout.size.height - top - bottom).max(0.0),
    )
  };

  match origin {
    BackgroundOrigin::PaddingBox => frame(b.left, b.right, b.top, b.bottom),
    BackgroundOrigin::ContentBox => frame(
      b.left + p.left,
      b.right + p.right,
      b.top + p.top,
      b.bottom + p.bottom,
    ),
    _ => Frame::new(x, y, layout.size.width, layout.size.height),
  }
}

/// Emits the element's background (color then image layers) clipped to the region selected by
/// `background-clip`.
fn emit_background(
  node: &RenderNode,
  border: &BorderProperties,
  layout: Layout,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let style = &node.context.style;
  if style.background_clip == BackgroundClip::Text {
    return Ok(());
  }

  // The colour fill carries the clip shape itself, so it goes outside the
  // group. Only the image layers need the clip.
  if style.background_color.resolve(node.context.current_color).0[3] != 0 {
    let mut device = DocumentDevice::new(doc);

    BoxPainter::new(&node.context, layout).background_color(Point { x, y }, &mut device);
    device.finish()?;
  }

  let Some(images) = style
    .background_image
    .as_deref()
    .filter(|images| !images.is_empty())
  else {
    return Ok(());
  };
  let group = background_clip_path(style.background_clip, border, layout, x, y)
    .map(|(data, even_odd)| {
      let clip = doc.clip_path(&data, even_odd, None)?;

      doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
    })
    .transpose()?;

  LayerEmitter::new(&node.context, doc).background_images(
    images,
    background_origin_frame(style.background_origin, layout, x, y),
    Frame::new(x, y, layout.size.width, layout.size.height),
  )?;
  if let Some(group) = group {
    doc.end_group(group)?;
  }
  Ok(())
}

/// Emits the element's `mask-image` as an SVG `<mask>` painted into the border
/// box at `(x, y)` and opens the masked group wrapping the element.
pub(crate) fn emit_mask_group(
  node: &RenderNode,
  size: Size<f32>,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<Option<GroupToken>> {
  let style = &node.context.style;
  let Some(images) = style.mask_image.as_deref() else {
    return Ok(None);
  };
  if !images.iter().any(BackgroundImage::paints) {
    return Ok(None);
  }
  if size.width <= 0.0 || size.height <= 0.0 {
    return Ok(None);
  }

  let (token, reference) = doc.begin_mask()?;
  let frame = Frame::new(x, y, size.width, size.height);

  LayerEmitter::new(&node.context, doc).image_layers(
    images,
    &style.mask_size,
    &style.mask_position,
    &style.mask_repeat,
    frame,
    frame,
  )?;
  doc.end_mask(token)?;
  Ok(Some(doc.begin_masked_group(&reference)?))
}

/// Resolves `clip-path` against the border box at `(x, y)` and opens a clip group
/// wrapping the element. Mirrors the raster backend's `render_clip_shape_mask`
/// geometry.
pub(crate) fn emit_clip_path_group(
  node: &RenderNode,
  size: Size<f32>,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<Option<GroupToken>> {
  let Some(shape) = node.context.style.clip_path.as_ref() else {
    return Ok(None);
  };
  let sizing = &node.context.sizing;
  let clip = match shape {
    BasicShape::Ellipse(ellipse) => {
      let cx = x + ellipse.position.0.x.to_px(sizing, size.width);
      let cy = y + ellipse.position.0.y.to_px(sizing, size.height);
      // closest/farthest-side measure each axis from the center to BOTH of its
      // sides, not just the top-left corner.
      let rx = resolve_shape_radius(
        ellipse.radius_x,
        cx - x,
        x + size.width - cx,
        sizing,
        size.width,
      );
      let ry = resolve_shape_radius(
        ellipse.radius_y,
        cy - y,
        y + size.height - cy,
        sizing,
        size.height,
      );
      doc.clip_ellipse(cx, cy, rx, ry)?
    }
    BasicShape::Inset(inset) => {
      let [top_l, right_l, bottom_l, left_l] = inset.inset.0;
      let top = top_l.to_px(sizing, size.height);
      let right = right_l.to_px(sizing, size.width);
      let bottom = bottom_l.to_px(sizing, size.height);
      let left = left_l.to_px(sizing, size.width);
      let inner = Size {
        width: (size.width - left - right).max(0.0),
        height: (size.height - top - bottom).max(0.0),
      };
      let mut border = BorderProperties::default();
      if let Some(radius) = inset.border_radius {
        border.radius = Sides(
          radius
            .0
            .map(|corner| SpacePair::from_single(corner.to_px(sizing, size.width))),
        );
      }
      let clip = ClipBox {
        border,
        size: inner,
        offset: Point { x: left, y: top },
      };
      doc.clip_path(&clip_box_path_data(clip, x, y), false, None)?
    }
    BasicShape::Polygon(polygon) => {
      if polygon.coordinates.is_empty() {
        return Ok(None);
      }
      let mut data =
        PathData::with_capacity(polygon.coordinates.len() * (2 * APPROX_CHARS_PER_NUMBER + 1));
      for (index, coord) in polygon.coordinates.iter().enumerate() {
        let px = x + coord.x.to_px(sizing, size.width);
        let py = y + coord.y.to_px(sizing, size.height);
        data.command(if index == 0 { b'M' } else { b'L' });
        data.pair(px, py);
      }
      data.close();
      let data = data.into_string();
      let even_odd = polygon.fill_rule.unwrap_or(node.context.style.clip_rule) == FillRule::EvenOdd;
      doc.clip_path(&data, even_odd, None)?
    }
    BasicShape::Path(path) => {
      let even_odd = path.fill_rule.unwrap_or(node.context.style.clip_rule) == FillRule::EvenOdd;
      // Inner scale lifts CSS-px path() coords to device space; translate offsets after.
      let [tx, ty, scale] = [x, y, sizing.to_device(1.0)].map(Num);
      let transform = format!("translate({tx} {ty}) scale({scale})");
      doc.clip_path(&path.path, even_odd, Some(&transform))?
    }
    _ => return Ok(None),
  };
  let group = doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)?;

  Ok(Some(group))
}

/// Resolves a [`ShapeRadius`] to pixels.
fn resolve_shape_radius(
  radius: ShapeRadius,
  near: f32,
  far: f32,
  sizing: &SizingContext,
  full: f32,
) -> f32 {
  match radius {
    ShapeRadius::ClosestSide => near.min(far),
    ShapeRadius::FarthestSide => near.max(far),
    ShapeRadius::Length(length) => length.to_px(sizing, full),
  }
}

/// An absolute SVG path for a [`ClipBox`]'s rounded rectangle.
fn clip_box_path_data(clip: ClipBox, x: f32, y: f32) -> String {
  let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
  clip
    .border
    .append_mask_commands(&mut commands, clip.size, clip.offset);
  path_data(&commands, [1.0, 0.0, 0.0, 1.0, x, y])
}

/// Absolute SVG path `d` for a rounded rectangle of `size` with `border`'s corner geometry.
pub(crate) fn border_box_path_data(
  border: &BorderProperties,
  size: Size<f32>,
  x: f32,
  y: f32,
) -> String {
  clip_box_path_data(
    ClipBox {
      border: *border,
      size,
      offset: Point::ZERO,
    },
    x,
    y,
  )
}

/// Absolute SVG path `d` for the padding-box rounded rectangle.
fn padding_box_path_data(border: &BorderProperties, layout: Layout, x: f32, y: f32) -> String {
  clip_box_path_data(ClipBox::padding_box(*border, layout), x, y)
}

/// Absolute SVG path `d` for the (non-rounded) overflow clip rectangle.
fn overflow_clip_rect_data(style: &ComputedStyle, layout: Layout, x: f32, y: f32) -> String {
  const UNBOUNDED: f32 = 1.0e6;
  let overflow = style.resolve_overflows();
  let clip_x = overflow.x != Overflow::Visible;
  let clip_y = overflow.y != Overflow::Visible;

  let (left, right) = if clip_x {
    let padding_left = x + layout.border.left;
    let padding_right = (x + layout.size.width - layout.border.right).max(padding_left);
    (padding_left, padding_right)
  } else {
    (x - UNBOUNDED, x + layout.size.width + UNBOUNDED)
  };
  let (top, bottom) = if clip_y {
    let padding_top = y + layout.border.top;
    let padding_bottom = (y + layout.size.height - layout.border.bottom).max(padding_top);
    (padding_top, padding_bottom)
  } else {
    (y - UNBOUNDED, y + layout.size.height + UNBOUNDED)
  };

  rect_path_data(left, top, right, bottom)
}

/// A [`PaintDevice`] writing into an [`SvgDocument`], keeping the first write error.
pub(crate) struct DocumentDevice<'d> {
  doc: &'d mut SvgDocument,
  error: Option<io::Error>,
}

impl<'d> DocumentDevice<'d> {
  pub(crate) fn new(doc: &'d mut SvgDocument) -> Self {
    Self { doc, error: None }
  }

  /// Surfaces the first write error.
  pub(crate) fn finish(self) -> io::Result<()> {
    self.error.map_or(Ok(()), Err)
  }
}

impl PaintDevice for DocumentDevice<'_> {
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    if self.error.is_some() {
      return;
    }
    let result = match shape {
      FillShape::Rect(size) if transform.only_translation() => self.doc.rect(
        transform.x,
        transform.y,
        size.width,
        size.height,
        Rgba(color.0),
      ),
      _ => {
        let data = path_data(&shape.to_commands(), transform.to_cols_array());

        self.doc.path(
          &data,
          Rgba(color.0),
          matches!(shape.rule(), FillRule::EvenOdd),
        )
      }
    };

    if let Err(error) = result {
      self.error = Some(error);
    }
  }

  fn stroke_shape(&mut self, shape: &FillShape, stroke: &StrokeStyle, transform: Affine) {
    if self.error.is_some() {
      return;
    }
    let data = path_data(&shape.to_commands(), transform.to_cols_array());

    if let Err(error) = self.doc.stroke_path(&data, stroke) {
      self.error = Some(error);
    }
  }
}

/// Builds the clip path and fill rule for a `background-clip` area.
fn background_clip_path(
  clip: BackgroundClip,
  border: &BorderProperties,
  layout: Layout,
  x: f32,
  y: f32,
) -> Option<(String, bool)> {
  match clip {
    BackgroundClip::PaddingBox => Some((padding_box_path_data(border, layout, x, y), false)),
    BackgroundClip::ContentBox => Some((
      clip_box_path_data(ClipBox::content_box(*border, layout), x, y),
      false,
    )),
    BackgroundClip::BorderArea => {
      // The border ring: the (rounded) border-box with the (rounded) padding box
      // punched out, drawn even-odd so the background shows only under the border.
      let outer = border_box_path_data(border, layout.size, x, y);
      let inner = padding_box_path_data(border, layout, x, y);
      Some((format!("{outer}{inner}"), true))
    }
    // `text` is handled separately by the text path; anything else clips to the
    // border box.
    _ => (!border.is_zero()).then(|| (border_box_path_data(border, layout.size, x, y), false)),
  }
}

/// Emits a node's own content — its inline run set, or its replaced image/text —
/// at the border-box top-left `(x, y)`. Block children are painted separately.
pub(crate) fn emit_own_content(
  node: &RenderNode,
  layout: Layout,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if node.should_create_inline_layout() {
    return emit_inline_content(node, layout, x, y, doc);
  }
  // A node whose anonymous text became a child item paints that text through the
  // child, not as its own content (mirroring the raster backend's guard).
  if node.has_anonymous_text_item_child() {
    return Ok(());
  }
  emit_replaced_content(node, layout, x, y, doc)
}

/// Emits an image or text leaf at the border-box top-left `(x, y)`.
fn emit_replaced_content(
  node: &RenderNode,
  layout: Layout,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  match node.node.as_ref().map(|n| &n.kind) {
    Some(NodeKind::Image(image)) => emit_image_node(image, node, layout, x, y, doc),
    Some(NodeKind::Text(text)) => emit_text(text, &node.context, layout, x, y, doc),
    _ => Ok(()),
  }
}

/// Recurses into an in-flow inline box (an atomic inline element such as an inline-block or
/// replaced box) positioned by the inline layout.
pub(crate) fn emit_inline_box(
  inline_box: &VisualInlineBox,
  item: &InlineBoxItem<'_>,
  container_layout: Layout,
  container_x: f32,
  container_y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Some((offset, paint)) = resolve_inline_box(inline_box, item, container_layout) else {
    return Ok(());
  };
  let box_x = container_x + offset.x;
  let box_y = container_y + offset.y;

  match paint {
    InlineBoxPaint::Container(subtree) => {
      let origin = Affine::translation(
        box_x + subtree.margin_offset.x,
        box_y + subtree.margin_offset.y,
      );
      let contexts = build_scene(SceneRequest {
        root: &subtree.root,
        layout_results: &subtree.results,
        transform: origin,
        container_size: subtree.size.map(Some),
        paint_bounds: true,
      })
      .map_err(io::Error::other)?;

      SceneEmitter {
        root: &subtree.root,
        contexts: &contexts,
        results: &subtree.results,
      }
      .emit(doc)
    }
    InlineBoxPaint::Replaced { node, layout } => {
      let group_transform =
        element_transform(&node.context, layout.size, box_x, box_y).unwrap_or(Affine::IDENTITY);
      let chrome = BoxChrome::open(node, layout, box_x, box_y, group_transform, doc)?;

      emit_replaced_content(node, layout, box_x, box_y, doc)?;
      chrome.close(doc)
    }
  }
}

/// Emits an image node's content into its content box.
fn emit_image_node(
  image: &ImageData,
  node: &RenderNode,
  layout: Layout,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  // `x`/`y` are the element's absolute border-box top-left; the content box is
  // inset by the border and padding (not `content_box_x`, which also folds in the
  // element's own `location` relative to its parent).
  let content = Frame::new(
    x + layout.border.left + layout.padding.left,
    y + layout.border.top + layout.padding.top,
    layout.content_box_width(),
    layout.content_box_height(),
  );
  let border = BorderProperties::from_context(&node.context, layout.size, layout.border);
  if border.is_zero() {
    return emit_image(image, &node.context, content, doc);
  }

  let group = doc.begin_clipped_group(&padding_box_path_data(&border, layout, x, y))?;
  emit_image(image, &node.context, content, doc)?;
  doc.end_group(group)
}

/// Emits the element's borders, reusing takumi-core's `BorderProperties` geometry.
fn emit_borders(
  border: &BorderProperties,
  x: f32,
  y: f32,
  size: Size<f32>,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if !border.has_visible_sides() {
    return Ok(());
  }

  let matrix = [1.0, 0.0, 0.0, 1.0, x, y];
  let mut device = DocumentDevice::new(doc);

  if paint_border(border, size, Point { x, y }, &mut device) {
    return device.finish();
  }

  let mut sides = border.painted_sides().peekable();

  if sides.peek().is_none() {
    return Ok(());
  }
  // Mixed per-side styles/colors: clip to the ring; fill solid sides as their
  // diagonal-split polygon and stroke dashed/dotted sides along their centerline.
  // A collapsed border's sides are squared rectangles already inside the ring,
  // so the clip only adds an antialiased edge that leaks the background where
  // two cells meet. Patterned sides still need it to trim their centerlines.
  let patterned = border
    .painted_sides()
    .any(|side| matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted));
  let clip = if border.collapsed && !patterned {
    None
  } else {
    let mut ring = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    border.append_border_ring_commands(&mut ring, size);

    Some(doc.clip_path(&path_data(&ring, matrix), true, None)?)
  };
  let group = doc.begin_group(Affine::IDENTITY, 1.0, clip.as_deref(), None)?;
  for side in sides {
    match side.style {
      BorderStyle::Dashed | BorderStyle::Dotted => {
        emit_side_pattern(border, side, matrix, size, doc)?;
      }
      _ => {
        for band in border.side_bands(side) {
          let mut strip = *border;

          strip.width = band.width;
          strip.expand_by(band.inset.map(|value| -value));

          let mut polygon = Vec::new();
          strip.append_side_clip_polygon_commands_at(
            side.side,
            &mut polygon,
            size.inset(band.inset),
            band.inset.top_left(),
          );
          doc.path(&path_data(&polygon, matrix), Rgba(band.color.0), false)?;
        }
      }
    }
  }
  doc.end_group(group)
}

/// Strokes one dashed/dotted border side along its centerline.
fn emit_side_pattern(
  border: &BorderProperties,
  side: PaintedSide,
  matrix: [f32; 6],
  size: Size<f32>,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let (half_top, half_right, half_bottom, half_left) = (
    border.width.top / 2.0,
    border.width.right / 2.0,
    border.width.bottom / 2.0,
    border.width.left / 2.0,
  );
  let ((x0, y0), (x1, y1)) = match side.side {
    BorderSide::Top => ((half_left, half_top), (size.width - half_right, half_top)),
    BorderSide::Right => (
      (size.width - half_right, half_top),
      (size.width - half_right, size.height - half_bottom),
    ),
    BorderSide::Bottom => (
      (half_left, size.height - half_bottom),
      (size.width - half_right, size.height - half_bottom),
    ),
    BorderSide::Left => (
      (half_left, half_top),
      (half_left, size.height - half_bottom),
    ),
  };
  let [a, b, c, d, e, f] = matrix;
  let map = |px: f32, py: f32| (a * px + c * py + e, b * px + d * py + f);
  let (mx0, my0) = map(x0, y0);
  let (mx1, my1) = map(x1, y1);
  let mut path = PathData::with_capacity(4 * APPROX_CHARS_PER_NUMBER);
  path.command(b'M');
  path.pair(mx0, my0);
  path.command(b'L');
  path.pair(mx1, my1);
  let length = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
  let dash = side.style.dash_pattern(side.width, length, false);

  doc.stroke_path(
    &path.into_string(),
    &StrokeStyle {
      color: side.color,
      width: side.width,
      dash: dash.map(|dash| dash.intervals),
      round_cap: dash.is_some_and(|dash| dash.round_cap),
    },
  )
}

/// A box's CSS `outline`, deferred until its content is painted.
pub(crate) struct PendingOutline {
  outline: OutlineGeometry,
  x: f32,
  y: f32,
}

impl PendingOutline {
  fn new(node: &RenderNode, layout: Layout, x: f32, y: f32) -> Option<Self> {
    let color = node
      .context
      .style
      .outline_color
      .resolve(node.context.current_color);

    if color.0[3] == 0 {
      return None;
    }

    Some(Self {
      outline: BoxPainter::fragment(&node.context, layout, layout.size).outline()?,
      x,
      y,
    })
  }

  /// Paints the outline as a ring around the border box, grown by
  /// `outline-offset + outline-width`.
  pub(crate) fn paint(&self, doc: &mut SvgDocument) -> io::Result<()> {
    emit_borders(
      &self.outline.border,
      self.x - self.outline.grow,
      self.y - self.outline.grow,
      self.outline.size,
      doc,
    )
  }
}

/// Runs `emit` inside a Gaussian-blur group when `blur_radius` is positive (the CSS shadow blur is
/// `2σ`), or directly otherwise.
fn emit_with_blur(
  doc: &mut SvgDocument,
  blur_radius: f32,
  emit: impl FnOnce(&mut SvgDocument) -> io::Result<()>,
) -> io::Result<()> {
  if blur_radius > 0.0 {
    let filter = doc.blur_filter(blur_radius / 2.0)?;
    let group = doc.begin_group(Affine::IDENTITY, 1.0, None, Some(&filter))?;
    emit(doc)?;
    doc.end_group(group)
  } else {
    emit(doc)
  }
}

/// Emits outset `box-shadow`s behind the element as offset, blurred rects.
fn emit_box_shadows(
  node: &RenderNode,
  border: &BorderProperties,
  layout: Layout,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  for resolved in BoxPainter::new(&node.context, layout).shadows().outer {
    // Shadow shape = the element's rounded border-box, radii expanded by the
    // spread (shared core geometry with the raster backend).
    let (shadow, spread_size) = border.outset_shadow_box(layout.size, resolved.spread_radius);
    if spread_size.width <= 0.0 || spread_size.height <= 0.0 {
      continue;
    }

    let sx = x + resolved.offset_x - resolved.spread_radius;
    let sy = y + resolved.offset_y - resolved.spread_radius;
    let fill = Rgba(resolved.color.0);
    let data = border_box_path_data(&shadow, spread_size, sx, sy);

    emit_with_blur(doc, resolved.blur_radius, |doc| {
      doc.path(&data, fill, false)
    })?;
  }
  Ok(())
}

/// Emits inset `box-shadow`s as a blurred ring inside the element's rounded padding box.
fn emit_inset_box_shadows(
  node: &RenderNode,
  border: &BorderProperties,
  layout: Layout,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if layout.size.width <= 0.0 || layout.size.height <= 0.0 {
    return Ok(());
  }
  let padding = ClipBox::padding_box(*border, layout);
  let outer = clip_box_path_data(padding, x, y);
  for resolved in BoxPainter::new(&node.context, layout).shadows().inset {
    let fill = Rgba(resolved.color.0);

    // The shadow fills the padding box minus the hole it leaves uncovered
    // (shared core geometry with the raster backend), drawn even-odd, blurred,
    // and clipped to the rounded padding box so the blur stays inside.
    let hole = ClipBox::inset_shadow_hole(
      padding.border,
      padding.size,
      resolved.spread_radius,
      Point {
        x: resolved.offset_x,
        y: resolved.offset_y,
      },
    );
    let ring = format!(
      "{outer}{}",
      clip_box_path_data(hole, x + padding.offset.x, y + padding.offset.y)
    );
    let clip_group = doc.begin_clipped_group(&outer)?;
    emit_with_blur(doc, resolved.blur_radius, |doc| doc.path(&ring, fill, true))?;
    doc.end_group(clip_group)?;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn renders_svg_wrapper_at_viewport_size() {
    let fonts = Fonts::default();
    let svg = render(
      SvgOptions::builder()
        .node(Node::container([]))
        .viewport(Viewport::new((120, 80)))
        .fonts(&fonts)
        .build(),
    )
    .unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("width=\"120\""));
    assert!(svg.contains("height=\"80\""));
    assert!(!svg.contains("base64"));
  }
}
