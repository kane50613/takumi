//! End-to-end SVG rendering: run takumi-core layout, walk the tree, emit SVG.

use std::{collections::HashMap, io, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  error::Result,
  font_style::SizedFontStyle,
  geometry::{Point, Rect},
  layout::{
    background_image_geometry::FillLayers,
    border::BorderProperties,
    decoration::ClipBox,
    inline::{InlineBoxItem, PositionedInlineRun, VisualInlineBox},
    inline_box::{InlineBoxPaint, resolve_inline_box},
    node::{ImageData, Node, NodeKind},
    tree::RenderNode,
  },
  painter::{
    BackgroundClipArea, BoxFrame, BoxPainter, FillShape, GlyphDevice, GlyphFill, OverflowClip,
    PaintDevice, PendingOutline, ShadowShape, StrokeStyle, UNBOUNDED,
  },
  path_data::{edges_path_data, path_data},
  resources::image::ImageSource,
  scene::Scene,
  shadow::SizedShadow,
  style::{
    Affine, BackgroundImage, BlendMode, Color, ComputedStyle, FillRule, FontFamily, Isolation,
    Lang, SizingContext, StyleSheet, ToCss,
  },
  viewport::Viewport,
};
use typed_builder::TypedBuilder;

use crate::{
  Frame, GlyphStroke, GroupToken, Rgba, SvgDocument,
  box_model::{rounded_rect_path_data, shape_path_data},
  gradient::LayerEmitter,
  image::emit_image,
  scene_emit::SceneEmitter,
  text::{emit_clip_text_run, emit_inline_content, emit_run_glyphs, emit_text, run_stroke},
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
    shape_path_data(
      &ClipBox::padding_box(*self.border(), self.frame.layout).into(),
      self.frame.origin,
    )
  }

  /// Absolute SVG path `d` the box clips its children to, or `None` when
  /// overflow is visible.
  fn overflow_clip_path_data(&self) -> Option<String> {
    Some(
      match OverflowClip::of(&self.node.context, self.frame.layout)? {
        OverflowClip::Rounded(clip) => shape_path_data(&clip.into(), self.frame.origin),
        OverflowClip::Axes { x, y } => edges_path_data(self.frame.overflow_clip_edges(x, y)),
      },
    )
  }

  /// The clip path `d` and fill rule for a background's `clip` area. A square border box needs
  /// none.
  fn background_clip_path_data(&self, clip: BackgroundClipArea) -> Option<(String, FillRule)> {
    match clip {
      BackgroundClipArea::BorderBox(border) => {
        (!border.is_zero()).then(|| (self.border_box_path_data(), FillRule::NonZero))
      }
      BackgroundClipArea::Inner(clip) => Some((
        shape_path_data(&clip.into(), self.frame.origin),
        FillRule::NonZero,
      )),
      BackgroundClipArea::BorderArea(_) => {
        // The border ring: the (rounded) border-box with the (rounded) padding box
        // punched out, drawn even-odd so the background shows only under the border.
        let outer = self.border_box_path_data();
        let inner = self.padding_box_path_data();
        Some((format!("{outer}{inner}"), FillRule::EvenOdd))
      }
      BackgroundClipArea::Text => None,
    }
  }

  /// Emits the element's background (color then image layers) clipped to the
  /// region selected by `background-clip`.
  fn emit_background(&self, doc: &mut SvgDocument) -> io::Result<()> {
    let background = self.painter.background();
    if matches!(background.clip, BackgroundClipArea::Text) {
      return Ok(());
    }
    // A blending layer mixes with the layers and color beneath it and nothing behind the box.
    let isolate = background
      .layers
      .iter()
      .any(|layer| layer.blend_mode != BlendMode::Normal)
      .then(|| doc.begin_isolate_group())
      .transpose()?;

    // The colour fill carries the clip shape itself, so it goes outside the
    // group. Only the image layers need the clip.
    if background.color.is_some() {
      DocumentDevice::paint(doc, |device| {
        self.painter.background_color(self.frame.origin, device);
      })?;
    }

    if background.layers.is_empty() {
      return Ok(());
    }
    let group = self
      .background_clip_path_data(background.clip)
      .map(|(data, rule)| {
        let clip = doc.clip_path(&data, rule, None)?;

        doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
      })
      .transpose()?;
    LayerEmitter::new(&self.node.context, doc).layers(
      &background.layers,
      Frame::origin_box(self.frame, background.origin),
      Frame::border_box(self.frame),
    )?;
    if let Some(group) = group {
      doc.end_group(group)?;
    }
    if let Some(isolate) = isolate {
      doc.end_group(isolate)?;
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
    let border_box = Frame::border_box(self.frame);

    let layers = FillLayers::mask(style).resolve(images, size, &self.node.context);

    LayerEmitter::new(&self.node.context, doc).layers(&layers, border_box, border_box)?;
    doc.end_mask(token)?;
    Ok(Some(doc.begin_masked_group(&reference)?))
  }

  /// Opens a group clipping the element and its descendants to its `clip-path`.
  pub(crate) fn begin_clip_path_group(
    &self,
    doc: &mut SvgDocument,
  ) -> io::Result<Option<GroupToken>> {
    let Some(shape) = self.painter.clip_path() else {
      return Ok(None);
    };
    let clip = doc.clip_shape(&shape, self.frame.translation())?;

    doc
      .begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
      .map(Some)
  }

  /// Emits the outer `box-shadow`s behind the element.
  fn emit_box_shadows(&self, doc: &mut SvgDocument) -> io::Result<()> {
    DocumentDevice::paint(doc, |device| {
      self
        .painter
        .paint_normal_box_shadows(self.frame.origin, device);
    })
  }

  /// Emits the inset `box-shadow`s inside the element's padding box.
  fn emit_inset_box_shadows(&self, doc: &mut SvgDocument) -> io::Result<()> {
    DocumentDevice::paint(doc, |device| {
      self
        .painter
        .paint_inset_box_shadows(self.frame.origin, device);
    })
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
    emit_image(image, &self.painter, self.frame, doc)
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
      doc.rect(Frame::border_box(placed.frame), Rgba::TRANSPARENT)?;
    }

    let clip_group = placed.begin_clip_path_group(doc)?;

    placed.emit_box_shadows(doc)?;

    // `background-clip` picks the shape a background fills, never when it paints:
    // the border draws over the ring, as it does in Blink.
    placed.emit_background(doc)?;
    placed.emit_inset_box_shadows(doc)?;
    DocumentDevice::paint(doc, |device| {
      placed.painter.paint_border(placed.frame.origin, device);
    })?;

    // Children, clipped to the (rounded) padding box when overflow is not visible.
    let child_group = placed
      .overflow_clip_path_data()
      .map(|data| doc.begin_clipped_group(&data))
      .transpose()?;

    Ok(Self {
      outline: placed.painter.pending_outline(placed.frame.origin),
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
      DocumentDevice::paint(doc, |device| pending.paint(device))?;
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

/// A [`PaintDevice`] writing into an [`SvgDocument`], keeping the first write error.
pub(crate) struct DocumentDevice<'d> {
  doc: &'d mut SvgDocument,
  groups: Vec<GroupToken>,
  /// The shadow every draw becomes while one is open: its colour and offset.
  shadow: Option<(Color, Point<f32>)>,
  /// The box whose background `background-clip: text` glyphs show.
  text_background: Option<&'d RenderContext>,
  error: Option<io::Error>,
}

impl<'d> DocumentDevice<'d> {
  pub(crate) fn new(doc: &'d mut SvgDocument) -> Self {
    Self {
      doc,
      groups: Vec::new(),
      shadow: None,
      text_background: None,
      error: None,
    }
  }

  /// Surfaces the first write error.
  pub(crate) fn finish(self) -> io::Result<()> {
    self.error.map_or(Ok(()), Err)
  }

  /// Runs `paint` against `doc`, surfacing the first write error.
  pub(crate) fn paint(doc: &'d mut SvgDocument, paint: impl FnOnce(&mut Self)) -> io::Result<()> {
    let mut device = Self::new(doc);

    paint(&mut device);
    device.finish()
  }

  /// [`DocumentDevice::paint`] for the text of the box `context` paints, whose background shows
  /// through `background-clip: text` glyphs.
  pub(crate) fn paint_text(
    doc: &'d mut SvgDocument,
    context: &'d RenderContext,
    paint: impl FnOnce(&mut Self),
  ) -> io::Result<()> {
    let mut device = Self::new(doc);

    device.text_background = Some(context);
    paint(&mut device);
    device.finish()
  }

  /// Runs `write` against the document unless an earlier write failed, keeping its error.
  pub(crate) fn write(&mut self, write: impl FnOnce(&mut SvgDocument) -> io::Result<()>) {
    if self.error.is_some() {
      return;
    }
    if let Err(error) = write(self.doc) {
      self.error = Some(error);
    }
  }

  /// Opens the group `open` writes, keeping the first write error.
  fn open_group(&mut self, open: impl FnOnce(&mut SvgDocument) -> io::Result<GroupToken>) {
    if self.error.is_some() {
      return;
    }

    match open(self.doc) {
      Ok(group) => self.groups.push(group),
      Err(error) => self.error = Some(error),
    }
  }

  /// Closes the most recent group, keeping the first write error.
  fn close_group(&mut self) {
    if let Some(group) = self.groups.pop() {
      self.write(|doc| doc.end_group(group));
    }
  }

  /// Opens a group clipped to `data`.
  fn begin_clip(&mut self, data: &str, rule: FillRule) {
    self.open_group(|doc| {
      let clip = doc.clip_path(data, rule, None)?;

      doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
    });
  }

  /// `color` and `transform`, or the open shadow's colour and `transform` moved by its offset.
  fn shadowed(&self, color: Color, transform: Affine) -> (Color, Affine) {
    match self.shadow {
      Some((shadow, offset)) => (shadow, Affine::translation(offset.x, offset.y) * transform),
      None => (color, transform),
    }
  }
}

impl PaintDevice for DocumentDevice<'_> {
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    let (color, transform) = self.shadowed(color, transform);

    self.write(|doc| match shape {
      FillShape::Rect(size) if transform.only_translation() => doc.rect(
        Frame::new(transform.x, transform.y, size.width, size.height),
        Rgba(color.0),
      ),
      _ => doc.fill_path(
        &path_data(&shape.to_commands(), transform),
        Rgba(color.0),
        shape.rule(),
      ),
    });
  }

  fn stroke_shape(&mut self, shape: &FillShape, stroke: &StrokeStyle, transform: Affine) {
    let (color, transform) = self.shadowed(stroke.color, transform);
    let stroke = StrokeStyle { color, ..*stroke };

    self.write(|doc| doc.stroke_path(&path_data(&shape.to_commands(), transform), &stroke));
  }

  fn push_clip(&mut self, shape: &FillShape, transform: Affine) {
    self.open_group(|doc| {
      let clip = doc.clip_shape(shape, transform)?;

      doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
    });
  }

  fn push_clip_out(&mut self, shape: &FillShape, transform: Affine) {
    let everywhere = edges_path_data(Rect {
      left: -UNBOUNDED,
      top: -UNBOUNDED,
      right: UNBOUNDED,
      bottom: UNBOUNDED,
    });
    let data = format!("{everywhere}{}", path_data(&shape.to_commands(), transform));

    self.begin_clip(&data, FillRule::EvenOdd);
  }

  fn pop_clip(&mut self) {
    self.close_group();
  }

  fn begin_layer(&mut self, opacity: f32) {
    self.open_group(|doc| doc.begin_group(Affine::IDENTITY, opacity, None, None));
  }

  fn end_layer(&mut self) {
    self.close_group();
  }

  fn fill_shadow(&mut self, shape: &ShadowShape, shadow: &SizedShadow, transform: Affine) {
    let fill = shape.fill_shape();
    let data = path_data(
      &fill.to_commands(),
      Affine::translation(shadow.offset_x, shadow.offset_y) * transform,
    );

    self.write(|doc| {
      doc.with_blur(shadow.blur_radius, |doc| {
        doc.fill_path(&data, Rgba(shadow.color.0), fill.rule())
      })
    });
  }
}

