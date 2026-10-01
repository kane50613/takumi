//! The scene walker that emits boxes, text and images onto a krilla surface.

use std::{cell::RefCell, collections::HashMap, ptr, rc::Rc};

#[cfg(feature = "images")]
use takumi_core::{
  context::RenderContext,
  layout::node::{ImageData, ImageSourceInput, NodeKind, resolve_image},
  resources::image::ImageSource,
};
use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{
    ComputedLayout as Layout, NodeId, PathCommand, Point as CorePoint, Rect as CoreRect, Size,
  },
  layout::{
    background_image_geometry::{BoxBackgroundPaintContext, FillLayers, ImageTiling},
    border::BorderProperties,
    inline::{
      BuiltInlineLayout, InlinePass, InlineRunLayout, PositionedInlineRun, ProcessedInlineSpan,
      ShapedRun,
    },
    inline_box::{InlineBoxPaint, InlineSubtree, resolve_inline_box},
    tree::{NodeOrigin, RenderNode},
  },
  paint::ConicGradientTile,
  paint_chunk::{ChunkPart, ConversionContext, PaintChunk, PropertySink},
  paint_property::{ClipId, ClipNode, EffectId, EffectNode},
  painter::{
    BoxBackground, BoxBorderPainter, BoxFrame, BoxPainter, FillShape, GlyphDevice, GlyphFill,
    LayerBounds, LineItem, OwnContent, PaintDevice, PendingOutline, ShadowShape, StripBackground,
    StrokeStyle, TextClip, UNBOUNDED,
  },
  scene::{NodePaint, Scene},
  shadow::SizedShadow,
  style::{
    Affine, BackgroundImage, BlendMode, BoxDecorationBreak, Color, ComputedStyle, Display,
    FillRule as CoreFillRule, Filter, Isolation, Lang, ResolvedGradientStop,
  },
};

#[cfg(feature = "images")]
use crate::krilla::{geom::Size as KrillaSize, image::Image as KrillaImage};
#[cfg(feature = "images")]
use crate::paint::rasterized_image;
#[cfg(all(feature = "svg", feature = "images"))]
use crate::svg;
use crate::{
  filter::{ColorFilter, filtered, unsupported_filter},
  glyph::{ColorGlyphs, PdfGlyph, Uncovered, run_glyphs},
  inline::{InlineMap, visit_inline_layout},
  krilla::{
    Data,
    geom::{Path as KrillaPath, Point, Rect as KrillaRect, Transform},
    mask::{Mask, MaskType},
    num::NormalizedF32,
    paint::{
      Fill, FillRule, LineCap, LineJoin, LinearGradient as KrillaLinearGradient, Paint, Pattern,
      RadialGradient as KrillaRadialGradient, SpreadMethod, Stroke, StrokeDash, SweepGradient,
    },
    stream::Stream,
    surface::Surface,
    tagging::{ContentTag, SpanTag},
    text::{Font, Tag},
  },
  options::{PT_PER_PX, PdfError},
  paint::{
    core_transform, draw_stream, empty_path, expanded_radial_stops, fill_from_rgba, krilla_blend,
    krilla_fill_rule, krilla_path, krilla_stop, krilla_stops, krilla_transform, normalized,
    pop_transforms, rect_path, shape_path, spread,
  },
  shadow::Band,
  tags::{ARTIFACT, TagCollector},
  tree::draws,
  window::Window,
};
#[cfg(feature = "images")]
use takumi_core::{blur::blur_rgba, resources::glyph::ResolvedBitmapGlyph, style::BlurType};

/// Blob identity, collection index, and the variation coordinates the run was shaped at.
type FontKey = (u64, u32, Vec<([u8; 4], u32)>);

/// Krilla fonts embedded so far, one per distinct instance.
type FontMap = HashMap<FontKey, Font>;

pub(crate) struct Emitter<'a> {
  pub(crate) scene: &'a Scene,
  pub(crate) document: &'a DocumentState<'a>,
  /// Pre-built inline layouts for the content tree; band trees build on the fly.
  pub(crate) inline: Option<&'a InlineMap<'a>>,
  /// The page window this walk paints through.
  pub(crate) window: Window,
  /// Whether this walk records marked content for the structure tree.
  pub(crate) tagged: bool,
  /// Path from the document root to this emitter's own root.
  pub(crate) tag_prefix: Vec<usize>,
  /// Color transform from the `filter` properties of the enclosing stacking contexts, applied to
  /// every color this subtree paints.
  pub(crate) color_filter: Option<Rc<ColorFilter>>,
}

/// Failures a page collects while emitting, raised once the surface is closed.
struct RenderIssues {
  uncovered: Uncovered,
  /// The first failure worth stopping for.
  failure: Option<PdfError>,
}

/// What every page of one document shares while it is emitted.
pub(crate) struct DocumentState<'a> {
  fonts: RefCell<FontMap>,
  /// Present when the document is tagged.
  pub(crate) tags: Option<RefCell<TagCollector>>,
  /// What the pages could not draw.
  issues: RefCell<RenderIssues>,
  /// The document's default language.
  pub(crate) lang: Option<&'a str>,
}

impl<'a> DocumentState<'a> {
  pub(crate) fn new(tagged: bool, lang: Option<&'a str>, uncovered: Uncovered) -> Self {
    Self {
      fonts: RefCell::new(FontMap::default()),
      tags: tagged.then(RefCell::default),
      issues: RefCell::new(RenderIssues {
        uncovered,
        failure: None,
      }),
      lang,
    }
  }

  /// The error the pages left behind, if any.
  pub(crate) fn into_error(self) -> Option<PdfError> {
    let issues = self.issues.into_inner();

    issues.failure.or_else(|| issues.uncovered.into_error())
  }
}

impl Emitter<'_> {
  /// The filter chain as colors, keeping the first function a PDF cannot express.
  fn composed_filter(
    &self,
    outer: Option<&ColorFilter>,
    filters: &[Filter],
  ) -> Option<Rc<ColorFilter>> {
    if let Some(unsupported) = unsupported_filter(filters) {
      self.fail(PdfError::UnsupportedFilter(unsupported));
    }

    ColorFilter::compose(outer, filters).map(Rc::new)
  }

  /// The image to draw, or nothing and a kept failure.
  #[cfg(feature = "images")]
  fn drawable(
    &self,
    label: &str,
    image: Result<Option<KrillaImage>, String>,
  ) -> Option<KrillaImage> {
    match image {
      Ok(image) => image,
      Err(reason) => {
        self.fail(PdfError::UndrawableImage(format!("{label}: {reason}")));
        None
      }
    }
  }

  fn fail(&self, error: PdfError) {
    let mut issues = self.document.issues.borrow_mut();

    if issues.failure.is_none() {
      issues.failure = Some(error);
    }
  }

  /// The marked-content identifiers this walk records into, if it tags.
  fn tags(&self) -> Option<&RefCell<TagCollector>> {
    self.tagged.then_some(self.document.tags.as_ref()?)
  }

  /// The marked-content tag a node's own content opens.
  fn content_tag<'t>(&self, node: &'t RenderNode) -> ContentTag<'t> {
    match node.context.style.lang.as_ref().map(Lang::as_str) {
      Some(lang) if Some(lang) != self.document.lang => {
        ContentTag::Span(SpanTag::empty().with_lang(Some(lang)))
      }
      _ => ContentTag::Other,
    }
  }
}

