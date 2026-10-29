//! The scene walker that emits boxes, text and images onto a krilla surface.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[cfg(all(feature = "images", feature = "svg"))]
use takumi_core::resources::image::{ImageSource, SvgSource};
#[cfg(feature = "images")]
use takumi_core::{
  context::RenderContext,
  layout::node::{ImageData, ImageSourceInput, NodeKind, resolve_image},
};
use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{
    ComputedLayout as Layout, NodeId, PathCommand, Point as CorePoint, Rect as CoreRect, Size,
  },
  layout::{
    background_image_geometry::{BoxBackgroundPaintContext, FillLayers, ImageTiling},
    border::BorderProperties,
    inline::{InlineBoxKind, PositionedInlineRun, ProcessedInlineSpan, ShapedRun},
    inline_box::resolve_inline_box,
    tree::{NodeOrigin, RenderNode},
  },
  paint::ConicGradientTile,
  paint_chunk::{ChunkPart, ConversionContext, PropertySink},
  paint_property::{ClipId, ClipKind, ClipNode, EffectId, EffectKind, EffectNode},
  painter::{
    BoxBackground, BoxBorderPainter, BoxFrame, BoxPainter, CellArea, CellBackground, FillShape,
    GlyphDevice, GlyphFill, LayerBounds, LineItem, OwnContent, PaintDevice, PaintedInline,
    PendingOutline, ShadowShape, StripBackground, StrokeStyle, TableDecorations, TextClip,
    UNBOUNDED, paint_collapsed_borders,
  },
  scene::{NodePaint, SceneView},
  shadow::SizedShadow,
  style::{
    Affine, BackgroundImage, BlendMode, Color, ComputedStyle, Display, FillRule as CoreFillRule,
    Filter, Isolation, Lang, ResolvedGradientStop,
  },
};

#[cfg(feature = "images")]
use crate::krilla::{geom::Size as KrillaSize, image::Image as KrillaImage};
#[cfg(feature = "images")]
use crate::paint::{RasterizedImages, rasterized_image};
#[cfg(all(feature = "svg", feature = "images"))]
use crate::svg::SvgPainter;
use crate::{
  chunks::{ResolvedChunk, SceneChunks},
  filter::{ColorFilter, filtered, unsupported_filter},
  glyph::{ColorGlyphs, PdfGlyph, Uncovered, run_glyphs},
  inline::{InlineCache, visit_inline_layout},
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
    draw_stream, empty_path, expanded_radial_stops, fill_from_rgba, krilla_path, krilla_stop,
    krilla_stops, normalized, pop_transforms, rect_path, shape_path, spread,
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
  pub(crate) scene: SceneView<'a>,
  pub(crate) document: &'a DocumentState<'a>,
  /// The content tree's inline layouts; band trees build theirs on the fly.
  pub(crate) inline: Option<&'a InlineCache<'a>>,
  /// Pre-resolved chunks for the content tree; other trees resolve theirs per walk.
  pub(crate) chunks: Option<&'a SceneChunks<'a>>,
  /// The page window this walk paints through.
  pub(crate) window: Window,
  /// Whether this walk records marked content for the structure tree.
  pub(crate) tagged: bool,
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
  #[cfg(feature = "images")]
  images: RasterizedImages,
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
      #[cfg(feature = "images")]
      images: RasterizedImages::default(),
    }
  }

  /// The error the pages left behind, if any.
  pub(crate) fn into_error(self) -> Option<PdfError> {
    let issues = self.issues.into_inner();

    issues.failure.or_else(|| issues.uncovered.into_error())
  }
}

