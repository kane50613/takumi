//! End-to-end SVG rendering: run takumi-core layout, walk the tree, emit SVG.

use std::{collections::HashMap, io, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  error::Result,
  geometry::{ComputedLayout as Layout, NodeId, Point, Size},
  layout::{
    border::{
      BorderProperties, BorderSide, PaintedSide, border_dash_pattern, inset_size, rect_offset,
      side_bands,
    },
    decoration::{ClipBox, OutlineGeometry},
    inline::{InlineBoxItem, VisualInlineBox},
    inline_box::{InlineBoxPaint, resolve_inline_box},
    node::{ImageData, Node, NodeKind},
    tree::{LayoutTree, RenderNode},
  },
  painter::{BoxPainter, FillShape, PaintDevice, StrokeStyle, paint_border},
  resources::image::ImageSource,
  scene::build_stacking_contexts,
  style::{
    Affine, BackgroundClip, BackgroundImage, BackgroundOrigin, BasicShape, BlendMode, BorderStyle,
    Color, ComputedStyle, FillRule, FontFamily, Isolation, Lang, Overflow, ShapeRadius, Sides,
    SizingContext, SpacePair, StyleSheet, ToCss,
  },
  viewport::Viewport,
};
use typed_builder::TypedBuilder;

use crate::{
  APPROX_CHARS_PER_NUMBER, Frame, IDENTITY, Num, Rgba, SvgDocument,
  box_model::{PathData, element_transform, path_data},
  gradient::LayerEmitter,
  image::emit_image,
  scene_emit::emit_scene,
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
  /// Per-render font fallback chain (family names in order). `None` uses all
  /// registered families in registration order.
  #[builder(default)]
  pub(crate) font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root, inherited by nodes without
  /// their own `lang`. Drives locale-aware shaping and line-breaking.
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
    .style({
      Box::new(ComputedStyle {
        lang: options.lang,
        font_family: options.font_families.unwrap_or_default(),
        ..Default::default()
      })
    })
    .build();

  let root = RenderNode::from_node(&context, options.node);
  let mut tree = LayoutTree::from_render_node(&root);

  tree.compute_layout(viewport.into());

  let results = tree.into_results();
  let root_id = NodeId::ROOT;

  let root_layout = results.layout(root_id)?;
  let width = viewport
    .size
    .width
    .map_or(root_layout.size.width, |w| w as f32);
  let height = viewport
    .size
    .height
    .map_or(root_layout.size.height, |h| h as f32);
  let mut doc = SvgDocument::new(width, height)?;

  let contexts = build_stacking_contexts(
    &root,
    &results,
    root_id,
    IDENTITY,
    (Some(width), Some(height)),
  )?;
  emit_scene(&root, &contexts, &results, &mut doc)?;

  Ok(doc.render()?)
}

/// Open group tokens from [`emit_box_chrome`], to be closed (innermost first:
/// `child_group`, `outer`, then `blend`) after the box content.
pub(crate) struct BoxChrome {
  /// The outline, painted when the box closes. CSS 2.1 Appendix E puts it
  /// above the box's own content, so it cannot go with the other decorations.
  outline: Option<PendingOutline>,
  blend: Option<crate::GroupToken>,
  isolate: Option<crate::GroupToken>,
  mask: Option<crate::GroupToken>,
  filter_wrappers: Vec<crate::GroupToken>,
  outer: Option<crate::GroupToken>,
  clip_group: Option<crate::GroupToken>,
  child_group: Option<crate::GroupToken>,
}

/// An outline waiting for its box's content to finish.
pub(crate) struct PendingOutline {
  outline: OutlineGeometry,
  x: f32,
  y: f32,
}

impl BoxChrome {
  pub(crate) fn take_outline(&mut self) -> Option<PendingOutline> {
    self.outline.take()
  }

