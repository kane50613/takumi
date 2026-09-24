//! End-to-end SVG rendering: run takumi-core layout, walk the tree, emit SVG.

use std::{collections::HashMap, io, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  error::Result,
  geometry::{Point, Rect, Size},
  layout::{
    border::{BorderProperties, BorderSide, PaintedSide},
    decoration::{ClipBox, OutlineGeometry},
    inline::{InlineBoxItem, VisualInlineBox},
    inline_box::{InlineBoxPaint, resolve_inline_box},
    node::{ImageData, Node, NodeKind},
    tree::RenderNode,
  },
  painter::{BoxPainter, FillShape, PaintDevice, StrokeStyle, paint_border},
  resources::image::ImageSource,
  scene::Scene,
  style::{
    Affine, BackgroundClip, BackgroundImage, BasicShape, BlendMode, BorderStyle, Color,
    ComputedStyle, FillRule, FontFamily, Isolation, Lang, Overflow, ShapeRadius, Sides,
    SizingContext, SpacePair, StyleSheet, ToCss,
  },
  viewport::Viewport,
};
use typed_builder::TypedBuilder;

use crate::{
  APPROX_CHARS_PER_NUMBER, Frame, GroupToken, Num, Rgba, SvgDocument,
  box_model::{
    BoxFrame, PathData, clip_box_path_data, edges_path_data, path_data, rounded_rect_path_data,
  },
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
    .style(Box::new(ComputedStyle::root(
      options.lang,
      options.font_families,
    )))
    .build();

  let scene = Scene::lay_out(
    RenderNode::from_node(&context, options.node),
    viewport,
    true,
  )?;
  let mut doc = SvgDocument::new(scene.size.width, scene.size.height)?;

  SceneEmitter { scene: &scene }.emit(&mut doc)?;

  Ok(doc.finish()?)
}

/// A render node laid out at its [`BoxFrame`].
pub(crate) struct PlacedBox<'n> {
  pub node: &'n RenderNode,
  pub frame: BoxFrame,
  painter: BoxPainter<'n>,
}

impl<'n> PlacedBox<'n> {
  pub(crate) fn new(node: &'n RenderNode, frame: BoxFrame) -> Self {
    Self {
      node,
      frame,
      painter: BoxPainter::new(&node.context, frame.layout),
    }
  }

  /// The box's border geometry, corners included.
  pub(crate) fn border(&self) -> &BorderProperties {
    self.painter.border()
  }

  /// The element's paint transform moved into absolute space, or `None` when
  /// it has none.
  fn element_transform(&self) -> Option<Affine> {
    let context = &self.node.context;
    let Point { x, y } = self.frame.origin;
    let size = self.frame.layout.size;
    let local = context
      .style
      .local_transform(size.width, size.height, &context.sizing);

    if local.is_identity() {
      return None;
    }
    // Children are emitted in absolute coordinates; move the local transform into
    // that space: M_abs = T(x, y) * local * T(-x, -y).
    Some(Affine::translation(x, y) * local * Affine::translation(-x, -y))
  }

  /// Absolute SVG path `d` for the rounded border box.
  pub(crate) fn border_box_path_data(&self) -> String {
    rounded_rect_path_data(self.border(), self.frame.layout.size, self.frame.origin)
  }

  /// Absolute SVG path `d` for the rounded padding box.
  fn padding_box_path_data(&self) -> String {
    clip_box_path_data(
      ClipBox::padding_box(*self.border(), self.frame.layout),
      self.frame.origin,
    )
  }

  /// Absolute SVG path `d` the box clips its children to when overflow is not
  /// visible.
  fn overflow_clip_path_data(&self) -> String {
    // With border-radius present the raster backend clips both axes to the rounded
    // padding box regardless of the per-axis overflow values, so the rounded path is
    // used as-is. Without radius a two-value overflow (e.g. `overflow-x: hidden;
    // overflow-y: visible`) must leave the visible axis unbounded.
    if !self.border().is_zero() {
      return self.padding_box_path_data();
    }

    const UNBOUNDED: f32 = 1.0e6;
    let BoxFrame {
      layout,
      origin: Point { x, y },
    } = self.frame;
    let overflow = self.node.context.style.resolve_overflows();
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

    edges_path_data(Rect {
      left,
      top,
      right,
      bottom,
    })
  }