impl<'a> Emitter<'a> {
  pub(crate) fn new(
    scene: SceneView<'a>,
    document: &'a DocumentState<'a>,
    inline: Option<&'a InlineCache<'a>>,
    chunks: Option<&'a SceneChunks<'a>>,
    tagged: bool,
  ) -> Self {
    Self {
      scene,
      document,
      inline,
      chunks,
      window: Window::default(),
      tagged: tagged && document.tags.is_some(),
      color_filter: None,
    }
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
    let resolved;
    let chunks = match self.chunks {
      Some(chunks) => chunks,
      None => {
        resolved = SceneChunks::new(scene);
        &resolved
      }
    };
    let visible = chunks.visible(self.window);
    let mut conversion = ConversionContext::new(
      scene.properties,
      ChunkWriter {
        emitter: self,
        surface,
        owners: &chunks.owners,
        current: Affine::IDENTITY,
        entries: Vec::new(),
        error: None,
      },
    );

    for chunk in visible {
      conversion.switch_to(chunk.chunk.state());
      conversion.sink().emit(chunk);
    }

    conversion.finish().error.map_or(Ok(()), Err)
  }

  /// Pushes the box's mask, returning how many states went on.
  fn push_mask(&mut self, node: &RenderNode, frame: BoxFrame, surface: &mut Surface) -> usize {
    if let Some(mask) = self.mask(node, frame, surface) {
      surface.push_mask(mask);
      1
    } else {
      0
    }
  }

  /// Paints shadows, backgrounds, and borders in CSS order.
  fn emit_decorations(
    &self,
    node: &RenderNode,
    frame: BoxFrame,
    text_clip: Option<&TextClip>,
    table: TableDecorations,
    surface: &mut Surface,
  ) {
    if !node.paints_own_box() {
      return;
    }

    match table {
      TableDecorations::None => self.emit_box(node, frame, text_clip, true, surface),
      TableDecorations::Cell {
        paints_border,
        clip: None,
      } => {
        self.emit_box(node, frame, text_clip, paints_border, surface);
      }
      TableDecorations::Cell {
        paints_border,
        clip: Some(clip),
      } => {
        self.within_cell(frame.origin, clip, surface, |surface| {
          self.emit_box(node, frame, text_clip, paints_border, surface);
        });
      }
      TableDecorations::Table {
        grid,
        grid_offset,
        paints_border,
        columns,
      } => {
        let grid_frame = BoxFrame::new(grid, frame.origin + grid_offset);

        self.emit_box(node, grid_frame, text_clip, paints_border, surface);
        for column in columns {
          let Some(source) = node.node_at_path(&column.path) else {
            continue;
          };
          let mut column_node = source.clone();

          column_node.context = column.context;
          for background in column.cells {
            self.emit_cell_background(&column_node, frame, background, surface);
          }
        }
      }
      TableDecorations::Part { cells } => {
        let painter = BoxPainter::new(&node.context, frame.layout);

        painter.paint_normal_box_shadows(frame.origin, &mut self.device(surface, self.tagged));
        for background in cells {
          self.emit_cell_background(node, frame, background, surface);
        }
        painter.paint_inset_box_shadows(frame.origin, &mut self.device(surface, self.tagged));
      }
    }
  }

  /// Emits the box's shadows, background, through `text_clip` when it has one, and its border
  /// when `paints_border`.
  fn emit_box(
    &self,
    node: &RenderNode,
    frame: BoxFrame,
    text_clip: Option<&TextClip>,
    paints_border: bool,
    surface: &mut Surface,
  ) {
    let painter = BoxPainter::new(&node.context, frame.layout);

    painter.paint_normal_box_shadows(frame.origin, &mut self.device(surface, self.tagged));
    self.emit_background(node, frame, surface);
    if let Some(text_clip) = text_clip {
      let painted = text_clip.paint_background(
        frame.origin,
        &mut TextDevice {
          emitter: self,
          device: self.device(surface, self.tagged),
          text: None,
          shadow: None,
          through: None,
        },
      );

      if let Err(error) = painted {
        self.fail(error.into());
      }
    }
    painter.paint_inset_box_shadows(frame.origin, &mut self.device(surface, self.tagged));
    if paints_border {
      painter.paint_border(frame.origin, &mut self.device(surface, self.tagged));
    }
  }

  /// Emits the box's background colour and image layers.
  fn emit_background(&self, node: &RenderNode, frame: BoxFrame, surface: &mut Surface) {
    let painter = BoxPainter::new(&node.context, frame.layout);

    painter.background_color(frame.origin, &mut self.device(surface, self.tagged));
    self.emit_background_layers(node, &painter.background(), frame, surface);
  }