  /// Closes the box's groups innermost first, painting the outline once the
  /// content group is closed so it lands above the content and outside its
  /// overflow clip.
  pub(crate) fn close(self, doc: &mut SvgDocument) -> io::Result<()> {
    if let Some(group) = self.child_group {
      doc.end_group(group)?;
    }
    if let Some(pending) = self.outline {
      paint_outline(&pending, doc)?;
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

/// Emits a box's shared chrome (the positioning/opacity group, box-shadows,
/// rounded-clipped background, borders), then opens the overflow-clip child group.
/// Decorations are drawn at `(x, y)`; `group_transform` is the group the box (and
/// its children) are emitted under — the node's transform relative to its parent
/// frame, so nesting composes it. The returned group tokens must be closed by the
/// caller after emitting the box's content.
pub(crate) fn emit_box_chrome(
  node: &RenderNode,
  layout: Layout,
  x: f32,
  y: f32,
  group_transform: Affine,
  doc: &mut SvgDocument,
) -> io::Result<BoxChrome> {
  let width = layout.size.width;
  let height = layout.size.height;
  let style = &node.context.style;
  let cc = node.context.current_color;

  let blend = (style.mix_blend_mode != BlendMode::Normal)
    .then(|| doc.begin_blend_group(&style.mix_blend_mode.to_css_string()))
    .transpose()?;

  let isolate = (style.isolation == Isolation::Isolate)
    .then(|| doc.begin_isolate_group())
    .transpose()?;

  let mask = emit_mask_group(node, x, y, width, height, doc)?;

  let opacity = style.opacity.0;
  let filter_refs = doc.filter(&style.filter, &node.context.sizing, cc, layout.size, false)?;
  // Later filters in the list apply after earlier ones, so they wrap outside.
  let filter_wrappers = filter_refs
    .iter()
    .skip(1)
    .rev()
    .map(|reference| doc.begin_group(IDENTITY, 1.0, None, Some(reference)))
    .collect::<io::Result<Vec<_>>>()?;
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
    doc.rect(x, y, width, height, Rgba([0, 0, 0, 0]))?;
  }

  let clip_group = emit_clip_path_group(node, layout.size, x, y, doc)?;

  emit_box_shadows(node, layout, x, y, width, height, doc)?;

  // Border/radius geometry is reused from takumi-core (the same `BorderProperties`
  // the raster backend rasterizes) instead of being reimplemented here.
  let border = BorderProperties::from_context(&node.context, layout.size, layout.border);
  let rounded = !border.is_zero();

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
      let path = if rounded {
        padding_box_path_data(&border, layout, x, y)
      } else {
        overflow_clip_rect_data(style, layout, x, y)
      };
      doc
        .clip_path(&path)
        .and_then(|clip| doc.begin_group(IDENTITY, 1.0, Some(&clip), None))
    })
    .transpose()?;

