//! End-to-end SVG rendering: run takumi-core layout, walk the tree, emit SVG.

use std::{collections::HashMap, io, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  error::Result,
  font_style::SizedFontStyle,
  geometry::{Point, Rect, Size},
  layout::{
    background_image_geometry::{BoxBackgroundPaintContext, FillLayers},
    border::BorderProperties,
    inline::{InlineBoxItem, InlinePass, PositionedInlineRun, VisualInlineBox},
    inline_box::{InlineBoxPaint, resolve_inline_box},
    node::Node,
    tree::RenderNode,
  },
  painter::{
    BackgroundClipArea, BoxBackground, BoxBorderPainter, BoxFrame, BoxPainter, FillShape,
    GlyphDevice, GlyphFill, LayerBounds, OverflowClip, OwnContent, PaintDevice, ShadowShape,
    SpanBackground, StrokeStyle, UNBOUNDED,
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
  text::{
    ClipTextBackground, emit_clip_text_run, emit_inline_content, emit_run_glyphs, run_stroke,
  },
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

  /// Absolute SVG path `d` the box clips its children to, or `None` when
  /// overflow is visible.
  fn overflow_clip_path_data(&self) -> Option<String> {
    Some(
      match OverflowClip::of(
        &self.node.context,
        self.frame.layout,
        self.node.context.box_paint_offset(self.frame.layout),
      )? {
        OverflowClip::Rounded(clip) => shape_path_data(&(*clip).into(), self.frame.origin),
        OverflowClip::Axes(edges) => edges_path_data(Rect {
          left: self.frame.origin.x + edges.left,
          top: self.frame.origin.y + edges.top,
          right: self.frame.origin.x + edges.right,
          bottom: self.frame.origin.y + edges.bottom,
        }),
      },
    )
  }

  /// The clip path `d` and fill rule for a background's `clip` area. A square border box needs
  /// none, since the layers already stay inside it.
  fn background_clip_path_data(&self, background: &BoxBackground) -> Option<(String, FillRule)> {
    match background.clip.shape(background.size)? {
      FillShape::Rect(_) => None,
      shape => Some((
        shape_path_data(&shape, self.frame.origin + background.offset),
        shape.rule(),
      )),
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
    if let Some(mask) = background.clip.border_mask() {
      DocumentDevice::paint(doc, |device| {
        let origin = self.frame.origin + background.offset;

        device.with_border_mask(&mask, background.size, origin, |device| {
          device.write(|doc| {
            LayerEmitter::new(&self.node.context, doc)
              .layers(&background.layers, Frame::border_box(self.frame))
          });
        });
      })?;
      if let Some(isolate) = isolate {
        doc.end_group(isolate)?;
      }
      return Ok(());
    }
    let group = self
      .background_clip_path_data(&background)
      .map(|(data, rule)| {
        let clip = doc.clip_path(&data, rule, None)?;

        doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
      })
      .transpose()?;
    LayerEmitter::new(&self.node.context, doc)
      .layers(&background.layers, Frame::border_box(self.frame))?;
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

    let layers = FillLayers::mask(style).resolve(
      images,
      &BoxBackgroundPaintContext::mask(size, self.node.context.box_paint_offset(self.frame.layout)),
      &self.node.context,
    );

    LayerEmitter::new(&self.node.context, doc).layers(&layers, border_box)?;
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

  /// Emits the box's shadows, background and border.
  pub(crate) fn emit_decorations(&self, doc: &mut SvgDocument) -> io::Result<()> {
    if !self.node.paints_own_box() {
      return Ok(());
    }

    self.emit_box_shadows(doc)?;
    // `background-clip` picks the shape a background fills, never when it paints:
    // the border draws over the ring, as it does in Blink.
    self.emit_background(doc)?;
    self.emit_inset_box_shadows(doc)?;
    DocumentDevice::paint(doc, |device| {
      self.painter.paint_border(self.frame.origin, device);
    })
  }

  /// Emits the box's outline.
  pub(crate) fn emit_outline(&self, doc: &mut SvgDocument) -> io::Result<()> {
    match self.painter.pending_outline(self.frame.origin) {
      Some(outline) => DocumentDevice::paint(doc, |device| outline.paint(device)),
      None => Ok(()),
    }
  }

  /// Emits the node's own content: its inline content or its image. Block children are
  /// painted separately.
  pub(crate) fn emit_own_content(&self, pass: InlinePass, doc: &mut SvgDocument) -> io::Result<()> {
    match OwnContent::of(self.node) {
      OwnContent::Inline(_) => emit_inline_content(self.node, self.frame, pass, doc),
      OwnContent::Image(image) if pass == InlinePass::Content => {
        emit_image(image, &self.painter, self.frame, doc)
      }
      OwnContent::Image(_) | OwnContent::None => Ok(()),
    }
  }
}

/// The groups a box's effects open around everything it and its descendants paint.
pub(crate) struct EffectGroups(Vec<GroupToken>);

impl EffectGroups {
  /// Opens `placed`'s blend, isolation, mask, filter and opacity groups, moved by
  /// `group_transform`, then its `clip-path`.
  pub(crate) fn open(
    placed: &PlacedBox,
    group_transform: Affine,
    doc: &mut SvgDocument,
  ) -> io::Result<Self> {
    let context = &placed.node.context;
    let style = &context.style;
    let mut groups = Vec::new();

    if style.mix_blend_mode != BlendMode::Normal {
      groups.push(doc.begin_blend_group(&style.mix_blend_mode.to_css_string())?);
    }
    if style.isolation == Isolation::Isolate {
      groups.push(doc.begin_isolate_group()?);
    }
    groups.extend(placed.begin_mask_group(doc)?);

    let opacity = style.opacity.0;
    let filter_refs = doc.filter(&style.filter, context, placed.frame.layout.size, false)?;

    groups.extend(doc.begin_filter_wrappers(&filter_refs)?);

    if !group_transform.is_identity() || opacity < 1.0 || !filter_refs.is_empty() {
      groups.push(doc.begin_group(
        group_transform,
        opacity,
        None,
        filter_refs.first().map(String::as_str),
      )?);
    }

    // Anchor the filter region to the border box: the raster backend filters the
    // element's full layer box, but an SVG filter's default objectBoundingBox
    // region collapses when nothing inside the group paints (e.g. an empty
    // overlay driving feTurbulence). The invisible rect only ever grows the bbox,
    // so painted content is unaffected.
    if !filter_refs.is_empty() {
      doc.rect(Frame::border_box(placed.frame), Rgba::TRANSPARENT)?;
    }

    groups.extend(placed.begin_clip_path_group(doc)?);

    Ok(Self(groups))
  }

  /// The groups, outermost first.
  pub(crate) fn into_tokens(self) -> Vec<GroupToken> {
    self.0
  }

  /// Closes the groups, innermost first.
  pub(crate) fn close(self, doc: &mut SvgDocument) -> io::Result<()> {
    for group in self.0.into_iter().rev() {
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
      Some((shadow, _)) => (shadow, self.shadow_moved(transform)),
      None => (color, transform),
    }
  }

  /// `transform` moved by the open shadow's offset. A clip opened inside a shadow clips what casts
  /// it, as Blink draws a text shadow's content into a `DropShadowPaintFilter` layer.
  fn shadow_moved(&self, transform: Affine) -> Affine {
    match self.shadow {
      Some((_, offset)) => Affine::translation(offset.x, offset.y) * transform,
      None => transform,
    }
  }
}

impl PaintDevice for DocumentDevice<'_> {
  fn transform(&self) -> Affine {
    self.doc.transform()
  }

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
    let transform = self.shadow_moved(transform);

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
    let data = format!(
      "{everywhere}{}",
      path_data(&shape.to_commands(), self.shadow_moved(transform))
    );

    self.begin_clip(&data, FillRule::EvenOdd);
  }

  fn push_aliased_clip_out(&mut self, shape: &FillShape, transform: Affine) {
    let everywhere = edges_path_data(Rect {
      left: -UNBOUNDED,
      top: -UNBOUNDED,
      right: UNBOUNDED,
      bottom: UNBOUNDED,
    });
    let data = format!(
      "{everywhere}{}",
      path_data(&shape.to_commands(), self.shadow_moved(transform))
    );

    self.open_group(|doc| {
      let clip = doc.aliased_clip_path(&data, FillRule::EvenOdd)?;

      doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
    });
  }

  fn push_aliased_clip(&mut self, shape: &FillShape, transform: Affine) {
    let data = path_data(&shape.to_commands(), self.shadow_moved(transform));

    self.open_group(|doc| {
      let clip = doc.aliased_clip_path(&data, shape.rule())?;

      doc.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
    });
  }

  fn pop_clip(&mut self) {
    self.close_group();
  }

  fn with_border_mask(
    &mut self,
    border: &BorderProperties,
    size: Size<f32>,
    origin: Point<f32>,
    content: impl FnOnce(&mut Self),
  ) {
    if self.error.is_some() {
      return;
    }
    let (token, reference) = match self.doc.begin_mask() {
      Ok(mask) => mask,
      Err(error) => {
        self.error = Some(error);
        return;
      }
    };

    BoxBorderPainter::new(border, size).paint(origin, self);
    self.write(|doc| doc.end_mask(token));
    self.open_group(|doc| doc.begin_masked_group(&reference));
    content(self);
    self.close_group();
  }

  fn begin_layer(&mut self, opacity: f32, _bounds: Option<LayerBounds>) {
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
  fn fill_background_layers(
    &mut self,
    span: &SpanBackground<'_>,
    clip: &FillShape,
    transform: Affine,
  ) {
    self.push_clip(clip, transform);
    self.write(|doc| {
      LayerEmitter::new(&span.node.context, doc)
        .layers(&span.background.layers, Frame::border_box(span.strip))
    });
    self.pop_clip();
  }

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

    let context = self
      .text_background
      .filter(|_| fill == GlyphFill::Background);
    let background = context.map(|context| BoxPainter::new(context, frame.layout).background());
    let fill = context
      .zip(background.as_ref())
      .map(|(context, background)| ClipTextBackground {
        context,
        background,
        area: frame,
      });

    self.emit_glyph_run(run, style, frame, fill.as_ref());
  }

  fn draw_glyph_run_through(
    &mut self,
    run: &PositionedInlineRun,
    style: &SizedFontStyle,
    frame: BoxFrame,
    span: &SpanBackground<'_>,
  ) {
    let fill = ClipTextBackground {
      context: &span.node.context,
      background: &span.background,
      area: span.strip,
    };

    self.write(|doc| emit_clip_text_run(run, style, frame, &fill, doc));
  }
}

impl DocumentDevice<'_> {
  /// Emits `run`'s glyphs in the block at `frame`, over `fill` seen through them.
  fn emit_glyph_run(
    &mut self,
    run: &PositionedInlineRun,
    style: &SizedFontStyle,
    frame: BoxFrame,
    fill: Option<&ClipTextBackground<'_>>,
  ) {
    let stroke = run_stroke(&run.glyph_run, style);

    self.write(|doc| {
      if let Some(fill) = fill {
        emit_clip_text_run(run, style, frame, fill, doc)?;
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
      let groups = EffectGroups::open(&placed, group_transform, doc)?;

      placed.emit_decorations(doc)?;

      let content_clip = placed
        .overflow_clip_path_data()
        .map(|data| doc.begin_clipped_group(&data))
        .transpose()?;

      placed.emit_own_content(InlinePass::Content, doc)?;
      if let Some(group) = content_clip {
        doc.end_group(group)?;
      }
      placed.emit_outline(doc)?;
      groups.close(doc)
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