  /// The clip path `d` and fill rule for the `background-clip` area.
  fn background_clip_path_data(&self) -> Option<(String, FillRule)> {
    let border = self.border();

    match self.node.context.style.background_clip {
      BackgroundClip::PaddingBox => Some((self.padding_box_path_data(), FillRule::NonZero)),
      BackgroundClip::ContentBox => Some((
        clip_box_path_data(
          ClipBox::content_box(*border, self.frame.layout),
          self.frame.origin,
        ),
        FillRule::NonZero,
      )),
      BackgroundClip::BorderArea => {
        // The border ring: the (rounded) border-box with the (rounded) padding box
        // punched out, drawn even-odd so the background shows only under the border.
        let outer = self.border_box_path_data();
        let inner = self.padding_box_path_data();
        Some((format!("{outer}{inner}"), FillRule::EvenOdd))
      }
      // `text` is handled separately by the text path; anything else clips to the
      // border box.
      _ => (!border.is_zero()).then(|| (self.border_box_path_data(), FillRule::NonZero)),
    }
  }

  /// Emits the element's background (color then image layers) clipped to the
  /// region selected by `background-clip`.
  fn emit_background(&self, doc: &mut SvgDocument) -> io::Result<()> {
    let context = &self.node.context;
    let style = &context.style;
    if style.background_clip == BackgroundClip::Text {
      return Ok(());
    }

    // The colour fill carries the clip shape itself, so it goes outside the
    // group. Only the image layers need the clip.
    if style.background_color.resolve(context.current_color).0[3] != 0 {
      let mut device = DocumentDevice::new(doc);

      self
        .painter
        .background_color(self.frame.origin, &mut device);
      device.finish()?;
    }

    let Some(images) = style
      .background_image
      .as_deref()
      .filter(|images| !images.is_empty())
    else {
      return Ok(());
    };
    let group = self
      .background_clip_path_data()
      .map(|(data, rule)| {
        let clip = doc.clip_path(&data, rule, None)?;

        doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
      })
      .transpose()?;

    LayerEmitter::new(context, doc).background_images(
      images,
      self.frame.background_origin_box(style.background_origin),
      self.frame.border_box(),
    )?;
    if let Some(group) = group {
      doc.end_group(group)?;
    }
    Ok(())
  }

  /// Emits the element's `mask-image` as an SVG `<mask>` painted into the border
  /// box and opens the masked group wrapping the element.
  pub(crate) fn begin_mask_group(&self, doc: &mut SvgDocument) -> io::Result<Option<GroupToken>> {
    let style = &self.node.context.style;
    let Some(images) = style.mask_image.as_deref() else {
      return Ok(None);
    };
    if !images.iter().any(BackgroundImage::paints) {
      return Ok(None);
    }
    let size = self.frame.layout.size;
    if size.width <= 0.0 || size.height <= 0.0 {
      return Ok(None);
    }

    let (token, reference) = doc.begin_mask()?;
    let border_box = self.frame.border_box();

    LayerEmitter::new(&self.node.context, doc).image_layers(
      images,
      &style.mask_size,
      &style.mask_position,
      &style.mask_repeat,
      border_box,
      border_box,
    )?;
    doc.end_mask(token)?;
    Ok(Some(doc.begin_masked_group(&reference)?))
  }