  /// Emits the background of `node`, a table part, over `background.area` inside its cell, both
  /// relative to `frame`, the box painting it.
  fn emit_cell_background(
    &self,
    node: &RenderNode,
    frame: BoxFrame,
    background: CellBackground,
    surface: &mut Surface,
  ) {
    let cell_top = frame.origin.y + background.cell.offset.y;

    if self
      .window
      .excludes(cell_top, cell_top + background.cell.size.height)
    {
      return;
    }
    let at = background.area.offset;
    let layout = Layout {
      location: frame.layout.location + at,
      size: background.area.size,
      border: Default::default(),
      padding: Default::default(),
    };

    self.within_cell(frame.origin, background.cell, surface, |surface| {
      self.emit_background(node, BoxFrame::new(layout, frame.origin + at), surface);
    });
  }

  /// Runs `emit` clipped to `cell`, relative to `origin`.
  fn within_cell(
    &self,
    origin: CorePoint<f32>,
    cell: CellArea,
    surface: &mut Surface,
    emit: impl FnOnce(&mut Surface),
  ) {
    let clip = shape_path(&FillShape::Rect(cell.size), origin + cell.offset);

    let Some(clip) = clip else {
      return;
    };

    surface.push_clip_path(&clip, &FillRule::NonZero);
    emit(surface);
    surface.pop();
  }

  /// Emits the box's own content inside its structure tag when tagging is on.
  fn emit_tagged_content(
    &mut self,
    node: &RenderNode,
    content: &OwnContent<'_>,
    paint: &NodePaint,
    frame: BoxFrame,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    // A repeated table header is an artifact: the occurrence where the table begins carries the
    // tags, and a second marked one would double the reading order.
    let is_repeated = self
      .scene
      .results
      .is_repeated_fragment(paint.node_id, paint.fragment_index);
    let tagged = self.tagged && draws(content) && !is_repeated;
    let artifact = self.tagged && is_repeated;

    if tagged {
      self.start_node_region(node, Some(&paint.path), surface);
    } else if artifact {
      surface.start_tagged(ARTIFACT);
    }
    // Nothing inside the artifact is tagged, the replaced boxes on its lines included.
    let tagging = self.tagged;

    self.tagged = tagging && !is_repeated;
    let emitted = self.emit_own_content(node, content, paint.node_id, frame, surface);

    self.tagged = tagging;
    emitted?;
    if tagged || artifact {
      surface.end_tagged();
    }

    Ok(())
  }