impl Emitter<'_> {
  /// Emits the scene chunk by chunk, entering each chunk's clips and effects.
  pub(crate) fn emit(&mut self, surface: &mut Surface) -> Result<(), PdfError> {
    let scene = self.scene;
    let chunks = PaintChunk::in_paint_order(&scene.contexts);
    let owners = PaintChunk::effect_owners(&chunks, &scene.properties);
    let mut conversion = ConversionContext::new(
      &scene.properties,
      ChunkWriter {
        emitter: self,
        surface,
        chunks: &chunks,
        owners: &owners,
        current: Affine::IDENTITY,
        entries: Vec::new(),
        error: None,
      },
    );

    for chunk in &chunks {
      conversion.switch_to(chunk.state());
      conversion.sink().emit(chunk);
    }

    conversion.finish().error.map_or(Ok(()), Err)
  }

  /// `box-decoration-break: clone`: the fragment of the box on this page
  /// paints its own complete decorations (paint-only; cloned padding does not
  /// reserve layout space). `slice` needs nothing: the page window slices the
  /// full-box decorations, which is exactly the sliced rendering.
  fn decoration_frame(&self, style: &ComputedStyle, frame: BoxFrame) -> BoxFrame {
    let BoxFrame {
      layout,
      origin: CorePoint { x, y },
    } = frame;

    if style.box_decoration_break == BoxDecorationBreak::Clone
      && let Some((window_top, window_bottom)) = self.window.y
    {
      let top = y.max(window_top);
      let bottom = (y + layout.size.height).min(window_bottom);

      return BoxFrame::new(
        Layout {
          size: Size {
            width: layout.size.width,
            height: (bottom - top).max(0.0),
          },
          ..layout
        },
        CorePoint { x, y: top },
      );
    }

    frame
  }

  /// Pushes the box's mask and `clip-path`, returning how many states went on.
  /// The mask covers the element and its descendants; `clip-path` clips the
  /// element itself, decorations included, so both go on before any paint.
  fn push_mask_and_clip(
    &mut self,
    node: &RenderNode,
    frame: BoxFrame,
    surface: &mut Surface,
  ) -> usize {
    let BoxFrame { layout, .. } = frame;
    let mut pushed = 0;

    if let Some(mask) = self.mask(node, frame, surface) {
      surface.push_mask(mask);
      pushed += 1;
    }

    if let Some(shape) = BoxPainter::new(&node.context, layout).clip_path() {
      // A shape that resolves to no area clips everything away, so a missing
      // path becomes an empty region rather than no clip at all.
      let path = shape_path(&shape, frame.origin).or_else(|| empty_path(frame.origin));

      if let Some(path) = path {
        surface.push_clip_path(&path, &krilla_fill_rule(shape.rule()));
        pushed += 1;
      }
    }

    pushed
  }

  /// Paints shadows, backgrounds, and borders in CSS order.
  /// `background-clip` picks the shape a background fills, never when it
  /// paints: the border draws over the ring, as it does in Blink.
  fn emit_decorations(
    &self,
    node: &RenderNode,
    frame: BoxFrame,
    text_clip: Option<&TextClip>,
    surface: &mut Surface,
  ) {
    if !node.paints_own_box() {
      return;
    }

    let painter = BoxPainter::new(&node.context, frame.layout);

    painter.paint_normal_box_shadows(frame.origin, &mut self.device(surface, self.tagged));
    painter.background_color(frame.origin, &mut self.device(surface, self.tagged));
    self.emit_background_layers(node, &painter.background(), frame, surface);
    if let Some(text_clip) = text_clip {
      let painted = text_clip.paint_background(
        frame.origin,
        &mut TextDevice {
          emitter: self,
          device: self.device(surface, self.tagged),
          built: None,
          shadow: None,
          through: None,
        },
      );

      if let Err(error) = painted {
        self.fail(error.into());
      }
    }
    painter.paint_inset_box_shadows(frame.origin, &mut self.device(surface, self.tagged));
    painter.paint_border(frame.origin, &mut self.device(surface, self.tagged));
  }

  /// Emits the box's own content inside its structure tag when tagging is on.
  fn emit_tagged_content(
    &mut self,
    node: &RenderNode,
    paint: &NodePaint,
    frame: BoxFrame,
    pass: InlinePass,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let tagged = self.tagged && pass == InlinePass::Content && draws(&OwnContent::of(node));

    if tagged {
      self.start_node_region(node, Some(&paint.path), surface);
    }
    self.emit_own_content(node, paint.node_id, frame, pass, surface)?;
    if tagged {
      surface.end_tagged();
    }

    Ok(())
  }

  /// Paints `background-image` layers, bottom layer first, clipped to the
  /// `background-clip` box. Gradient layers paint as shadings; `url()` layers
  /// rasterize when the `images` feature is on. `background-origin` sets the
  /// positioning area the size and position resolve against; a repeating
  /// layer still tiles across the whole clip region.
  fn emit_background_layers(
    &self,
    node: &RenderNode,
    background: &BoxBackground<'_>,
    frame: BoxFrame,
    surface: &mut Surface,
  ) {
    if background.layers.is_empty() {
      return;
    }
    let Some(shape) = background.clip.shape(background.size) else {
      return;
    };
    let mask = background.clip.border_mask();
    let origin = frame.origin + background.offset;
    let clip = shape_path(&shape, origin);

    if mask.is_none() && clip.is_none() {
      return;
    }

    self.in_artifact(surface, |surface| {
      match (mask, &clip) {
        (Some(border), _) => {
          let stream = border_mask_stream(&border, background.size, origin, surface);

          surface.push_mask(Mask::new(stream, MaskType::Alpha));
        }
        (None, Some(clip)) => surface.push_clip_path(clip, &krilla_fill_rule(shape.rule())),
        (None, None) => return,
      }
      self.paint_background_layers(node, background, frame, surface);
      surface.pop();
    });
  }

  /// Draws one layer of a box whose border box sits at `origin`: a lone tile on its own, or
  /// one tile in a pattern filling the layer's `dest`, so a repeated layer costs one shading
  /// instead of one per tile. `tile_space` is the space the pattern's tile draws in.
  fn layer(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    tiling: &ImageTiling,
    origin: CorePoint<f32>,
    surface: &mut Surface,
    tile_space: Transform,
  ) {
    let dest = tiling.dest;
    let Some(dest_path) = KrillaRect::from_ltrb(
      origin.x + dest.left,
      origin.y + dest.top,
      origin.x + dest.right,
      origin.y + dest.bottom,
    )
    .and_then(rect_path) else {
      return;
    };
    let (xs, ys) = tiling.origins();

    if let ([x], [y]) = (xs.as_slice(), ys.as_slice()) {
      let clipped = !tiling.covers(CoreRect {
        left: *x,
        top: *y,
        right: x + tiling.tile.width,
        bottom: y + tiling.tile.height,
      });

      if clipped {
        surface.push_clip_path(&dest_path, &FillRule::NonZero);
      }
      self.background_layer(
        image,
        node,
        tiling.tile,
        origin + CorePoint { x: *x, y: *y },
        surface,
        Transform::identity(),
      );
      if clipped {
        surface.pop();
      }
      return;
    }

    let stream = draw_stream(surface, |tile| {
      self.background_layer(image, node, tiling.tile, CorePoint::ZERO, tile, tile_space);
    });
    let step = tiling.step();

    surface.set_fill(Some(Fill {
      paint: Pattern {
        stream,
        transform: Transform::from_translate(origin.x + tiling.phase.x, origin.y + tiling.phase.y),
        width: step.width,
        height: step.height,
      }
      .into(),
      opacity: NormalizedF32::ONE,
      rule: FillRule::NonZero,
    }));
    surface.draw_path(&dest_path);
  }

  fn background_layer(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    size: Size<f32>,
    at: CorePoint<f32>,
    surface: &mut Surface,
    pattern_space: Transform,
  ) {
    let (w, h) = (size.width, size.height);

    // A url() layer draws as an image tile; the transform applies to pixels,
    // so it goes through the same rasterization as a filtered <img>.
    #[cfg(feature = "images")]
    if let BackgroundImage::Url(url) = image {
      let Ok(source) = resolve_image(url, &node.context) else {
        return;
      };
      let Some(krilla_image) = self.drawable(
        url,
        rasterized_image(&source, &node.context, size, self.color_filter.as_deref()),
      ) else {
        return;
      };
      let Some(target) = KrillaSize::from_wh(w, h) else {
        return;
      };

      surface.push_transform(&Transform::from_translate(at.x, at.y));
      surface.draw_image(krilla_image, target);
      surface.pop();
      return;
    }
    let Some(paint) = self.gradient_paint(image, node, size, at, pattern_space) else {
      return;
    };
    let Some(path) = KrillaRect::from_xywh(at.x, at.y, w, h).and_then(rect_path) else {
      return;
    };

    surface.set_fill(Some(Fill {
      paint,
      opacity: NormalizedF32::ONE,
      rule: FillRule::NonZero,
    }));
    surface.draw_path(&path);
  }

  /// The krilla paint of one gradient layer, its geometry anchored at `(x, y)`
  /// with `size` as the tile. `None` for layers that are not gradients.
  ///
  /// PDF 32000-1 8.7.3.1 resolves a pattern matrix against the default space of the stream the
  /// pattern is used in, which a nested stream does not inherit; `pattern_space` carries what
  /// krilla no longer composes.
  fn gradient_paint(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    size: Size<f32>,
    at: CorePoint<f32>,
    pattern_space: Transform,
  ) -> Option<Paint> {
    let CorePoint { x, y } = at;
    let (w, h) = (size.width, size.height);
    let sizing = &node.context.sizing;
    let current_color = node.context.current_color;

    let paint: Paint = match image {
      BackgroundImage::Linear(gradient) => {
        let mut geometry = gradient.resolve_geometry(w, h, sizing, current_color);
        let axis_length = geometry.axis_length;
        let (dir_x, dir_y) = (geometry.dir_x, geometry.dir_y);
        self.filter_stops(geometry.stops_mut());
        let resolved = geometry.stops();
        if resolved.is_empty() {
          return None;
        }
        let max_extent = axis_length / 2.0;
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let point_at = |t: f32| (cx + (t - max_extent) * dir_x, cy + (t - max_extent) * dir_y);
        let (t0, t1, base, span) = if gradient.repeating {
          let first = resolved.first().map_or(0.0, |s| s.position);
          let last = resolved.last().map_or(axis_length, |s| s.position);
          (first, last, first, (last - first).max(1e-6))
        } else {
          (0.0, axis_length, 0.0, axis_length.max(1e-6))
        };
        let (x1, y1) = point_at(t0);
        let (x2, y2) = point_at(t1);

        KrillaLinearGradient {
          x1,
          y1,
          x2,
          y2,
          transform: pattern_space,
          spread_method: spread(gradient.repeating),
          stops: krilla_stops(resolved, base, span),
          anti_alias: false,
        }
        .into()
      }
      BackgroundImage::Radial(gradient) => {
        let mut geometry = gradient.resolve_geometry(w, h, sizing, current_color);
        let (cx, cy) = (geometry.cx, geometry.cy);
        let radius_x = geometry.inv_radius_x.max(1e-6).recip();
        let radius_y = geometry.inv_radius_y.max(1e-6).recip();
        let extent = geometry.radius_scale.max(1e-6);
        self.filter_stops(geometry.stops_mut());
        let resolved = geometry.stops();
        if resolved.is_empty() {
          return None;
        }
        // PDF radial shadings cannot repeat, so a repeating gradient expands
        // its period across the full radius instead of relying on the spread.
        let stops = if gradient.repeating {
          expanded_radial_stops(resolved, extent)
        } else {
          krilla_stops(resolved, 0.0, extent)
        };
        let scale_x = (radius_x / extent).max(1e-6);
        let scale_y = (radius_y / extent).max(1e-6);

        KrillaRadialGradient {
          fx: 0.0,
          fy: 0.0,
          fr: 0.0,
          cx: 0.0,
          cy: 0.0,
          cr: extent,
          transform: pattern_space.pre_concat(Transform::from_row(
            scale_x,
            0.0,
            0.0,
            scale_y,
            x + cx,
            y + cy,
          )),
          spread_method: SpreadMethod::Pad,
          stops,
          anti_alias: false,
        }
        .into()
      }
      BackgroundImage::Conic(gradient) => {
        let tile =
          ConicGradientTile::new(gradient, w as u32, h as u32, sizing, current_color, false);
        let lut_len = tile.lut.len();
        if lut_len == 0 {
          return None;
        }
        const SWEEP_STOPS: usize = 64;
        let stops = (0..=SWEEP_STOPS)
          .map(|i| {
            let t = i as f32 / SWEEP_STOPS as f32;
            let index =
              tile.lut_index_for_adjusted_angle_with_len(t * core::f32::consts::TAU, lut_len);
            let color = tile.lut.sample(index).demultiply();

            krilla_stop(
              t,
              self.filtered(Color([
                color.red(),
                color.green(),
                color.blue(),
                color.alpha(),
              ])),
            )
          })
          .collect();
        let (ccx, ccy) = (x + tile.cx, y + tile.cy);

        SweepGradient {
          cx: ccx,
          cy: ccy,
          start_angle: 0.0,
          end_angle: 360.0,
          transform: pattern_space.pre_concat(Transform::from_rotate_at(
            tile.start_rad.to_degrees() - 90.0,
            ccx,
            ccy,
          )),
          spread_method: SpreadMethod::Pad,
          stops,
          anti_alias: false,
        }
        .into()
      }
      BackgroundImage::Url(_) | BackgroundImage::None => return None,
    };

    Some(paint)
  }

  /// Builds the soft mask for `mask-image`, drawing its layers into their own stream.
  fn mask(&mut self, node: &RenderNode, frame: BoxFrame, surface: &mut Surface) -> Option<Mask> {
    let size = frame.layout.size;
    let images = node.context.style.mask_image.as_deref()?;

    if !images.iter().any(BackgroundImage::paints) {
      return None;
    }
    let filter = self.color_filter.take();
    let layers = FillLayers::mask(&node.context.style).resolve(
      images,
      &BoxBackgroundPaintContext::mask(size, node.context.box_paint_offset(frame.layout)),
      &node.context,
    );
    let stream = draw_stream(surface, |content| {
      for layer in &layers {
        self.layer(
          layer.image,
          node,
          &layer.tiling,
          frame.origin,
          content,
          Transform::identity(),
        );
      }
    });

    self.color_filter = filter;
    Some(Mask::new(stream, MaskType::Alpha))
  }

  /// A color as this subtree's `filter` leaves it.
  fn filtered(&self, color: Color) -> [u8; 4] {
    filtered(self.color_filter.as_deref(), color)
  }

  /// Gradient stops as this subtree's `filter` leaves them.
  fn filter_stops(&self, resolved: &mut [ResolvedGradientStop]) {
    if let Some(filter) = &self.color_filter {
      for stop in resolved {
        stop.color = filter.apply_color(stop.color);
      }
    }
  }

  /// Draws a decoration inside an artifact sequence when tagging is on, so it
  /// stays out of the structure tree.
  /// Paints `background`'s layers, `node`'s laid over `frame`, unclipped.
  fn paint_background_layers(
    &self,
    node: &RenderNode,
    background: &BoxBackground<'_>,
    frame: BoxFrame,
    surface: &mut Surface,
  ) {
    for layer in &background.layers {
      let blended = layer.blend_mode != BlendMode::Normal;

      if blended {
        surface.push_blend_mode(krilla_blend(layer.blend_mode));
      }
      self.layer(
        layer.image,
        node,
        &layer.tiling,
        frame.origin,
        surface,
        Transform::from_scale(PT_PER_PX, PT_PER_PX),
      );
      if blended {
        surface.pop();
      }
    }
  }

  fn in_artifact(&self, surface: &mut Surface, draw: impl FnOnce(&mut Surface)) {
    if self.tagged {
      surface.start_tagged(ARTIFACT);
    }
    draw(surface);
    if self.tagged {
      surface.end_tagged();
    }
  }

  /// The PDF surface as a [`PaintDevice`] in this subtree's colors.
  fn device<'s, 'a>(
    &'s self,
    surface: &'s mut Surface<'a>,
    artifact: bool,
  ) -> SurfaceDevice<'s, 'a> {
    SurfaceDevice {
      surface,
      filter: self.color_filter.as_deref(),
      artifact,
      stack: Vec::new(),
    }
  }

  fn paint_outline(&self, pending: Option<&PendingOutline>, surface: &mut Surface) {
    if let Some(pending) = pending {
      pending.paint(&mut self.device(surface, self.tagged));
    }
  }

  fn emit_own_content(
    &mut self,
    node: &RenderNode,
    node_id: NodeId,
    frame: BoxFrame,
    pass: InlinePass,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    match OwnContent::of(node) {
      OwnContent::Inline(_) => self.emit_node_text(node, node_id, frame, pass, surface),
      #[cfg(feature = "images")]
      OwnContent::Image(image) if pass == InlinePass::Content => {
        self.emit_image(image, &node.context, frame, surface);
        Ok(())
      }
      _ => Ok(()),
    }
  }

  #[cfg(feature = "images")]
  /// Draws an image node into its content box, honoring `object-fit` and
  /// `object-position`. SVG sources draw as vector ops; everything else
  /// rasterizes at its intrinsic size and embeds once per distinct pixel data
  /// (krilla dedups by content hash).
  fn emit_image(
    &self,
    image: &ImageData,
    context: &RenderContext,
    frame: BoxFrame,
    surface: &mut Surface,
  ) {
    let BoxFrame {
      layout,
      origin: CorePoint { x, y },
    } = frame;
    let content = layout.content_box_size();
    let offset = layout.content_box_offset();
    let (bx, by, w, h) = (x + offset.x, y + offset.y, content.width, content.height);
    if w <= 0.0 || h <= 0.0 {
      return;
    }
    let Ok(source) = image.src.resolve(context) else {
      return;
    };
    let (iw, ih) = source.size(&context.sizing);

    if iw <= 0.0 || ih <= 0.0 {
      return;
    }
    let replaced = BoxPainter::new(context, layout).replaced_content(Size {
      width: iw,
      height: ih,
    });
    let placement = replaced.placement;
    let (dw, dh) = (placement.size.width, placement.size.height);
    // SVG sources embed as vector ops; everything else rasterizes. A color
    // filter rasterizes them too, since the transform applies to pixels.
    #[cfg(feature = "svg")]
    let vector = if let (ImageSource::Svg(svg), None) = (&source, &self.color_filter) {
      let (svg_width, svg_height) = svg.dimensions();
      if svg_width <= 0.0 || svg_height <= 0.0 {
        return;
      }
      // Fallback rasters (filters, embedded bitmaps) keep the old 2x density.
      let raster_scale = 2.0 * (dw / svg_width).max(dh / svg_height);

      Some((
        svg.vector_ops(raster_scale, context.current_color, Some(context.fonts())),
        svg_width,
        svg_height,
      ))
    } else {
      None
    };
    #[cfg(not(feature = "svg"))]
    let vector: Option<((), f32, f32)> = None;

    let krilla_image = if vector.is_none() {
      let Some(image) = self.drawable(
        image_label(&image.src),
        rasterized_image(
          &source,
          context,
          placement.size,
          self.color_filter.as_deref(),
        ),
      ) else {
        return;
      };

      Some(image)
    } else {
      None
    };
    let ix = bx + placement.offset.x;
    let iy = by + placement.offset.y;

    let Some(size) = KrillaSize::from_wh(dw, dh) else {
      return;
    };
    let clip_path = replaced.clip.and_then(|clip| {
      if clip.border.is_zero() {
        KrillaRect::from_xywh(bx, by, w, h).and_then(rect_path)
      } else {
        shape_path(&clip.into(), frame.origin)
      }
    });

    if let Some(path) = &clip_path {
      surface.push_clip_path(path, &FillRule::NonZero);
    }
    #[cfg(feature = "svg")]
    if let Some((ops, svg_width, svg_height)) = vector {
      let canvas = KrillaRect::from_xywh(0.0, 0.0, svg_width, svg_height).and_then(rect_path);

      surface.push_transform(&Transform::from_row(
        dw / svg_width,
        0.0,
        0.0,
        dh / svg_height,
        ix,
        iy,
      ));
      if let Some(canvas) = &canvas {
        surface.push_clip_path(canvas, &FillRule::NonZero);
      }
      svg::draw_svg_ops(surface, ops);
      if canvas.is_some() {
        surface.pop();
      }
      surface.pop();
    }
    if let Some(krilla_image) = krilla_image {
      surface.push_transform(&Transform::from_translate(ix, iy));
      surface.draw_image(krilla_image, size);
      surface.pop();
    }
    if clip_path.is_some() {
      surface.pop();
    }
  }

  /// Draws a text-bearing box's runs, from the pre-built inline map when the node is in it (content
  /// tree) or built on the fly (band trees).
  fn emit_node_text(
    &mut self,
    node: &RenderNode,
    node_id: NodeId,
    frame: BoxFrame,
    pass: InlinePass,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    visit_inline_layout(
      self.inline,
      node,
      node_id,
      frame.layout,
      |built, runs, font_style| match pass {
        InlinePass::Content => self.draw_runs(node, runs, built, frame, font_style, surface),
        InlinePass::Floats => self.emit_inline_boxes(node, runs, built, frame, pass, surface),
      },
    )?;
    Ok(())
  }

  /// Paints the runs on the lines this page owns through takumi-core's text painter, then the
  /// inline boxes.
  #[allow(clippy::too_many_arguments)]
  fn draw_runs(
    &mut self,
    node: &RenderNode,
    runs: &InlineRunLayout,
    built: &BuiltInlineLayout<'_>,
    frame: BoxFrame,
    font_style: &SizedFontStyle,
    surface: &mut Surface,
  ) {
    let y = frame.origin.y;
    let lines = runs.lines(frame.layout, |item| {
      self.window.shows_line_item(item.below(y))
    });
    let mut device = TextDevice {
      emitter: self,
      device: self.device(surface, false),
      built: Some(built),
      shadow: None,
      through: None,
    };

    lines.paint(&built.spans, font_style, frame, &mut device);
    self.emit_inline_boxes(node, runs, built, frame, InlinePass::Content, surface);
  }

  /// Paints the inline layout's replaced boxes and nested container subtrees.
  #[allow(clippy::too_many_arguments)]
  fn emit_inline_boxes(
    &mut self,
    owner: &RenderNode,
    runs: &InlineRunLayout,
    built: &BuiltInlineLayout<'_>,
    frame: BoxFrame,
    pass: InlinePass,
    surface: &mut Surface,
  ) {
    let BoxFrame {
      layout,
      origin: CorePoint { y, .. },
    } = frame;

    // The caller opened a marked-content region for the text around these
    // boxes. Marked content does not nest, so each box closes it, takes a
    // region of its own, and hands it back.
    let owner_tagged = self.tagged && pass == InlinePass::Content && draws(&OwnContent::of(owner));

    for positioned in runs
      .inline_boxes
      .iter()
      .filter(|positioned| pass.paints(positioned))
    {
      let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(positioned.id as usize) else {
        continue;
      };
      let node = item.render_node;

      // An in-flow box belongs to the page that owns its line, like the glyph
      // runs beside it.
      if positioned.line_baseline.is_some_and(|baseline| {
        let content_y = y + layout.content_box_offset().y;
        let top = content_y + positioned.y;

        !self.window.shows_line_item(LineItem {
          baseline: content_y + baseline,
          top,
          bottom: top + positioned.height,
        })
      }) {
        continue;
      }
      let Some((offset, paint)) = resolve_inline_box(positioned, item, layout) else {
        continue;
      };
      let marker_target = (self.tagged && node.origin == NodeOrigin::Marker)
        .then(|| self.marker_tag_target(owner))
        .flatten();
      let marker_tagged = marker_target.is_some();

      // The box never reaches `emit_box`, so the state that would paint it
      // there is applied here: its own opacity, and its `filter` composed onto
      // the one the enclosing stacking contexts left.
      let opacity = node.context.style.opacity.0;
      let faded = opacity < 1.0;
      // A container box runs its own emitter, which tags every node it walks.
      // A replaced one paints here. Its decorations are artifacts and marked
      // content cannot nest within one stream, so the box's region wraps the
      // whole box only when opacity moves it into a group of its own, and
      // wraps just the content otherwise.
      let box_tagged = cfg!(feature = "images")
        && self.tagged
        && !marker_tagged
        && matches!(paint, InlineBoxPaint::Replaced { .. });
      let box_wrapped = box_tagged && faded;

      if owner_tagged {
        surface.end_tagged();
      }
      if let Some(path) = marker_target.as_ref() {
        let identifier = surface.start_tagged(self.content_tag(node));

        if let Some(tags) = self.tags() {
          tags.borrow_mut().record_label(path, identifier);
        }
      } else if box_wrapped {
        self.start_tagged_node(node, surface);
      }

      if faded {
        surface.push_opacity(normalized(opacity));
      }
      let outer_filter = self.color_filter.clone();
      self.color_filter = self.composed_filter(outer_filter.as_deref(), &node.context.style.filter);

      let origin = frame.origin + offset;

      match paint {
        #[cfg(feature = "images")]
        InlineBoxPaint::Replaced {
          node,
          layout: box_layout,
        } => self.emit_inline_replaced(
          node,
          BoxFrame::new(box_layout, origin),
          box_tagged && !box_wrapped,
          surface,
        ),
        #[cfg(not(feature = "images"))]
        InlineBoxPaint::Replaced { .. } => {}
        InlineBoxPaint::Container(subtree) => {
          self.emit_inline_subtree(subtree, node, origin, surface)
        }
      }
      self.color_filter = outer_filter;
      if faded {
        surface.pop();
      }
      if marker_tagged || box_wrapped {
        surface.end_tagged();
      }
      if owner_tagged {
        self.start_tagged_node(owner, surface);
      }
    }
  }

  /// Paints a replaced inline box: its decorations, then its content, which
  /// `tagged` wraps in the box's own region.
  #[cfg(feature = "images")]
  fn emit_inline_replaced(
    &mut self,
    node: &RenderNode,
    frame: BoxFrame,
    tagged: bool,
    surface: &mut Surface,
  ) {
    self.emit_decorations(node, frame, None, surface);
    if tagged {
      self.start_tagged_node(node, surface);
    }
    if let Some(NodeKind::Image(image)) = node.node.as_ref().map(|source| &source.kind) {
      self.emit_image(image, &node.context, frame, surface);
    }
    if tagged {
      surface.end_tagged();
    }
    self.paint_outline(
      BoxPainter::new(&node.context, frame.layout)
        .pending_outline(frame.origin)
        .as_ref(),
      surface,
    );
  }

  /// Paints an inline-level container from the scene it carries.
  fn emit_inline_subtree(
    &mut self,
    subtree: Box<InlineSubtree>,
    node: &RenderNode,
    origin: CorePoint<f32>,
    surface: &mut Surface,
  ) {
    let at = subtree.border_box_origin(origin);
    let Ok(scene) = subtree.into_scene(Affine::IDENTITY, true) else {
      return;
    };
    // The subtree root is a clone of `node`, so the box's own path is the
    // prefix that puts the subtree's nodes back on the document tree.
    let box_path = self.tagged.then(|| self.path_of(node)).flatten();
    let tagged = box_path.is_some();
    let tag_prefix = self.tag_path(box_path.as_deref().unwrap_or_default());
    let mut emitter = Emitter {
      scene: &scene,
      document: self.document,
      inline: None,
      window: Window::default(),
      tagged,
      tag_prefix,
      color_filter: self.color_filter.clone(),
    };
    surface.push_transform(&Transform::from_translate(at.x, at.y));
    let _ = emitter.emit(surface);
    surface.pop();
  }

  /// Opens the marked-content region a node's own content draws in: an
  /// artifact for a decorative image, otherwise a region recorded at `path`.
  fn start_node_region(&self, node: &RenderNode, path: Option<&[usize]>, surface: &mut Surface) {
    if decorative_image(node) {
      surface.start_tagged(ARTIFACT);
      return;
    }
    let identifier = surface.start_tagged(self.content_tag(node));

    if let Some(tags) = self.tags()
      && let Some(path) = path
    {
      tags.borrow_mut().record(&self.tag_path(path), identifier);
    }
  }

  /// Opens the region for a node the paint list never visited, so its content
  /// still reaches the structure tree.
  fn start_tagged_node(&self, node: &RenderNode, surface: &mut Surface) {
    self.start_node_region(node, self.path_of(node).as_deref(), surface);
  }

  /// The path of `node` on this emitter's tree, when it lies there.
  fn path_of(&self, node: &RenderNode) -> Option<Vec<usize>> {
    self
      .scene
      .root
      .path_where(|candidate| ptr::eq(candidate, node))
  }

  /// The document-rooted path of a node this emitter reached at `path`.
  fn tag_path(&self, path: &[usize]) -> Vec<usize> {
    if self.tag_prefix.is_empty() {
      return path.to_vec();
    }
    let mut full = self.tag_prefix.clone();

    full.extend_from_slice(path);
    full
  }

  /// Tag target for a generated marker: its nearest `display: list-item` ancestor, whose `Lbl`
  /// holds the label.
  fn marker_tag_target(&self, owner: &RenderNode) -> Option<Vec<usize>> {
    let owner_path = self.path_of(owner)?;
    let mut current = &self.scene.root;
    let mut length = owner_path.len();

    for (depth, index) in owner_path.iter().enumerate() {
      if current.context.style.display == Display::ListItem {
        length = depth;
      }
      current = current.children.as_deref()?.get(*index)?;
    }
    if current.context.style.display == Display::ListItem {
      length = owner_path.len();
    }

    Some(self.tag_path(&owner_path[..length]))
  }

  /// One layer as a pattern glyphs can be filled with, over a box whose border box sits at
  /// `origin`. An axis with one tile repeats it farther apart than the layer's `dest` reaches.
  fn layer_pattern(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    tiling: &ImageTiling,
    origin: CorePoint<f32>,
    surface: &mut Surface,
  ) -> Option<Paint> {
    let (xs, ys) = tiling.origins();
    let (first_x, first_y) = (*xs.first()?, *ys.first()?);
    let step = tiling.step();
    let dest = tiling.dest;
    let lone =
      |start: f32, end: f32, tile: f32, first: f32| end - start + tile + (first - start).abs();
    let width = if xs.len() > 1 {
      step.width
    } else {
      lone(dest.left, dest.right, tiling.tile.width, first_x)
    };
    let height = if ys.len() > 1 {
      step.height
    } else {
      lone(dest.top, dest.bottom, tiling.tile.height, first_y)
    };
    let stream = draw_stream(surface, |inner| {
      self.background_layer(
        image,
        node,
        tiling.tile,
        CorePoint::ZERO,
        inner,
        Transform::from_scale(PT_PER_PX, PT_PER_PX),
      );
    });

    Some(
      Pattern {
        stream,
        transform: Transform::from_translate(origin.x + first_x, origin.y + first_y),
        width,
        height,
      }
      .into(),
    )
  }

  /// The paints `background`, `node`'s laid over `frame`, fills glyphs with: its colour, then
  /// each layer.
  fn background_fills(
    &self,
    node: &RenderNode,
    background: &BoxBackground<'_>,
    frame: BoxFrame,
    surface: &mut Surface,
  ) -> Vec<GlyphBackground> {
    let mut fills = Vec::new();
    let rect_clip = |left: f32, top: f32, right: f32, bottom: f32| {
      KrillaRect::from_ltrb(
        frame.origin.x + left,
        frame.origin.y + top,
        frame.origin.x + right,
        frame.origin.y + bottom,
      )
      .and_then(rect_path)
    };

    if let Some(color) = background.color {
      let offset = background.offset;

      fills.push(GlyphBackground {
        fill: fill_from_rgba(self.filtered(color), 1.0),
        clip: rect_clip(
          offset.x,
          offset.y,
          offset.x + background.size.width,
          offset.y + background.size.height,
        ),
      });
    }

    for layer in &background.layers {
      let tiling = &layer.tiling;
      let Some(paint) = self.layer_pattern(layer.image, node, tiling, frame.origin, surface) else {
        continue;
      };
      let dest = tiling.dest;
      let clip = rect_clip(dest.left, dest.top, dest.right, dest.bottom);

      fills.push(GlyphBackground {
        fill: Fill {
          paint,
          opacity: NormalizedF32::ONE,
          rule: FillRule::NonZero,
        },
        clip,
      });
    }
    fills
  }

  /// A run this page draws at `(x, y)`, its text from `built` when given, or `None` when it has
  /// no glyphs, no font, or its line at `line_y` belongs to another page.
  fn glyph_run<'r>(
    &self,
    run: &PositionedInlineRun,
    built: Option<&'r BuiltInlineLayout<'_>>,
    frame: BoxFrame,
    line_y: f32,
  ) -> Option<GlyphRun<'r>> {
    let BoxFrame {
      layout,
      origin: CorePoint { x, y },
    } = frame;
    let shaped = &run.glyph_run;

    // A zero-sized run shows nothing, and its glyph positions divide by the size.
    if shaped.glyphs.is_empty() || shaped.font_size == 0.0 {
      return None;
    }
    let font = self.cached_font(shaped)?;
    let offset = run.glyph_offset(layout);
    let item = LineItem::of_run(run, layout)?.below(line_y);

    if self.window.disowns_line(item.baseline) {
      return None;
    }
    let text = built
      .and_then(|built| built.text.get(shaped.text_range.clone()))
      .unwrap_or_default();
    let glyphs = run_glyphs(
      shaped,
      text,
      &mut self.document.issues.borrow_mut().uncovered,
    );

    Some(GlyphRun {
      font,
      text,
      glyphs,
      origin: Point::from_xy(x + offset.x, y + offset.y),
    })
  }

  /// Shears the text about its baseline, the faux oblique the raster renderer applies to glyph
  /// outlines.
  fn push_oblique(&self, shaped: &ShapedRun, origin: Point, surface: &mut Surface) -> bool {
    let Some(degrees) = shaped.synthetic_skew else {
      return false;
    };
    let tangent = degrees.to_radians().tan();

    surface.push_transform(&Transform::from_row(
      1.0,
      0.0,
      -tangent,
      1.0,
      tangent * origin.y,
      0.0,
    ));
    true
  }

  /// A krilla font for a run's backing blob, instanced at the run's variation
  /// coordinates. Copies the blob into the cache once per distinct instance.
  fn cached_font(&self, shaped: &ShapedRun) -> Option<Font> {
    let key = (
      shaped.font_id(),
      shaped.font_index,
      shaped
        .variations
        .iter()
        .map(|(axis, value)| (*axis, value.to_bits()))
        .collect(),
    );

    if let Some(font) = self.document.fonts.borrow().get(&key) {
      return Some(font.clone());
    }
    let variations: Vec<(Tag, f32)> = shaped
      .variations
      .iter()
      .map(|(axis, value)| (Tag::new(axis), *value))
      .collect();
    let font = Font::new_variable(
      Data::from(shaped.font_data().to_vec()),
      shaped.font_index,
      &variations,
    )?;

    self.document.fonts.borrow_mut().insert(key, font.clone());
    Some(font)
  }
}