  Ok(BoxChrome {
    outline: pending_outline(node, layout, layout.size, x, y),
    blend,
    isolate,
    mask,
    filter_wrappers,
    outer,
    clip_group,
    child_group,
  })
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
    BackgroundOrigin::BorderBox => Frame::new(x, y, layout.size.width, layout.size.height),
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

/// Emits the element's background (color then image layers) clipped to the region
/// selected by `background-clip`. Mirrors the raster backend's `draw_background`.
/// `background-clip: text` is suppressed here and painted by the text path.
pub(crate) fn emit_background(
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
  let (width, height) = (layout.size.width, layout.size.height);
  let background = style.background_color.resolve(node.context.current_color);
  let has_bg_image = style
    .background_image
    .as_deref()
    .is_some_and(|images| !images.is_empty());
  if background.0[3] == 0 && !has_bg_image {
    return Ok(());
  }

  // The colour fill carries the clip shape itself, so it goes outside the
  // group. Only the image layers need the clip.
  if background.0[3] != 0 {
    let mut device = DocumentDevice { doc, error: None };

    BoxPainter::new(&node.context, layout).background_color(Point { x, y }, &mut device);
    if let Some(error) = device.error {
      return Err(error);
    }
  }
  if !has_bg_image {
    return Ok(());
  }
  let bg_clip = background_clip_path(style.background_clip, border, layout, x, y)
    .map(|(data, even_odd)| {
      if even_odd {
        doc.clip_path_evenodd(&data)
      } else {
        doc.clip_path(&data)
      }
    })
    .transpose()?;
  let bg_group = bg_clip
    .as_deref()
    .map(|clip| doc.begin_group(IDENTITY, 1.0, Some(clip), None))
    .transpose()?;
  if let Some(images) = style.background_image.as_deref() {
    LayerEmitter::new(&node.context, doc).background_images(
      images,
      background_origin_frame(style.background_origin, layout, x, y),
      Frame::new(x, y, width, height),
    )?;
  }
  if let Some(group) = bg_group {
    doc.end_group(group)?;
  }
  Ok(())
}

/// Emits the element's `mask-image` as an SVG `<mask>` (the mask layers painted
/// into the border box) and opens the masked group wrapping the element. CSS
/// `mask-image` defaults to alpha masking. `x`/`y` are the absolute border-box
/// top-left. Returns the open group token (closed by [`BoxChrome::close`]).
pub(crate) fn emit_mask_group(
  node: &RenderNode,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
  doc: &mut SvgDocument,
) -> io::Result<Option<crate::GroupToken>> {
  let style = &node.context.style;
  let Some(images) = style.mask_image.as_deref() else {
    return Ok(None);
  };
  if !images.iter().any(BackgroundImage::paints) {
    return Ok(None);
  }
  if width <= 0.0 || height <= 0.0 {
    return Ok(None);
  }

  let (token, reference) = doc.begin_mask()?;
  LayerEmitter::new(&node.context, doc).image_layers(
    images,
    &style.mask_size,
    &style.mask_position,
    &style.mask_repeat,
    Frame::new(x, y, width, height),
    Frame::new(x, y, width, height),
  )?;
  doc.end_mask(token)?;
  Ok(Some(doc.begin_masked_group(&reference)?))
}

/// Resolves `clip-path` (a `BasicShape`) against the element's border box and
/// opens a clip group wrapping the element. Mirrors the raster backend's
/// `render_clip_shape_mask` geometry. `x`/`y` are the absolute border-box
/// top-left. Returns the open group token (closed by [`BoxChrome::close`]).
pub(crate) fn emit_clip_path_group(
  node: &RenderNode,
  size: Size<f32>,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<Option<crate::GroupToken>> {
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
      let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
      border.append_mask_commands(&mut commands, inner, Point { x: left, y: top });
      let data = path_data(&commands, [1.0, 0.0, 0.0, 1.0, x, y]);
      doc.clip_path(&data)?
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
      doc.clip_path_transformed(&data, even_odd, None)?
    }
    BasicShape::Path(path) => {
      let even_odd = path.fill_rule.unwrap_or(node.context.style.clip_rule) == FillRule::EvenOdd;
      // Inner scale lifts CSS-px path() coords to device space; translate offsets after.
      let scale = sizing.to_device(1.0);
      let transform = format!("translate({} {}) scale({})", Num(x), Num(y), Num(scale));
      doc.clip_path_transformed(&path.path, even_odd, Some(&transform))?
    }
    _ => return Ok(None),
  };
  Ok(Some(doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?))
}

/// Resolves a [`ShapeRadius`] to pixels, mirroring the raster backend's
/// `resolve_radius` (closest/farthest measured from the resolved center).
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

/// Absolute SVG path `d` for a [`ClipBox`]'s rounded rectangle, reusing core's
/// `BorderProperties` geometry (cubic-bezier corners, overlap-scaled radii).
fn clip_box_path_data(clip: ClipBox, x: f32, y: f32) -> String {
  let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
  clip
    .border
    .append_mask_commands(&mut commands, clip.size, clip.offset);
  path_data(&commands, [1.0, 0.0, 0.0, 1.0, x, y])
}

/// Absolute SVG path `d` for a rounded rectangle of `size` with `border`'s
/// corner geometry. Also used for shadow spread boxes, whose size is not the
/// border box.
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
pub(crate) fn padding_box_path_data(
  border: &BorderProperties,
  layout: Layout,
  x: f32,
  y: f32,
) -> String {
  clip_box_path_data(ClipBox::padding_box(*border, layout), x, y)
}