  /// Paints `background-image` layers bottom first, clipped to the `background-clip` box.
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
        (None, Some(clip)) => surface.push_clip_path(clip, &shape.rule().into()),
        (None, None) => return,
      }
      self.paint_background_layers(node, background, frame, surface);
      surface.pop();
    });
  }

  /// Draws one layer, a repeated one as a single pattern tile.
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
        tiling,
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
      self.background_layer(image, node, tiling, CorePoint::ZERO, tile, tile_space);
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
    tiling: &ImageTiling,
    at: CorePoint<f32>,
    surface: &mut Surface,
    pattern_space: Transform,
  ) {
    let size = tiling.tile;
    let (w, h) = (size.width, size.height);

    #[cfg(feature = "images")]
    if let BackgroundImage::Url(url) = image {
      let Ok(source) = resolve_image(url, &node.context) else {
        return;
      };
      #[cfg(feature = "svg")]
      if let ImageSource::Svg(svg) = &source {
        self.draw_svg(svg, &node.context, at, tiling.image_size, size, surface);
        return;
      }
      let Some(krilla_image) = self.drawable(
        url,
        self
          .document
          .images
          .get(url, &source, &node.context, self.color_filter.as_deref()),
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

  /// The krilla paint of a gradient layer. `pattern_space` carries the stream's default space,
  /// per PDF 32000-1 8.7.3.1.
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
    let images = node.context.style.misc2_data.mask_image.as_deref()?;

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
        surface.push_blend_mode(layer.blend_mode.into());
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
    content: &OwnContent<'_>,
    node_id: NodeId,
    frame: BoxFrame,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    match *content {
      OwnContent::Inline(_) => self.emit_node_text(node, node_id, frame, surface),
      #[cfg(feature = "images")]
      OwnContent::Image(image) => {
        self.emit_image(image, &node.context, frame, surface);
        Ok(())
      }
      _ => Ok(()),
    }
  }

  #[cfg(feature = "images")]
  /// Draws an image node into its content box, an SVG as vector ops.
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
    #[cfg(feature = "svg")]
    let svg = match &source {
      ImageSource::Svg(svg) => Some(svg),
      _ => None,
    };
    #[cfg(not(feature = "svg"))]
    let svg: Option<()> = None;

    let krilla_image = if svg.is_none() {
      let Some(image) = self.drawable(
        image_label(&image.src),
        match &image.src {
          ImageSourceInput::Url(src) => {
            self
              .document
              .images
              .get(src, &source, context, self.color_filter.as_deref())
          }
          _ => rasterized_image(&source, context, self.color_filter.as_deref()),
        },
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
    if let Some(svg) = svg {
      let size = Size {
        width: dw,
        height: dh,
      };

      self.draw_svg(
        svg,
        context,
        CorePoint { x: ix, y: iy },
        size,
        size,
        surface,
      );
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

  #[cfg(all(feature = "svg", feature = "images"))]
  /// Draws `svg` laid out at `viewport`, Blink's container size, stretched over `size` at `at` and
  /// clipped to its canvas.
  fn draw_svg(
    &self,
    svg: &SvgSource,
    context: &RenderContext,
    at: CorePoint<f32>,
    viewport: Size<f32>,
    size: Size<f32>,
    surface: &mut Surface,
  ) {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
      return;
    }
    let ops = svg.vector_ops(context, viewport);
    let canvas =
      KrillaRect::from_xywh(0.0, 0.0, viewport.width, viewport.height).and_then(rect_path);

    surface.push_transform(&Transform::from_row(
      size.width / viewport.width,
      0.0,
      0.0,
      size.height / viewport.height,
      at.x,
      at.y,
    ));
    if let Some(canvas) = &canvas {
      surface.push_clip_path(canvas, &FillRule::NonZero);
    }
    SvgPainter::new(self.color_filter.as_deref()).draw(surface, ops);
    if canvas.is_some() {
      surface.pop();
    }
    surface.pop();
  }

  /// Draws a text-bearing box's runs, from the pre-built inline map when the node is in it (content
  /// tree) or built on the fly (band trees).
  fn emit_node_text(
    &mut self,
    node: &RenderNode,
    node_id: NodeId,
    frame: BoxFrame,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    visit_inline_layout(
      self.inline,
      node,
      node_id,
      frame.layout,
      |painted, font_style| self.draw_runs(node, painted, frame, font_style, surface),
    )?;
    Ok(())
  }

  /// Paints the runs on the lines this page owns through takumi-core's text painter, then the
  /// inline boxes.
  fn draw_runs(
    &mut self,
    node: &RenderNode,
    painted: &PaintedInline<'_>,
    frame: BoxFrame,
    font_style: &SizedFontStyle,
    surface: &mut Surface,
  ) {
    let y = frame.origin.y;
    let lines = painted.runs.lines(frame.layout, |item| {
      self.window.shows_line_item(item.below(y))
    });
    let mut device = TextDevice {
      emitter: self,
      device: self.device(surface, false),
      text: Some(painted.text()),
      shadow: None,
      through: None,
    };

    lines.paint(&painted.spans, font_style, frame, &mut device);
    self.emit_inline_boxes(node, painted, frame, surface);
  }

  /// Paints the inline layout's replaced boxes.
  fn emit_inline_boxes(
    &mut self,
    owner: &RenderNode,
    painted: &PaintedInline<'_>,
    frame: BoxFrame,
    surface: &mut Surface,
  ) {
    let BoxFrame {
      layout,
      origin: CorePoint { y, .. },
    } = frame;

    // Marked content does not nest, so each box takes the caller's region over and hands it back.
    let owner_tagged = self.tagged && draws(&OwnContent::of(owner));

    for positioned in painted
      .runs
      .inline_boxes
      .iter()
      .filter(|positioned| positioned.kind == InlineBoxKind::InFlow)
    {
      let Some(ProcessedInlineSpan::Box(item)) = painted.spans.get(positioned.id as usize) else {
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
      let Some(replaced) = resolve_inline_box(positioned, item, layout) else {
        continue;
      };

      // The opacity and `filter` `emit_box` would apply.
      let opacity = node.context.style.svg_data.opacity.0;
      let faded = opacity < 1.0;
      // Marked content cannot nest, so the region wraps the whole box only inside an opacity group.
      let box_tagged = cfg!(feature = "images") && self.tagged;
      let box_wrapped = box_tagged && faded;

      if owner_tagged {
        surface.end_tagged();
      }
      if box_wrapped {
        self.start_tagged_node(node, surface);
      }

      if faded {
        surface.push_opacity(normalized(opacity));
      }
      let outer_filter = self.color_filter.clone();
      self.color_filter = self.composed_filter(
        outer_filter.as_deref(),
        &node.context.style.misc1_data.filter,
      );

      #[cfg(feature = "images")]
      self.emit_inline_replaced(
        replaced.node,
        BoxFrame::new(replaced.layout, frame.origin + replaced.layout.location),
        box_tagged && !box_wrapped,
        surface,
      );
      #[cfg(not(feature = "images"))]
      let _ = replaced;

      self.color_filter = outer_filter;
      if faded {
        surface.pop();
      }
      if box_wrapped {
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
    self.emit_decorations(node, frame, None, TableDecorations::None, surface);
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

  /// Opens the marked-content region a node's own content draws in: an
  /// artifact for a decorative image, otherwise a region recorded at `path`.
  fn start_node_region(&self, node: &RenderNode, path: Option<&[usize]>, surface: &mut Surface) {
    if decorative_image(node) {
      surface.start_tagged(ARTIFACT);
      return;
    }
    let identifier = surface.start_tagged(self.content_tag(node));

    let Some(tags) = self.tags() else {
      return;
    };
    let Some(path) = path else {
      return;
    };

    if node.origin == NodeOrigin::Marker
      && let Some((_, owner_path)) = path.split_last()
    {
      if let Some(owner) = self
        .scene
        .root
        .node_at_path(self.marker_tag_target(owner_path))
      {
        tags
          .borrow_mut()
          .record_label(owner.context.box_id(), identifier);
      }
    } else {
      tags.borrow_mut().record(node.context.box_id(), identifier);
    }
  }

  /// Opens the region for a node the paint list never visited, so its content
  /// still reaches the structure tree.
  fn start_tagged_node(&self, node: &RenderNode, surface: &mut Surface) {
    self.start_node_region(node, self.scene.root.path_to(node).as_deref(), surface);
  }

  /// The nearest list item holding the box at `owner_path`, whose `Lbl` holds its marker.
  fn marker_tag_target<'p>(&self, owner_path: &'p [usize]) -> &'p [usize] {
    let mut current: &RenderNode = self.scene.root;
    let mut length = owner_path.len();

    for (depth, index) in owner_path.iter().enumerate() {
      if current.context.style.base_data.display == Display::ListItem {
        length = depth;
      }
      let Some(child) = current
        .children
        .as_deref()
        .and_then(|children| children.get(*index))
      else {
        break;
      };
      current = child;
    }
    if current.context.style.base_data.display == Display::ListItem {
      length = owner_path.len();
    }

    &owner_path[..length]
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
        tiling,
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

  /// A run this page draws at `(x, y)`, its text from `text` when given, or `None` when it has
  /// no glyphs, no font, or this page does not show it in its block at `line_y`.
  fn glyph_run<'r>(
    &self,
    run: &PositionedInlineRun,
    text: Option<&'r str>,
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

    if !self.window.shows_line_item(item) {
      return None;
    }
    let text = text
      .and_then(|text| text.get(shaped.text_range.clone()))
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
      owned: !self.window.disowns_line(item.baseline),
    })
  }

  /// Shears the text about the run's baseline for faux oblique. Approximate: raster shears each
  /// glyph about its own origin.
  fn push_oblique(&self, shaped: &ShapedRun, origin: Point, surface: &mut Surface) -> bool {
    let Some(degrees) = shaped.synthetic_skew else {
      return false;
    };
    let tangent = degrees.to_radians().tan();

    surface.push_transform(&Transform::from_row(
      1.0,
      0.0,
      tangent,
      1.0,
      -tangent * (origin.y + shaped.baseline),
      0.0,
    ));
    true
  }

  /// A krilla font for a run's backing blob, instanced at the run's variation
  /// coordinates, sharing the blob's bytes.
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
      Data::from(shaped.font_blob().shared_data()),
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
  owners: &'w [Option<&'a NodePaint>],
  /// The transform the pushed surface states add to the scene's space.
  current: Affine,
  entries: Vec<Entered>,
  error: Option<PdfError>,
}

impl<'a> ChunkWriter<'_, 'a, '_> {
  /// The box `paint` names and its offset, pushing a transform when needed; returns how many.
  fn place(
    &mut self,
    node: Option<&'a RenderNode>,
    paint: &NodePaint,
  ) -> Option<(&'a RenderNode, BoxFrame, usize)> {
    let node = node?;
    let layout = self.emitter.scene.enter(paint).ok()?;
    let relative = self.current.invert().unwrap_or(Affine::IDENTITY) * paint.transform;

    if relative.only_translation() {
      let origin = CorePoint {
        x: relative.x,
        y: relative.y,
      };

      return Some((node, BoxFrame::new(layout, origin), 0));
    }

    self.surface.push_transform(&relative.into());
    Some((node, BoxFrame::new(layout, CorePoint::ZERO), 1))
  }

  /// Writes one chunk under the states already pushed.
  fn emit(&mut self, resolved: &ResolvedChunk<'a>) {
    let chunk = &resolved.chunk;

    // Skipping a chunk that paints outside the window only saves work; the page's own clip would
    // drop it anyway.
    if self.error.is_some() || self.emitter.window.excludes_bounds(chunk.node.paint_bounds) {
      return;
    }

    let Some((node, frame, pushed)) = self.place(resolved.node, chunk.node) else {
      return;
    };
    let emitter = &mut *self.emitter;
    let surface = &mut *self.surface;
    let result = match chunk.part {
      ChunkPart::Decorations => {
        let scene = emitter.scene;

        TextClip::of(scene.root, scene.results, chunk.node, &node.context)
          .and_then(|text_clip| {
            let table = TableDecorations::of(scene.root, scene.results, chunk.node)?;

            emitter.emit_decorations(node, frame, text_clip.as_ref(), table, surface);
            Ok(())
          })
          .map_err(PdfError::from)
      }
      ChunkPart::CollapsedBorders if node.paints_collapsed_borders() => {
        // The window maps onto the table only while no transform was pushed for it.
        let visible = (pushed == 0)
          .then_some(emitter.window.y)
          .flatten()
          .map(|(top, bottom)| (top - frame.origin.y, bottom - frame.origin.y));

        paint_collapsed_borders(
          emitter.scene.results,
          chunk.node,
          frame.translation(),
          visible,
          &mut emitter.device(surface, emitter.tagged),
        )
        .map_err(PdfError::from)
      }
      ChunkPart::CollapsedBorders => Ok(()),
      ChunkPart::Content => {
        emitter.emit_tagged_content(node, &resolved.content, chunk.node, frame, surface)
      }
      ChunkPart::Outline => {
        let outline = BoxPainter::new(&node.context, frame.layout).pending_outline(frame.origin);

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
    let (ClipKind::Shape(shape) | ClipKind::ClipPath(shape)) = &clip.kind else {
      return;
    };
    let origin = if relative.only_translation() {
      CorePoint {
        x: relative.x,
        y: relative.y,
      }
    } else {
      self.surface.push_transform(&relative.into());
      pushed += 1;
      self.current = clip.transform;
      CorePoint::ZERO
    };

    // A clip keeps content off the page but not out of the text layer, so what it cuts away must
    // never be emitted; only a clip in page space maps onto the window's axis.
    if current == Affine::IDENTITY
      && relative.only_translation()
      && let Some((top, bottom)) = vertical_extent(shape)
    {
      self
        .emitter
        .window
        .narrow(origin.y + top, origin.y + bottom);
    }
    if let Some(path) = shape_path(shape, origin).or_else(|| empty_path(origin)) {
      self.surface.push_clip_path(&path, &shape.rule().into());
      pushed += 1;
    }

    self.enter(pushed, current, window, color_filter);
  }

  fn pop_clip(&mut self, clip: &ClipNode) {
    if !matches!(clip.kind, ClipKind::PixelMovingFilter(_)) {
      self.leave();
    }
  }

  fn begin_effect(&mut self, id: EffectId, effect: &EffectNode) {
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

      if effect.kind == EffectKind::Group {
        pushed += push_compositing(style, self.surface);

        if let Some((_, frame, transformed)) = self.place(Some(node), owner) {
          if transformed > 0 {
            self.current = owner.transform;
          }
          pushed += transformed + self.emitter.push_mask(node, frame, self.surface);
        }
      } else {
        self.emitter.color_filter = self
          .emitter
          .composed_filter(color_filter.as_deref(), &style.misc1_data.filter);
      }
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

  if style.misc_data.mix_blend_mode != BlendMode::Normal {
    surface.push_blend_mode(style.misc_data.mix_blend_mode.into());
    pushed += 1;
  }
  if style.misc_data.isolation == Isolation::Isolate {
    surface.push_isolated();
    pushed += 1;
  }
  let opacity = style.svg_data.opacity.0;

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
  /// Whether each fill opens an artifact region of its own.
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
      self.surface.push_clip_path(&path, &rule.into());
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

  /// Draws the path `build` makes, folding a pure translation into it.
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
      self.surface.push_transform(&transform.into());
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
          rule: shape.rule().into(),
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
      * Affine::from(self.surface.page_transform())
  }

  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    let fill = Fill {
      rule: shape.rule().into(),
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
        rule: band_shape.rule().into(),
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

/// The PDF surface as a [`PaintDevice`] for one block's text, glyphs drawn as extractable text.
/// Approximate: a blurred `text-shadow` steps through [`Band`]s.
struct TextDevice<'e, 'x, 's, 'a> {
  emitter: &'e Emitter<'x>,
  device: SurfaceDevice<'s, 'a>,
  /// The text the glyphs carry, unless they only draw a mask.
  text: Option<&'e str>,
  /// The shadow every draw becomes while one is open.
  shadow: Option<SizedShadow>,
  /// While a `background-clip: text` mask draws, the fills each of its shapes and glyphs shows
  /// in place of its own paint.
  through: Option<Rc<[GlyphBackground]>>,
}

impl TextDevice<'_, '_, '_, '_> {
  /// Runs `draw` once, or once per shadow [`Band`] while a blurred shadow is open.
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

impl PaintDevice for TextDevice<'_, '_, '_, '_> {
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

impl GlyphDevice for TextDevice<'_, '_, '_, '_> {
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
      (GlyphFill::Mask, Some(through)) => (Some(Rc::clone(through)), false),
      (GlyphFill::Text | GlyphFill::Mask, _) => (None, true),
    };

    self.paint_glyph_run(run, frame, fills.as_deref().unwrap_or_default(), with_paint);
  }

  /// Approximate: each shape of the mask paints the fills itself, since viewers disagree on
  /// patterns in a soft mask.
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

impl TextDevice<'_, '_, '_, '_> {
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
      owned,
    }) = self
      .emitter
      .glyph_run(run, self.text, shifted, frame.origin.y)
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

    // Outlined, so the text layer holds the text once.
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
            shadow_color.is_some() || !owned,
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

  surface.push_transform(&transform.into());
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
  /// Whether this page owns the run's line and shows its text, not just its outlines.
  owned: bool,
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