/// What entering a clip or an effect replaced, restored when it is left.
struct Entered {
  /// Surface states it pushed.
  pushed: usize,
  current: Affine,
  window: Window,
  color_filter: Option<Rc<ColorFilter>>,
}

/// A [`PropertySink`] writing chunks onto a krilla surface.
struct ChunkWriter<'w, 'a, 's> {
  emitter: &'w mut Emitter<'a>,
  surface: &'w mut Surface<'s>,
  chunks: &'w [PaintChunk<'a>],
  owners: &'w [Option<&'a NodePaint>],
  /// The transform the pushed surface states add to the scene's space.
  current: Affine,
  entries: Vec<Entered>,
  error: Option<PdfError>,
}

impl<'a> ChunkWriter<'_, 'a, '_> {
  /// The box `paint` names and where it sits relative to the pushed transforms, pushing a
  /// transform when it needs more than a translation. Returns how many states went on.
  fn place(&mut self, paint: &NodePaint) -> Option<(&'a RenderNode, BoxFrame, usize)> {
    let scene = self.emitter.scene;
    let node = scene.root.node_at_path(&paint.path)?;
    let layout = scene.results.layout(paint.node_id).ok()?;
    let relative = self.current.invert().unwrap_or(Affine::IDENTITY) * paint.transform;

    if relative.only_translation() {
      let origin = CorePoint {
        x: relative.x,
        y: relative.y,
      };

      return Some((node, BoxFrame::new(layout, origin), 0));
    }

    self
      .surface
      .push_transform(&krilla_transform(relative.to_cols_array()));
    Some((node, BoxFrame::new(layout, CorePoint::ZERO), 1))
  }