impl GlyphDevice for DocumentDevice<'_> {
  fn begin_shadow(&mut self, shadow: &SizedShadow) {
    self.open_group(|doc| {
      let filter = (shadow.blur_radius > 0.0)
        .then(|| doc.blur_filter(shadow.blur_radius / 2.0))
        .transpose()?;

      doc.begin_group(Affine::IDENTITY, 1.0, None, filter.as_deref())
    });
    self.shadow = Some((
      shadow.color,
      Point {
        x: shadow.offset_x,
        y: shadow.offset_y,
      },
    ));
  }

  fn end_shadow(&mut self) {
    self.shadow = None;
    self.close_group();
  }

  fn draw_glyph_run(
    &mut self,
    run: &PositionedInlineRun,
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
  ) {
    let stroke = run_stroke(&run.glyph_run, style);

    if let Some((color, offset)) = self.shadow {
      let color = Rgba(color.0);
      let stroke = stroke.map(|stroke| GlyphStroke { color, ..stroke });

      return self
        .write(|doc| emit_run_glyphs(run, style, frame.shifted(offset), Some(color), stroke, doc));
    }

    let background = self
      .text_background
      .filter(|_| fill == GlyphFill::Background);

    self.write(|doc| {
      if let Some(context) = background {
        emit_clip_text_run(run, style, context, frame, doc)?;
      }

      emit_run_glyphs(run, style, frame, None, stroke, doc)
    });
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