/// Absolute SVG path `d` for the (non-rounded) overflow clip rectangle. Each
/// clipped axis is bounded to the content box (mirroring the raster backend's
/// rectangular overflow mask); a `visible` axis is left effectively unbounded so
/// content overflows there while being clipped on the other axis.
pub(crate) fn overflow_clip_rect_data(
  style: &ComputedStyle,
  layout: Layout,
  x: f32,
  y: f32,
) -> String {
  const UNBOUNDED: f32 = 1.0e6;
  let clip_x = style.overflow_x != Overflow::Visible;
  let clip_y = style.overflow_y != Overflow::Visible;

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

  let mut path = PathData::with_capacity(5 * APPROX_CHARS_PER_NUMBER);
  path.command(b'M');
  path.pair(left, top);
  path.command(b'H');
  path.number(right);
  path.command(b'V');
  path.number(bottom);
  path.command(b'H');
  path.number(left);
  path.close();
  path.into_string()
}

/// Builds the clip path (and even-odd flag) for the background fill area per
/// `background-clip`, or `None` when no clip is needed (an unrounded
/// `border-box`, which the full border-box rect already covers). Mirrors the
/// raster backend's `draw_background` clip regions.
/// The SVG document as a [`PaintDevice`].
pub(crate) struct DocumentDevice<'d> {
  pub(crate) doc: &'d mut SvgDocument,
  pub(crate) error: Option<io::Error>,
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
          matches!(shape.rule(), takumi_core::style::FillRule::EvenOdd),
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
    let dash = stroke
      .dash
      .map(|[dash, gap]| format!("{} {}", Num(dash), Num(gap)));

    if let Err(error) = self.doc.stroke_path(
      &data,
      Rgba(stroke.color.0),
      stroke.width,
      dash.as_deref(),
      stroke.round_cap.then_some("round"),
    ) {
      self.error = Some(error);
    }
  }
}

fn background_clip_path(
  clip: BackgroundClip,
  border: &BorderProperties,
  layout: Layout,
  x: f32,
  y: f32,
) -> Option<(String, bool)> {
  let rounded = !border.is_zero();
  match clip {
    BackgroundClip::BorderBox => {
      rounded.then(|| (border_box_path_data(border, layout.size, x, y), false))
    }
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
    // `text` is handled separately by the text path; treat anything else as the
    // full border box.
    _ => rounded.then(|| (border_box_path_data(border, layout.size, x, y), false)),
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
  match node.node.as_ref().map(|n| &n.kind) {
    Some(NodeKind::Image(image)) => emit_image_node(image, node, layout, x, y, doc),
    Some(NodeKind::Text(text)) => emit_text(text, &node.context, layout, x, y, doc),
    _ => Ok(()),
  }
}

/// Recurses into an in-flow inline box (an atomic inline element such as an
/// inline-block or replaced box) positioned by the inline layout. Mirrors the
/// raster backend's `draw_inline_box`: lay the child subtree out fresh at the box
/// size, then emit it at the box's absolute origin. `container_x`/`container_y`
/// are the container's border-box top-left; the box is offset by the container's
/// border/padding plus the inline-resolved position.
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
      let contexts = build_stacking_contexts(
        &subtree.root,
        &subtree.results,
        NodeId::ROOT,
        origin,
        (Some(subtree.size.width), Some(subtree.size.height)),
      )
      .map_err(io::Error::other)?;

      emit_scene(&subtree.root, &contexts, &subtree.results, doc)
    }
    InlineBoxPaint::Replaced { node, layout } => {
      let group_transform =
        element_transform(&node.context, layout.size, box_x, box_y).unwrap_or(IDENTITY);
      let chrome = emit_box_chrome(node, layout, box_x, box_y, group_transform, doc)?;

      match node.node.as_ref().map(|n| &n.kind) {
        Some(NodeKind::Image(image)) => emit_image_node(image, node, layout, box_x, box_y, doc)?,
        Some(NodeKind::Text(text)) => emit_text(text, &node.context, layout, box_x, box_y, doc)?,
        _ => {}
      }

      chrome.close(doc)
    }
  }
}