  /// Writes one chunk under the states already pushed.
  fn emit(&mut self, chunk: &PaintChunk<'a>) {
    // Skipping a chunk that paints outside the window only saves work; the page's own clip would
    // drop it anyway.
    if self.error.is_some() || self.emitter.window.excludes_bounds(chunk.node.paint_bounds) {
      return;
    }

    let Some((node, frame, pushed)) = self.place(chunk.node) else {
      return;
    };
    let emitter = &mut *self.emitter;
    let surface = &mut *self.surface;
    let decoration_frame = emitter.decoration_frame(&node.context.style, frame);
    let result = match chunk.part {
      ChunkPart::Decorations => {
        let scene = emitter.scene;

        TextClip::of(&scene.root, &scene.results, self.chunks, chunk.node)
          .map(|text_clip| {
            emitter.emit_decorations(node, decoration_frame, text_clip.as_ref(), surface);
          })
          .map_err(PdfError::from)
      }
      ChunkPart::Content => {
        emitter.emit_tagged_content(node, chunk.node, frame, InlinePass::Content, surface)
      }
      ChunkPart::Floats => {
        emitter.emit_tagged_content(node, chunk.node, frame, InlinePass::Floats, surface)
      }
      ChunkPart::Outline => {
        let outline = BoxPainter::new(&node.context, decoration_frame.layout)
          .pending_outline(decoration_frame.origin);

        emitter.paint_outline(outline.as_ref(), surface);
        Ok(())
      }
    };

    pop_transforms(surface, pushed);

    if let Err(error) = result {
      self.error.get_or_insert(error);
    }
  }