  /// Resolves `clip-path` against the border box and opens a clip group wrapping
  /// the element. Mirrors the raster backend's `render_clip_shape_mask` geometry.
  pub(crate) fn begin_clip_path_group(
    &self,
    doc: &mut SvgDocument,
  ) -> io::Result<Option<GroupToken>> {
    let style = &self.node.context.style;
    let Some(shape) = style.clip_path.as_ref() else {
      return Ok(None);
    };
    let sizing = &self.node.context.sizing;
    let Point { x, y } = self.frame.origin;
    let size = self.frame.layout.size;
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
        doc.clip_path(
          &clip_box_path_data(clip, self.frame.origin),
          FillRule::NonZero,
          None,
        )?
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
        let rule = polygon.fill_rule.unwrap_or(style.clip_rule);

        doc.clip_path(&data.into_string(), rule, None)?
      }
      BasicShape::Path(path) => {
        let rule = path.fill_rule.unwrap_or(style.clip_rule);
        // Inner scale lifts CSS-px path() coords to device space; translate offsets after.
        let [tx, ty, scale] = [x, y, sizing.to_device(1.0)].map(Num);
        let transform = format!("translate({tx} {ty}) scale({scale})");
        doc.clip_path(&path.path, rule, Some(&transform))?
      }
      _ => return Ok(None),
    };
    let group = doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)?;

    Ok(Some(group))
  }

  /// Emits outset `box-shadow`s behind the element as offset, blurred rects.
  fn emit_box_shadows(&self, doc: &mut SvgDocument) -> io::Result<()> {
    let BoxFrame { layout, origin } = self.frame;

    for resolved in self.painter.shadows().outer {
      // Shadow shape = the element's rounded border-box, radii expanded by the
      // spread (shared core geometry with the raster backend).
      let (shadow, spread_size) = self
        .border()
        .outset_shadow_box(layout.size, resolved.spread_radius);
      if spread_size.width <= 0.0 || spread_size.height <= 0.0 {
        continue;
      }

      let shadow_origin = Point {
        x: origin.x + resolved.offset_x - resolved.spread_radius,
        y: origin.y + resolved.offset_y - resolved.spread_radius,
      };
      let fill = Rgba(resolved.color.0);
      let data = rounded_rect_path_data(&shadow, spread_size, shadow_origin);

      doc.with_blur(resolved.blur_radius, |doc| {
        doc.fill_path(&data, fill, FillRule::NonZero)
      })?;
    }
    Ok(())
  }

  /// Emits inset `box-shadow`s as a blurred ring inside the element's rounded
  /// padding box.
  fn emit_inset_box_shadows(&self, doc: &mut SvgDocument) -> io::Result<()> {
    let BoxFrame { layout, origin } = self.frame;
    if layout.size.width <= 0.0 || layout.size.height <= 0.0 {
      return Ok(());
    }
    let padding = ClipBox::padding_box(*self.border(), layout);
    let outer = clip_box_path_data(padding, origin);
    for resolved in self.painter.shadows().inset {
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
        clip_box_path_data(hole, origin + padding.offset)
      );
      let clip_group = doc.begin_clipped_group(&outer)?;
      doc.with_blur(resolved.blur_radius, |doc| {
        doc.fill_path(&ring, fill, FillRule::EvenOdd)
      })?;
      doc.end_group(clip_group)?;
    }
    Ok(())
  }

  /// Emits the node's own content: its inline run set, or its replaced
  /// image/text. Block children are painted separately.
  pub(crate) fn emit_own_content(&self, doc: &mut SvgDocument) -> io::Result<()> {
    if self.node.should_create_inline_layout() {
      return emit_inline_content(self.node, self.frame, doc);
    }
    // A node whose anonymous text became a child item paints that text through the
    // child, not as its own content (mirroring the raster backend's guard).
    if self.node.has_anonymous_text_item_child() {
      return Ok(());
    }
    self.emit_replaced_content(doc)
  }

  /// Emits an image or text leaf.
  fn emit_replaced_content(&self, doc: &mut SvgDocument) -> io::Result<()> {
    match self.node.node.as_ref().map(|n| &n.kind) {
      Some(NodeKind::Image(image)) => self.emit_image(image, doc),
      Some(NodeKind::Text(text)) => emit_text(text, &self.node.context, self.frame, doc),
      _ => Ok(()),
    }
  }

  /// Emits an image node's content into its content box.
  fn emit_image(&self, image: &ImageData, doc: &mut SvgDocument) -> io::Result<()> {
    let context = &self.node.context;
    let content = self.frame.content_box();
    if self.border().is_zero() {
      return emit_image(image, context, content, doc);
    }

    let group = doc.begin_clipped_group(&self.padding_box_path_data())?;
    emit_image(image, context, content, doc)?;
    doc.end_group(group)
  }
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
    placed: &PlacedBox,
    group_transform: Affine,
    doc: &mut SvgDocument,
  ) -> io::Result<Self> {
    let context = &placed.node.context;
    let style = &context.style;

    let blend = (style.mix_blend_mode != BlendMode::Normal)
      .then(|| doc.begin_blend_group(&style.mix_blend_mode.to_css_string()))
      .transpose()?;

    let isolate = (style.isolation == Isolation::Isolate)
      .then(|| doc.begin_isolate_group())
      .transpose()?;

    let mask = placed.begin_mask_group(doc)?;

    let opacity = style.opacity.0;
    let filter_refs = doc.filter(&style.filter, context, placed.frame.layout.size, false)?;
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
      doc.rect(placed.frame.border_box(), Rgba::TRANSPARENT)?;
    }

    let clip_group = placed.begin_clip_path_group(doc)?;

    placed.emit_box_shadows(doc)?;

    // `background-clip` picks the shape a background fills, never when it paints:
    // the border draws over the ring, as it does in Blink.
    placed.emit_background(doc)?;
    placed.emit_inset_box_shadows(doc)?;
    emit_borders(
      placed.border(),
      placed.frame.layout.size,
      placed.frame.origin,
      doc,
    )?;

    // Children, clipped to the (rounded) padding box when overflow is not visible.
    let child_group = style
      .clips_overflow()
      .then(|| doc.begin_clipped_group(&placed.overflow_clip_path_data()))
      .transpose()?;

    Ok(Self {
      outline: PendingOutline::new(placed),
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
      pending.emit(doc)?;
    }
    let groups = [self.clip_group, self.outer];
    for group in groups.into_iter().flatten() {
      doc.end_group(group)?;
    }
    doc.end_filter_wrappers(self.filter_wrappers)?;
    let groups = [self.mask, self.isolate, self.blend];
    for group in groups.into_iter().flatten() {
      doc.end_group(group)?;
    }
    Ok(())
  }
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
        Frame::new(transform.x, transform.y, size.width, size.height),
        Rgba(color.0),
      ),
      _ => {
        let data = path_data(&shape.to_commands(), transform);

        self.doc.fill_path(&data, Rgba(color.0), shape.rule())
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
    let data = path_data(&shape.to_commands(), transform);

    if let Err(error) = self.doc.stroke_path(&data, stroke) {
      self.error = Some(error);
    }
  }
}