/// Emits an image node's content into its content box. When the element has a
/// border-radius, the replaced content is clipped to the rounded **padding box**
/// (border-box inset by the border widths), mirroring the raster backend's
/// `draw_image` which masks the image with `BorderProperties::inset_by_border_width`.
/// `x`/`y` are the element's absolute border-box top-left.
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
  let ix = x + layout.border.left + layout.padding.left;
  let iy = y + layout.border.top + layout.padding.top;
  let (iw, ih) = (layout.content_box_width(), layout.content_box_height());

  let content = Frame::new(ix, iy, iw, ih);
  let border = BorderProperties::from_context(&node.context, layout.size, layout.border);
  if border.is_zero() {
    return emit_image(image, &node.context, content, doc);
  }

  let path = padding_box_path_data(&border, layout, x, y);
  let clip = doc.clip_path(&path)?;
  let group = doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?;
  emit_image(image, &node.context, content, doc)?;
  doc.end_group(group)
}

/// Emits the element's borders, reusing takumi-core's `BorderProperties` geometry.
/// A uniform-color solid border becomes one even-odd "ring" (outer rounded
/// border-box minus inner rounded padding-box). Mixed per-side colors clip that
/// ring and fill each side's polygon in its own color (diagonal corner split).
/// Uniform `dashed`/`dotted`/`double` borders stroke a centerline rounded-rect.
/// 3D styles (groove/ridge/inset/outset) approximate as a solid fill. `x`/`y` are
/// the absolute border-box top-left; `size` is the border-box size.
/// The absolute placement of a border box: the `[1,0,0,1,x,y]` device-space
/// transform and the border-box size, threaded together through the stroke
/// emitters.
#[derive(Clone, Copy)]
struct BorderGeom {
  matrix: [f32; 6],
  size: Size<f32>,
}

pub(crate) fn emit_borders(
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
  let geom = BorderGeom { matrix, size };
  let mut device = DocumentDevice { doc, error: None };

  if paint_border(border, size, Point { x, y }, &mut device) {
    return device.error.map_or(Ok(()), Err);
  }

  let mut sides = border.painted_sides().peekable();

  if sides.peek().is_none() {
    return Ok(());
  }
  let mut ring = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

  border.append_border_ring_commands(&mut ring, size);

  // Mixed per-side styles/colors: clip to the ring; fill solid sides as their
  // diagonal-split polygon and stroke dashed/dotted sides along their centerline.
  let clip = doc.clip_path_evenodd(&path_data(&ring, matrix))?;
  let group = doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?;
  for side in sides {
    match side.style {
      BorderStyle::Dashed | BorderStyle::Dotted => {
        emit_side_pattern(border, side, geom, doc)?;
      }
      _ => {
        for band in side_bands(border, side) {
          let mut strip = *border;

          strip.width = band.width;
          strip.expand_by(band.inset.map(|value| -value));

          let mut polygon = Vec::new();
          strip.append_side_clip_polygon_commands_at(
            side.side,
            &mut polygon,
            inset_size(size, band.inset),
            rect_offset(band.inset),
          );
          doc.path(&path_data(&polygon, matrix), Rgba(band.color.0), false)?;
        }
      }
    }
  }
  doc.end_group(group)
}

/// Strokes one dashed/dotted border side along its centerline. The centerline is
/// inset by half the side's width and shortened at each end by half the adjacent
/// side's width (matching the raster backend's per-side stroke). Dash intervals
/// mirror the uniform stroked path (dashed `3w 2w`, dotted `0 2w` + round caps).
fn emit_side_pattern(
  border: &BorderProperties,
  side: PaintedSide,
  geom: BorderGeom,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let BorderGeom { matrix, size } = geom;
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
  let data = path.into_string();
  let length = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
  let (dasharray, linecap) = dash_attrs(side.width, side.style, length, false);
  doc.stroke_path(
    &data,
    Rgba(side.color.0),
    side.width,
    dasharray.as_deref(),
    linecap,
  )
}