  /// Records what entering pushed and what it replaced.
  fn enter(
    &mut self,
    pushed: usize,
    current: Affine,
    window: Window,
    color_filter: Option<Rc<ColorFilter>>,
  ) {
    self.entries.push(Entered {
      pushed,
      current,
      window,
      color_filter,
    });
  }

  fn leave(&mut self) {
    let Some(entered) = self.entries.pop() else {
      return;
    };

    pop_transforms(self.surface, entered.pushed);
    self.current = entered.current;
    self.emitter.window = entered.window;
    self.emitter.color_filter = entered.color_filter;
  }
}

impl PropertySink for ChunkWriter<'_, '_, '_> {
  fn push_clip(&mut self, _id: ClipId, clip: &ClipNode) {
    let (current, window, color_filter) = (
      self.current,
      self.emitter.window,
      self.emitter.color_filter.clone(),
    );
    let relative = current.invert().unwrap_or(Affine::IDENTITY) * clip.transform;
    let mut pushed = 0;
    let origin = if relative.only_translation() {
      CorePoint {
        x: relative.x,
        y: relative.y,
      }
    } else {
      self
        .surface
        .push_transform(&krilla_transform(relative.to_cols_array()));
      pushed += 1;
      self.current = clip.transform;
      CorePoint::ZERO
    };

    // A clip keeps content off the page but not out of the text layer, so what it cuts away must
    // never be emitted; only a clip in page space maps onto the window's axis.
    if current == Affine::IDENTITY
      && relative.only_translation()
      && let Some((top, bottom)) = vertical_extent(&clip.shape)
    {
      self
        .emitter
        .window
        .narrow(origin.y + top, origin.y + bottom);
    }
    if let Some(path) = shape_path(&clip.shape, origin) {
      self
        .surface
        .push_clip_path(&path, &krilla_fill_rule(clip.shape.rule()));
      pushed += 1;
    }

    self.enter(pushed, current, window, color_filter);
  }

  fn pop_clip(&mut self) {
    self.leave();
  }

  fn begin_effect(&mut self, id: EffectId, _effect: &EffectNode) {
    let (current, window, color_filter) = (
      self.current,
      self.emitter.window,
      self.emitter.color_filter.clone(),
    );
    let mut pushed = 0;

    if let Some(owner) = self.owners[id.index()]
      && let Some(node) = self.emitter.scene.root.node_at_path(&owner.path)
    {
      let style = &node.context.style;

      pushed += push_compositing(style, self.surface);

      if let Some((_, frame, transformed)) = self.place(owner) {
        if transformed > 0 {
          self.current = owner.transform;
        }
        pushed += transformed + self.emitter.push_mask_and_clip(node, frame, self.surface);
      }

      self.emitter.color_filter = self
        .emitter
        .composed_filter(color_filter.as_deref(), &style.filter);
    }

    self.enter(pushed, current, window, color_filter);
  }

  fn end_effect(&mut self) {
    self.leave();
  }
}