/// Recurses into an in-flow inline box (an atomic inline element such as an inline-block or
/// replaced box) positioned by the inline layout.
pub(crate) fn emit_inline_box(
  inline_box: &VisualInlineBox,
  item: &InlineBoxItem<'_>,
  container: BoxFrame,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Some((offset, paint)) = resolve_inline_box(inline_box, item, container.layout) else {
    return Ok(());
  };
  let origin = container.origin + offset;

  match paint {
    InlineBoxPaint::Container(subtree) => {
      let at = subtree.border_box_origin(origin);
      let scene = subtree
        .into_scene(Affine::translation(at.x, at.y), true)
        .map_err(io::Error::other)?;

      SceneEmitter { scene: &scene }.emit(doc)
    }
    InlineBoxPaint::Replaced { node, layout } => {
      let placed = PlacedBox::new(node, BoxFrame::new(layout, origin));
      let group_transform = placed.element_transform().unwrap_or(Affine::IDENTITY);
      let chrome = BoxChrome::open(&placed, group_transform, doc)?;

      placed.emit_replaced_content(doc)?;
      chrome.close(doc)
    }
  }
}

/// Emits a border's rings at `origin`, reusing takumi-core's `BorderProperties` geometry.
fn emit_borders(
  border: &BorderProperties,
  size: Size<f32>,
  origin: Point<f32>,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if !border.has_visible_sides() {
    return Ok(());
  }

  let transform = Affine::translation(origin.x, origin.y);
  let mut device = DocumentDevice::new(doc);

  if paint_border(border, size, origin, &mut device) {
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

    Some(doc.clip_path(&path_data(&ring, transform), FillRule::EvenOdd, None)?)
  };
  let group = doc.begin_group(Affine::IDENTITY, 1.0, clip.as_deref(), None)?;
  for side in sides {
    match side.style {
      BorderStyle::Dashed | BorderStyle::Dotted => {
        emit_side_pattern(border, side, size, transform, doc)?;
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
          doc.fill_path(
            &path_data(&polygon, transform),
            Rgba(band.color.0),
            FillRule::NonZero,
          )?;
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
  size: Size<f32>,
  transform: Affine,
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
  let [a, b, c, d, e, f] = transform.to_cols_array();
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
  origin: Point<f32>,
}

impl PendingOutline {
  fn new(placed: &PlacedBox) -> Option<Self> {
    let context = &placed.node.context;
    let color = context.style.outline_color.resolve(context.current_color);

    if color.0[3] == 0 {
      return None;
    }

    Some(Self {
      outline: placed.painter.outline()?,
      origin: placed.frame.origin,
    })
  }

  /// Paints the outline as a ring around the border box, grown by
  /// `outline-offset + outline-width`.
  pub(crate) fn emit(&self, doc: &mut SvgDocument) -> io::Result<()> {
    emit_borders(
      &self.outline.border,
      self.outline.size,
      Point {
        x: self.origin.x - self.outline.grow,
        y: self.origin.y - self.outline.grow,
      },
      doc,
    )
  }
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