/// Emits the CSS `outline` as a ring around the border-box, expanded outward by
/// `outline-offset + outline-width`. `outline` does not affect layout; it follows
/// the border-radius. Mirrors the raster backend's `draw_outline`: a uniform
/// `BorderProperties` (outline width/color/style on all four sides, radii from the
/// element) drawn on an expanded box, so all border styles (solid/dashed/dotted/
/// double, and the 3D approximations) are reused from [`emit_borders`].
/// The outline a box will paint once its content is done, or `None` when it
/// paints none. A transparent outline is an element nobody sees, so it is
/// skipped to keep the document smaller.
fn pending_outline(
  node: &RenderNode,
  layout: Layout,
  size: Size<f32>,
  x: f32,
  y: f32,
) -> Option<PendingOutline> {
  let color = node
    .context
    .style
    .outline_color
    .resolve(node.context.current_color);

  if color.0[3] == 0 {
    return None;
  }

  Some(PendingOutline {
    outline: BoxPainter::fragment(&node.context, layout, size).outline()?,
    x,
    y,
  })
}

pub(crate) fn paint_outline(pending: &PendingOutline, doc: &mut SvgDocument) -> io::Result<()> {
  emit_borders(
    &pending.outline.border,
    pending.x - pending.outline.grow,
    pending.y - pending.outline.grow,
    pending.outline.size,
    doc,
  )
}

/// SVG `stroke-dasharray`/`stroke-linecap` for a `dashed`/`dotted` border or
/// outline, computed from the shared [`border_dash_pattern`] so the intervals
/// match the raster backend.
fn dash_attrs(
  width: f32,
  style: BorderStyle,
  length: f32,
  closed: bool,
) -> (Option<String>, Option<&'static str>) {
  match border_dash_pattern(width, style, length, closed) {
    Some(([dash, gap], round_cap)) => (
      Some(format!("{} {}", Num(dash), Num(gap))),
      round_cap.then_some("round"),
    ),
    None => (None, None),
  }
}

/// Runs `emit` inside a Gaussian-blur group when `blur_radius` is positive (the
/// CSS shadow blur is `2σ`), or directly otherwise.
fn emit_with_blur(
  doc: &mut SvgDocument,
  blur_radius: f32,
  emit: impl FnOnce(&mut SvgDocument) -> io::Result<()>,
) -> io::Result<()> {
  if blur_radius > 0.0 {
    let filter = doc.blur_filter(blur_radius / 2.0)?;
    let group = doc.begin_group(IDENTITY, 1.0, None, Some(&filter))?;
    emit(doc)?;
    doc.end_group(group)
  } else {
    emit(doc)
  }
}

/// Emits outset `box-shadow`s behind the element as offset, blurred rects.
/// Inset shadows are handled by [`emit_inset_box_shadows`].
pub(crate) fn emit_box_shadows(
  node: &RenderNode,
  layout: Layout,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  for resolved in BoxPainter::new(&node.context, layout).shadows().outer {
    {}

    // Shadow shape = the element's rounded border-box, radii expanded by the
    // spread (shared core geometry with the raster backend).
    let element_border = BorderProperties::from_context(&node.context, layout.size, layout.border);
    let (shadow, spread_size) = element_border.outset_shadow_box(
      Size {
        width: w,
        height: h,
      },
      resolved.spread_radius,
    );
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

/// Emits inset `box-shadow`s as a blurred ring inside the element's rounded
/// padding box. Mirrors the PDF backend's `emit_inset_shadows`: the shadow
/// color fills the padding box minus an inner rounded-rect (shrunk by the
/// spread, shifted by the offset), blurred and clipped to the rounded padding
/// box.
pub(crate) fn emit_inset_box_shadows(
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
    // (shared core geometry with the raster backend), drawn even-odd.
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

    // Padding box minus the hole, drawn even-odd, blurred, and clipped to the
    // rounded padding box so the blur stays inside the element.
    let clip = doc.clip_path(&outer)?;
    let clip_group = doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?;
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