/// How far `shape` reaches down from its origin, as `(top, bottom)`.
fn vertical_extent(shape: &FillShape) -> Option<(f32, f32)> {
  match shape {
    FillShape::Rect(size) => Some((0.0, size.height)),
    FillShape::RoundedRect { size, offset, .. } => Some((offset.y, offset.y + size.height)),
    FillShape::Ellipse { center, radius } => Some((center.y - radius.y, center.y + radius.y)),
    FillShape::Contoured(clip) => Some((clip.offset.y, clip.offset.y + clip.size.height)),
    FillShape::Path { .. } => None,
  }
}

/// Pushes the blend mode, isolation, and opacity a box composites with,
/// returning how many states went on.
fn push_compositing(style: &ComputedStyle, surface: &mut Surface) -> usize {
  let mut pushed = 0;

  if style.mix_blend_mode != BlendMode::Normal {
    surface.push_blend_mode(krilla_blend(style.mix_blend_mode));
    pushed += 1;
  }
  if style.isolation == Isolation::Isolate {
    surface.push_isolated();
    pushed += 1;
  }
  let opacity = style.opacity.0;

  if opacity < 1.0 {
    surface.push_opacity(normalized(opacity));
    pushed += 1;
  }

  pushed
}

/// The PDF surface as a [`PaintDevice`], so the shared painting code can drive
/// it without knowing about krilla.
struct SurfaceDevice<'s, 'a> {
  surface: &'s mut Surface<'a>,
  filter: Option<&'s ColorFilter>,
  /// Whether each fill opens an artifact region of its own. Opening one only
  /// once something paints leaves no empty region behind, since marked
  /// content does not nest.
  artifact: bool,
  /// Each open clip and layer, innermost last.
  stack: Vec<Saved>,
}

/// A clip or layer the device holds open.
#[derive(Clone, Copy, PartialEq)]
enum Saved {
  Clip,
  /// A clip with no area, which hides every draw until it is popped.
  EmptyClip,
  Layer,
}

impl SurfaceDevice<'_, '_> {
  /// Clips later draws to `path`, or hides them when the clip has no area.
  fn open_clip(&mut self, path: Option<KrillaPath>, rule: CoreFillRule) {
    let saved = match &path {
      Some(_) => Saved::Clip,
      None => Saved::EmptyClip,
    };

    self.open(saved);

    if let Some(path) = path {
      self.surface.push_clip_path(&path, &krilla_fill_rule(rule));
    }
  }

  /// Records `saved`, opening the artifact the whole outermost state shares, since marked content
  /// does not nest.
  fn open(&mut self, saved: Saved) {
    if self.artifact && self.stack.is_empty() {
      self.surface.start_tagged(ARTIFACT);
    }

    self.stack.push(saved);
  }

  /// Pops the innermost clip or layer.
  fn close(&mut self) {
    let Some(saved) = self.stack.pop() else {
      return;
    };

    if saved != Saved::EmptyClip {
      self.surface.pop();
    }
    if self.artifact && self.stack.is_empty() {
      self.surface.end_tagged();
    }
  }

  /// Draws the path `build` makes at `transform`'s translation, with the rest
  /// of `transform` pushed around it. A pure translation folds into the path,
  /// which keeps the content stream free of a `cm` pair for every fill.
  fn draw(
    &mut self,
    transform: Affine,
    build: impl FnOnce(CorePoint<f32>) -> Option<KrillaPath>,
    paint: impl FnOnce(&mut Surface, &KrillaPath),
  ) {
    let flat = transform.only_translation();
    let origin = if flat {
      CorePoint {
        x: transform.x,
        y: transform.y,
      }
    } else {
      CorePoint::ZERO
    };
    if self.stack.contains(&Saved::EmptyClip) {
      return;
    }
    let Some(path) = build(origin) else {
      return;
    };
    let artifact = self.artifact && self.stack.is_empty();

    if !flat {
      self
        .surface
        .push_transform(&krilla_transform(transform.to_cols_array()));
    }
    if artifact {
      self.surface.start_tagged(ARTIFACT);
    }
    paint(self.surface, &path);
    if !flat {
      self.surface.pop();
    }
    if artifact {
      self.surface.end_tagged();
    }
  }
}

/// The alpha mask `border` paints on a box of `size` at `origin`.
fn border_mask_stream(
  border: &BorderProperties,
  size: Size<f32>,
  origin: CorePoint<f32>,
  surface: &mut Surface,
) -> Stream {
  draw_stream(surface, |surface| {
    BoxBorderPainter::new(border, size).paint(
      origin,
      &mut SurfaceDevice {
        surface,
        filter: None,
        artifact: false,
        stack: Vec::new(),
      },
    );
  })
}

impl SurfaceDevice<'_, '_> {
  /// Strokes `shape` under `transform` as `stroke` says, in `paint` at `opacity`.
  fn stroke_path(
    &mut self,
    shape: &FillShape,
    stroke: &StrokeStyle,
    paint: Paint,
    opacity: NormalizedF32,
    transform: Affine,
  ) {
    let stroke = Stroke {
      paint,
      opacity,
      width: stroke.width,
      line_cap: if stroke.round_cap {
        LineCap::Round
      } else {
        LineCap::Butt
      },
      dash: stroke.dash.map(|intervals| StrokeDash {
        array: intervals.to_vec(),
        offset: 0.0,
      }),
      ..Stroke::default()
    };

    self.draw(
      transform,
      |origin| krilla_path(&shape.to_commands(), origin),
      |surface, path| {
        surface.set_fill(None);
        surface.set_stroke(Some(stroke));
        surface.draw_path(path);
        surface.set_stroke(None);
      },
    );
  }

  /// Fills `shape` under `transform` with each of `fills` in turn, within its clip.
  fn fill_shape_with(&mut self, shape: &FillShape, fills: &[GlyphBackground], transform: Affine) {
    for background in fills {
      self.within(background, |device| {
        let fill = Fill {
          rule: krilla_fill_rule(shape.rule()),
          ..background.fill.clone()
        };

        device.draw(
          transform,
          |origin| shape_path(shape, origin),
          |surface, path| {
            surface.set_fill(Some(fill));
            surface.draw_path(path);
          },
        );
      });
    }
  }

  /// Strokes `shape` under `transform` as `stroke` says, with each of `fills` in turn, within its
  /// clip.
  fn stroke_shape_with(
    &mut self,
    shape: &FillShape,
    stroke: &StrokeStyle,
    fills: &[GlyphBackground],
    transform: Affine,
  ) {
    if stroke.width <= 0.0 {
      return;
    }
    for background in fills {
      self.within(background, |device| {
        let fill = &background.fill;

        device.stroke_path(shape, stroke, fill.paint.clone(), fill.opacity, transform);
      });
    }
  }

  /// Runs `draw` within `background`'s clip.
  fn within(&mut self, background: &GlyphBackground, draw: impl FnOnce(&mut Self)) {
    if let Some(clip) = &background.clip {
      self.surface.push_clip_path(clip, &FillRule::NonZero);
    }
    draw(self);
    if background.clip.is_some() {
      self.surface.pop();
    }
  }
}

impl PaintDevice for SurfaceDevice<'_, '_> {
  fn with_border_mask(
    &mut self,
    border: &BorderProperties,
    size: Size<f32>,
    origin: CorePoint<f32>,
    content: impl FnOnce(&mut Self),
  ) {
    let stream = border_mask_stream(border, size, origin, self.surface);

    self.open(Saved::Layer);
    self.surface.push_mask(Mask::new(stream, MaskType::Alpha));
    content(self);
    self.close();
  }

  fn transform(&self) -> Affine {
    Affine::scale(PT_PER_PX.recip(), PT_PER_PX.recip())
      * core_transform(self.surface.page_transform())
  }

  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    let fill = Fill {
      rule: krilla_fill_rule(shape.rule()),
      ..fill_from_rgba(filtered(self.filter, color), 1.0)
    };

    self.draw(
      transform,
      |origin| shape_path(shape, origin),
      |surface, path| {
        surface.set_fill(Some(fill));
        surface.draw_path(path);
      },
    );
  }

  fn stroke_shape(&mut self, shape: &FillShape, stroke: &StrokeStyle, transform: Affine) {
    if stroke.color.0[3] == 0 || stroke.width <= 0.0 {
      return;
    }
    let paint = fill_from_rgba(filtered(self.filter, stroke.color), 1.0).paint;

    self.stroke_path(shape, stroke, paint, NormalizedF32::ONE, transform);
  }

  fn push_clip(&mut self, shape: &FillShape, transform: Affine) {
    let path = if transform.only_translation() {
      shape_path(
        shape,
        CorePoint {
          x: transform.x,
          y: transform.y,
        },
      )
    } else {
      krilla_path(&device_commands(shape, transform), CorePoint::ZERO)
    };

    self.open_clip(path, shape.rule());
  }

  // PDF leaves antialiasing to the viewer, as Skia's PDF backend does.
  fn push_aliased_clip(&mut self, shape: &FillShape, transform: Affine) {
    self.push_clip(shape, transform);
  }

  fn push_aliased_clip_out(&mut self, shape: &FillShape, transform: Affine) {
    self.push_clip_out(shape, transform);
  }

  fn push_clip_out(&mut self, shape: &FillShape, transform: Affine) {
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    BorderProperties::default().append_mask_commands(
      &mut commands,
      Size {
        width: UNBOUNDED * 2.0,
        height: UNBOUNDED * 2.0,
      },
      CorePoint {
        x: -UNBOUNDED,
        y: -UNBOUNDED,
      },
    );
    commands.extend(device_commands(shape, transform));
    self.open_clip(
      krilla_path(&commands, CorePoint::ZERO),
      CoreFillRule::EvenOdd,
    );
  }

  fn pop_clip(&mut self) {
    self.close();
  }

  fn begin_layer(&mut self, opacity: f32, _bounds: Option<LayerBounds>) {
    self.open(Saved::Layer);
    self.surface.push_opacity(normalized(opacity));
  }

  fn end_layer(&mut self) {
    self.close();
  }

  fn fill_shadow(&mut self, shape: &ShadowShape, shadow: &SizedShadow, transform: Affine) {
    let color = filtered(self.filter, shadow.color);
    let transform = Affine::translation(shadow.offset_x, shadow.offset_y) * transform;
    let bands = Band::of(shadow.blur_radius);
    // The bands' alphas add up to the blur's coverage for an opaque colour, so a translucent one
    // applies its alpha once, over all of them.
    let grouped = bands.len() > 1 && color[3] < u8::MAX;
    let band_color = if grouped {
      [color[0], color[1], color[2], u8::MAX]
    } else {
      color
    };

    if grouped {
      self.begin_layer(f32::from(color[3]) / f32::from(u8::MAX), None);
    }

    for band in bands {
      let band_shape = shape.spread(band.spread).fill_shape();
      let fill = Fill {
        rule: krilla_fill_rule(band_shape.rule()),
        ..fill_from_rgba(band_color, band.alpha)
      };

      self.draw(
        transform,
        |origin| shape_path(&band_shape, origin),
        |surface, path| {
          surface.set_fill(Some(fill));
          surface.draw_path(path);
        },
      );
    }

    if grouped {
      self.end_layer();
    }
  }
}

/// `shape`'s path with every point mapped through `transform`.
fn device_commands(shape: &FillShape, transform: Affine) -> Vec<PathCommand> {
  shape
    .to_commands()
    .into_iter()
    .map(|command| {
      command.map_points(|point| {
        let (x, y) = transform.transform_point(point.x, point.y);

        CorePoint { x, y }
      })
    })
    .collect()
}

/// A paint glyphs show under `background-clip: text`, and the rectangle it shows in.
struct GlyphBackground {
  fill: Fill,
  clip: Option<KrillaPath>,
}

/// The PDF surface as a [`PaintDevice`] for one block's text: shapes draw as on any surface, and
/// glyphs draw with text operators so the text stays extractable.
///
/// Approximate: a blurred `text-shadow` fades through the stepped bands of [`Band`], since PDF has
/// no blur operator.
struct TextDevice<'e, 's, 'a> {
  emitter: &'e Emitter<'e>,
  device: SurfaceDevice<'s, 'a>,
  /// The inline layout whose text the glyphs carry, unless they only draw a mask.
  built: Option<&'e BuiltInlineLayout<'e>>,
  /// The shadow every draw becomes while one is open.
  shadow: Option<SizedShadow>,
  /// While a `background-clip: text` mask draws, the fills each of its shapes and glyphs shows
  /// in place of its own paint.
  through: Option<Rc<[GlyphBackground]>>,
}

impl TextDevice<'_, '_, '_> {
  /// `color` and `transform`, or the open shadow's colour and `transform` moved by its offset.
  /// Runs `draw` once in `color` with no spread, or while a blurred shadow is open, once per shadow
  /// [`Band`] with the stroke width that spreads it, inside a group of the band's opacity. The
  /// bands draw `color` opaque inside one group of its alpha, since their alphas add up to the
  /// blur's coverage for an opaque colour.
  fn in_shadow_bands(
    &mut self,
    color: Color,
    mut draw: impl FnMut(&mut SurfaceDevice<'_, '_>, Color, f32),
  ) {
    let Some(shadow) = self.shadow.filter(|shadow| shadow.blur_radius > 0.0) else {
      return draw(&mut self.device, color, 0.0);
    };
    let alpha = color.0[3];
    let opaque = Color([color.0[0], color.0[1], color.0[2], u8::MAX]);

    if alpha < u8::MAX {
      self
        .device
        .begin_layer(f32::from(alpha) / f32::from(u8::MAX), None);
    }
    for band in Band::of(shadow.blur_radius) {
      self.device.begin_layer(band.alpha, None);
      draw(&mut self.device, opaque, 2.0 * band.spread);
      self.device.end_layer();
    }
    if alpha < u8::MAX {
      self.device.end_layer();
    }
  }

  fn shadowed(&self, color: Color, transform: Affine) -> (Color, Affine) {
    match self.shadow {
      Some(shadow) => (shadow.color, self.shadow_moved(transform)),
      None => (color, transform),
    }
  }

  /// `transform` moved by the open shadow's offset. A clip opened inside a shadow clips what casts
  /// it, as Blink draws a text shadow's content into a `DropShadowPaintFilter` layer.
  fn shadow_moved(&self, transform: Affine) -> Affine {
    match self.shadow {
      Some(shadow) => Affine::translation(shadow.offset_x, shadow.offset_y) * transform,
      None => transform,
    }
  }

  /// The stroke a run's glyphs draw with: its `-webkit-text-stroke`, else its faux bold, both in
  /// `color` when a shadow recolours them.
  fn glyph_stroke(&self, shaped: &ShapedRun, fill: &Fill, color: Option<Color>) -> Option<Stroke> {
    let brush = &shaped.brush;

    if brush.stroke_width > 0.0 && brush.stroke_color.0[3] != 0 {
      let stroke_fill = fill_from_rgba(
        self.emitter.filtered(color.unwrap_or(brush.stroke_color)),
        1.0,
      );

      return Some(Stroke {
        paint: stroke_fill.paint,
        opacity: stroke_fill.opacity,
        width: brush.stroke_width,
        ..Stroke::default()
      });
    }

    synthetic_stroke(shaped, fill)
  }
}

impl PaintDevice for TextDevice<'_, '_, '_> {
  fn with_border_mask(
    &mut self,
    border: &BorderProperties,
    size: Size<f32>,
    origin: CorePoint<f32>,
    content: impl FnOnce(&mut Self),
  ) {
    let stream = border_mask_stream(border, size, origin, self.device.surface);

    self.device.open(Saved::Layer);
    self
      .device
      .surface
      .push_mask(Mask::new(stream, MaskType::Alpha));
    content(self);
    self.device.close();
  }

  fn transform(&self) -> Affine {
    self.device.transform()
  }

  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    if let Some(fills) = &self.through {
      return self.device.fill_shape_with(shape, fills, transform);
    }

    let (color, transform) = self.shadowed(color, transform);

    self.in_shadow_bands(color, |device, color, spread| {
      device.fill_shape(shape, color, transform);

      if spread > 0.0 {
        device.stroke_shape(
          shape,
          &StrokeStyle {
            color,
            width: spread,
            dash: None,
            round_cap: false,
          },
          transform,
        );
      }
    });
  }

  fn stroke_shape(&mut self, shape: &FillShape, stroke: &StrokeStyle, transform: Affine) {
    if let Some(fills) = &self.through {
      return self
        .device
        .stroke_shape_with(shape, stroke, fills, transform);
    }

    let (color, transform) = self.shadowed(stroke.color, transform);

    self.in_shadow_bands(color, |device, color, spread| {
      device.stroke_shape(
        shape,
        &StrokeStyle {
          color,
          width: stroke.width + spread,
          ..*stroke
        },
        transform,
      );
    });
  }

  fn push_clip(&mut self, shape: &FillShape, transform: Affine) {
    let transform = self.shadow_moved(transform);

    self.device.push_clip(shape, transform);
  }

  fn push_clip_out(&mut self, shape: &FillShape, transform: Affine) {
    let transform = self.shadow_moved(transform);

    self.device.push_clip_out(shape, transform);
  }

  fn push_aliased_clip(&mut self, shape: &FillShape, transform: Affine) {
    let transform = self.shadow_moved(transform);

    self.device.push_aliased_clip(shape, transform);
  }

  fn push_aliased_clip_out(&mut self, shape: &FillShape, transform: Affine) {
    let transform = self.shadow_moved(transform);

    self.device.push_aliased_clip_out(shape, transform);
  }

  fn pop_clip(&mut self) {
    self.device.pop_clip();
  }

  fn begin_layer(&mut self, opacity: f32, bounds: Option<LayerBounds>) {
    self.device.begin_layer(opacity, bounds);
  }

  fn end_layer(&mut self) {
    self.device.end_layer();
  }

  fn fill_shadow(&mut self, shape: &ShadowShape, shadow: &SizedShadow, transform: Affine) {
    self.device.fill_shadow(shape, shadow, transform);
  }
}

impl GlyphDevice for TextDevice<'_, '_, '_> {
  fn fill_background_layers(
    &mut self,
    background: &StripBackground<'_>,
    clip: &FillShape,
    transform: Affine,
  ) {
    self.push_clip(clip, transform);
    self.emitter.paint_background_layers(
      background.node,
      &background.background,
      background.strip,
      self.device.surface,
    );
    self.pop_clip();
  }

  fn begin_shadow(&mut self, shadow: &SizedShadow) {
    self.shadow = Some(*shadow);
  }

  fn end_shadow(&mut self) {
    self.shadow = None;
  }

  fn draw_glyph_run(
    &mut self,
    run: &PositionedInlineRun,
    _style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
  ) {
    let (fills, with_paint) = match (fill, &self.through) {
      (GlyphFill::Mask, Some(through)) => (through.clone(), false),
      (GlyphFill::Text | GlyphFill::Mask, _) => (Rc::from([]), true),
    };

    self.paint_glyph_run(run, frame, &fills, with_paint);
  }

  /// Approximate: PDF viewers disagree on where the patterns inside a soft-mask group land, so each
  /// shape and glyph of the mask paints the fills on its own instead. A translucent background then
  /// stacks where they overlap, where Blink fills it once through the whole mask.
  fn fill_text_clip(
    &mut self,
    background: &StripBackground<'_>,
    clip: &FillShape,
    transform: Affine,
    mask: &mut dyn FnMut(&mut dyn GlyphDevice),
  ) {
    let fills = self.emitter.background_fills(
      background.node,
      &background.background,
      background.strip,
      self.device.surface,
    );
    let outer = self.through.replace(fills.into());

    self.device.push_clip(clip, transform);
    mask(self);
    self.device.pop_clip();
    self.through = outer;
  }
}

impl TextDevice<'_, '_, '_> {
  /// Paints `fills` seen through `run`'s glyphs in the block at `frame`, then the glyphs themselves
  /// `with_paint`, or their shadow while one is open.
  fn paint_glyph_run(
    &mut self,
    run: &PositionedInlineRun,
    frame: BoxFrame,
    fills: &[GlyphBackground],
    with_paint: bool,
  ) {
    let shifted = match self.shadow {
      Some(shadow) => frame.shifted(CorePoint {
        x: shadow.offset_x,
        y: shadow.offset_y,
      }),
      None => frame,
    };
    let Some(GlyphRun {
      font,
      text,
      glyphs,
      origin,
    }) = self
      .emitter
      .glyph_run(run, self.built, shifted, frame.origin.y)
    else {
      return;
    };
    let shaped = &run.glyph_run;
    let shadow_color = self.shadow.map(|shadow| shadow.color);
    let rgba = self
      .emitter
      .filtered(shadow_color.unwrap_or(shaped.brush.color));
    let paint = fill_from_rgba(rgba, 1.0);
    let colors = shadow_color.map(|_| {
      ColorGlyphs::of(
        run,
        CorePoint {
          x: origin.x,
          y: origin.y,
        },
      )
    });
    let glyphs = match colors {
      Some(_) => glyphs
        .into_iter()
        .filter(|glyph| !ColorGlyphs::contains(run, glyph.id))
        .collect(),
      None => glyphs,
    };
    let stroke = self.glyph_stroke(shaped, &paint, shadow_color);
    let surface = &mut *self.device.surface;
    let oblique = self.emitter.push_oblique(shaped, origin, surface);

    // Outlined: text extraction keys on the text-showing operator, whatever the rendering mode,
    // so glyphs drawn a second time for a shadow or a background would put the text in the text
    // layer twice. Paths paint the same pixels and stay out of it.
    if shadow_color.is_none() {
      for background in fills {
        if let Some(clip) = &background.clip {
          surface.push_clip_path(clip, &FillRule::NonZero);
        }
        surface.set_fill(Some(background.fill.clone()));
        surface.set_stroke(background_stroke(shaped, &background.fill));
        surface.draw_glyphs(origin, &glyphs, font.clone(), text, shaped.font_size, true);
        if background.clip.is_some() {
          surface.pop();
        }
      }
    }

    if with_paint {
      match self.shadow.filter(|shadow| shadow.blur_radius > 0.0) {
        // Each band spreads the glyphs by stroking them, inside a group of the band's opacity so
        // the fill and stroke, and neighbouring glyphs, don't stack where they overlap.
        Some(shadow) => {
          // The shadow colour's alpha applies once, over bands drawn opaque.
          let translucent = paint.opacity != NormalizedF32::ONE;

          if translucent {
            surface.push_opacity(paint.opacity);
          }

          for band in Band::of(shadow.blur_radius) {
            let width = 2.0 * band.spread + stroke.as_ref().map_or(0.0, |stroke| stroke.width);

            surface.push_opacity(normalized(band.alpha));
            surface.set_fill(Some(Fill {
              opacity: NormalizedF32::ONE,
              ..paint.clone()
            }));
            surface.set_stroke((width > 0.0).then(|| Stroke {
              paint: paint.paint.clone(),
              opacity: NormalizedF32::ONE,
              width,
              line_join: LineJoin::Round,
              ..Stroke::default()
            }));
            surface.draw_glyphs(origin, &glyphs, font.clone(), text, shaped.font_size, true);
            if let Some(colors) = &colors {
              colors.draw_outlines(surface);
            }
            surface.pop();
          }
          #[cfg(feature = "images")]
          if let Some(colors) = &colors {
            draw_blurred_silhouettes(
              &colors.bitmaps,
              shadow.blur_radius,
              [rgba[0], rgba[1], rgba[2], u8::MAX],
              surface,
            );
          }
          if translucent {
            surface.pop();
          }
        }
        None => {
          surface.set_fill(Some(paint));
          surface.set_stroke(stroke);
          surface.draw_glyphs(
            origin,
            &glyphs,
            font,
            text,
            shaped.font_size,
            shadow_color.is_some(),
          );
          if let Some(colors) = &colors {
            colors.draw_outlines(surface);
            #[cfg(feature = "images")]
            draw_silhouettes(&colors.bitmaps, rgba, surface);
          }
        }
      }
    }

    if oblique {
      surface.pop();
    }
    surface.set_stroke(None);
  }
}

/// Draws each bitmap glyph's alpha filled with `color`.
#[cfg(feature = "images")]
fn draw_silhouettes(
  bitmaps: &[(&ResolvedBitmapGlyph, CorePoint<f32>)],
  color: [u8; 4],
  surface: &mut Surface,
) {
  for (bitmap, origin) in bitmaps {
    let alpha: Vec<u8> = bitmap
      .image
      .data()
      .iter()
      .skip(3)
      .step_by(4)
      .copied()
      .collect();

    draw_alpha(
      &alpha,
      bitmap.image.width(),
      bitmap.image.height(),
      color,
      Affine::translation(origin.x, origin.y) * bitmap.image_transform(),
      surface,
    );
  }
}

/// Draws each bitmap glyph's alpha blurred as Blink's `DropShadowPaintFilter` blurs it.
#[cfg(feature = "images")]
fn draw_blurred_silhouettes(
  bitmaps: &[(&ResolvedBitmapGlyph, CorePoint<f32>)],
  blur_radius: f32,
  color: [u8; 4],
  surface: &mut Surface,
) {
  let sigma = BlurType::Shadow.to_sigma(blur_radius);
  let pad = (3.0 * sigma).ceil() as usize;

  for (bitmap, origin) in bitmaps {
    let (width, height) = (
      bitmap.placement.width as usize,
      bitmap.placement.height as usize,
    );
    let padded_width = width + 2 * pad;
    let padded_height = height + 2 * pad;
    let mut alpha = vec![0u8; width * height];
    let mut pixels = vec![[0u8; 4]; padded_width * padded_height];

    bitmap.write_alpha_mask(&mut alpha);
    for (row, line) in alpha.chunks_exact(width.max(1)).enumerate() {
      for (column, value) in line.iter().enumerate() {
        pixels[(row + pad) * padded_width + column + pad][3] = *value;
      }
    }
    blur_rgba(&mut pixels, padded_width, padded_height, sigma);

    let alpha: Vec<u8> = pixels.iter().map(|pixel| pixel[3]).collect();

    draw_alpha(
      &alpha,
      padded_width as u32,
      padded_height as u32,
      color,
      Affine::translation(
        origin.x + bitmap.placement.left as f32 - pad as f32,
        origin.y - bitmap.placement.top as f32 - pad as f32,
      ),
      surface,
    );
  }
}

/// Draws an alpha mask filled with `color`.
#[cfg(feature = "images")]
fn draw_alpha(
  alpha: &[u8],
  width: u32,
  height: u32,
  color: [u8; 4],
  transform: Affine,
  surface: &mut Surface,
) {
  let Some(size) = KrillaSize::from_wh(width as f32, height as f32) else {
    return;
  };
  let data = alpha
    .iter()
    .flat_map(|alpha| {
      let alpha = (u16::from(*alpha) * u16::from(color[3]) + 127) / 255;

      [color[0], color[1], color[2], alpha as u8]
    })
    .collect();

  surface.push_transform(&krilla_transform(transform.to_cols_array()));
  surface.draw_image(KrillaImage::from_rgba8(data, width, height), size);
  surface.pop();
}

/// A run ready to draw: its font, the text its glyphs map to, and where it
/// starts.
struct GlyphRun<'r> {
  font: Font,
  text: &'r str,
  glyphs: Vec<PdfGlyph>,
  origin: Point,
}

/// Names an image in an error: its URL, or that it came in as raw bytes.
#[cfg(feature = "images")]
fn image_label(src: &ImageSourceInput) -> &str {
  match src {
    ImageSourceInput::Url(url) => url,
    _ => "inline image bytes",
  }
}

/// Whether the node is an image explicitly marked decorative (`alt=""`), so its content is emitted
/// as an artifact instead of a `Figure` element.
fn decorative_image(node: &RenderNode) -> bool {
  node.node.as_ref().is_some_and(|source| {
    source.tag_name().is_some_and(|name| name == "img") && source.alt() == Some("")
  })
}

/// The stroke that fakes bold for a face with no weight of its own to reach, at the width the
/// raster renderer emboldens with.
fn background_stroke(shaped: &ShapedRun, fill: &Fill) -> Option<Stroke> {
  let width = shaped
    .synthetic_bold
    .unwrap_or(0.0)
    .max(shaped.brush.stroke_width);

  (width > 0.0).then(|| Stroke {
    paint: fill.paint.clone(),
    opacity: fill.opacity,
    width,
    ..Stroke::default()
  })
}

fn synthetic_stroke(shaped: &ShapedRun, fill: &Fill) -> Option<Stroke> {
  Some(Stroke {
    paint: fill.paint.clone(),
    // A colour's alpha lives in the fill's opacity, not its paint, so a stroke
    // built from the paint alone comes out fully opaque.
    opacity: fill.opacity,
    width: shaped.synthetic_bold?,
    ..Stroke::default()
  })
}
