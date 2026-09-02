//! The scene walker that emits boxes, text and images onto a krilla surface.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[cfg(feature = "images")]
use takumi_core::{
  context::RenderContext,
  layout::{
    node::{ImageData, ImageSourceInput, Node, resolve_image},
    replaced::place_replaced,
  },
  resources::image::ImageSource,
};
use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, NodeId, Point as CorePoint, Size},
  layout::{
    border::{BorderProperties, inset_size, rect_offset, side_bands},
    clip::clip_shape_commands,
    decoration::{ClipBox, OutlineGeometry},
    inline::{
      BuiltInlineLayout, InlineRunLayout, PositionedInlineRun, ProcessedInlineSpan, ShapedRun,
      inline_background_path, run_decorations,
    },
    inline_box::{InlineBoxPaint, InlineSubtree, resolve_inline_box},
    node::NodeKind,
    tree::{LayoutResults, NodeOrigin, RenderNode},
  },
  paint::{ConicGradientTile, LinearGradientTile, RadialGradientTile, resolve_stops_along_axis},
  painter::{
    BoxPainter, BoxShadows, FillShape, PaintDevice, StrokeStyle, paint_border,
    paint_run_decorations,
  },
  scene::{NodePaint, PaintItemKind, StackingContextNode, build_stacking_contexts},
  shadow::SizedShadow,
  style::{
    Affine, BackgroundClip, BackgroundImage, BackgroundOrigin, BlendMode, BoxDecorationBreak,
    Color, Display, FillRule as CoreFillRule, Filter, Isolation, Lang, ResolvedGradientStop,
    TextDecorationLines,
  },
};

use crate::interactive::is_form_control;
#[cfg(feature = "images")]
use crate::krilla::{geom::Size as KrillaSize, image::Image as KrillaImage};
use crate::options::FormOptions;
#[cfg(feature = "images")]
use crate::paint::rasterized_image;
#[cfg(all(feature = "svg", feature = "images"))]
use crate::svg;
use crate::{
  background::{Placement, cycled},
  filter::{ColorFilter, unsupported_filter},
  glyph::run_glyphs,
  inline::{InlineMap, build_inline_runs, node_inline_items},
  krilla::{
    Data,
    geom::{Point, Rect as KrillaRect, Transform},
    mask::{Mask, MaskType},
    num::NormalizedF32,
    paint::{
      Fill, FillRule, LineCap, LinearGradient as KrillaLinearGradient, Paint, Pattern,
      RadialGradient as KrillaRadialGradient, SpreadMethod, Stroke, StrokeDash, SweepGradient,
    },
    surface::Surface,
    tagging::{Artifact, ArtifactType, ContentTag, SpanTag},
    text::{Font, Tag},
  },
  options::PdfError,
  paint::{
    empty_path, expanded_radial_stops, fill_from_rgba, krilla_blend, krilla_path, krilla_stop,
    krilla_stops, overflow_clip_rect, pop_transforms, rect_path, spread,
  },
  shadow::{emit_inset_shadows, emit_outer_shadows},
  tags::TagCollector,
  window::Window,
};

/// What a box left on the surface for its caller to unwind.
#[derive(Default)]
pub(crate) struct BoxState {
  /// Transforms, clips and layers to pop once the box and its children are
  /// done.
  pushed: usize,
  /// The `overflow` clip, popped before the outline so the outline escapes it.
  overflow_clip: usize,
  /// The outline, painted between the two pops.
  outline: Option<PendingOutline>,
}

/// An outline waiting for its box's state to be popped.
#[derive(Clone)]
pub(crate) struct PendingOutline {
  outline: OutlineGeometry,
  x: f32,
  y: f32,
}

/// Blob identity, collection index, and the variation coordinates the run was
/// shaped at. One blob instanced at two weights is two embedded fonts, so the
/// coordinates belong in the key.
pub(crate) type FontKey = (u64, u32, Vec<([u8; 4], u32)>);

/// Krilla fonts embedded so far, one per distinct instance.
pub(crate) type FontMap = HashMap<FontKey, Font>;

pub(crate) struct Emitter<'a> {
  pub(crate) root: &'a RenderNode,
  pub(crate) contexts: &'a [StackingContextNode],
  pub(crate) results: &'a LayoutResults,
  pub(crate) document: &'a DocumentState<'a>,
  /// Pre-built inline layouts for the content tree; band trees build on the
  /// fly.
  pub(crate) inline: Option<&'a InlineMap<'a>>,
  /// The page window this walk paints through.
  pub(crate) window: Window,
  /// Whether this walk records marked content for the structure tree. A
  /// replayed artifact walks the same document untagged.
  pub(crate) tagged: bool,
  /// Path from the document root to this emitter's own root. Empty for the
  /// document; an inline box's subtree emitter carries the box's path, so the
  /// nodes it tags land where the structure tree walk looks for them.
  pub(crate) tag_prefix: Vec<usize>,
  /// Color transform from the `filter` properties of the enclosing stacking
  /// contexts, applied to every color this subtree paints.
  pub(crate) color_filter: Option<Rc<ColorFilter>>,
}

/// Names an image in an error: its URL, or that it came in as raw bytes.
#[cfg(feature = "images")]
fn image_label(src: &ImageSourceInput) -> &str {
  match src {
    ImageSourceInput::Url(url) => url,
    _ => "inline image bytes",
  }
}

/// Failures a page collects while emitting, raised once the surface is closed.
#[derive(Default)]
pub(crate) struct RenderIssues {
  /// Characters no registered font covered.
  pub(crate) uncovered: String,
  /// The first failure worth stopping for.
  pub(crate) failure: Option<PdfError>,
}

/// What every page of one document shares while it is emitted.
pub(crate) struct DocumentState<'a> {
  pub(crate) fonts: RefCell<FontMap>,
  /// Present when the document is tagged.
  pub(crate) tags: Option<RefCell<TagCollector>>,
  /// What the pages could not draw. Collected rather than raised on the spot:
  /// the surface has open transforms and clips mid-page, and unwinding past
  /// them would leave it unbalanced.
  pub(crate) issues: RefCell<RenderIssues>,
  /// The document's default language. A node declaring a different one has
  /// its content marked with that language, which is how a reader knows to
  /// switch voices mid-document.
  pub(crate) lang: Option<&'a str>,
  /// Present when the document emits fillable fields.
  pub(crate) form: Option<&'a FormOptions>,
}

impl<'a> DocumentState<'a> {
  pub(crate) fn new(tagged: bool, lang: Option<&'a str>, form: Option<&'a FormOptions>) -> Self {
    Self {
      fonts: RefCell::new(FontMap::default()),
      tags: tagged.then(RefCell::default),
      issues: RefCell::default(),
      lang,
      form,
    }
  }

  /// The error the pages left behind, if any: what failed outright, else the
  /// characters no font covered.
  pub(crate) fn into_error(self) -> Option<PdfError> {
    let issues = self.issues.into_inner();

    if issues.failure.is_some() || issues.uncovered.is_empty() {
      return issues.failure;
    }
    let named = issues
      .uncovered
      .chars()
      .map(|character| format!("{character} (U+{:04X})", character as u32))
      .collect::<Vec<_>>()
      .join(", ");

    Some(PdfError::MissingGlyphs(named))
  }
}

impl Emitter<'_> {
  /// The filter chain as colors, keeping the first function a PDF cannot
  /// express.
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

  /// Whether the run's line belongs to another page. The one ownership test
  /// every inline paint pass shares: glyphs, shadows, decorations, boxes and
  /// background fragments all key on it.
  fn window_disowns_run(&self, run: &PositionedInlineRun, layout: Layout, y: f32) -> bool {
    run.glyph_run.glyphs.first().is_some_and(|glyph| {
      self
        .window
        .disowns_line(y + run.glyph_offset(layout).y + glyph.y)
    })
  }

  /// The marked-content identifiers this walk records into, if it tags.
  fn tags(&self) -> Option<&RefCell<TagCollector>> {
    self.tagged.then_some(self.document.tags.as_ref()?)
  }

  /// The marked-content tag a node's own content opens.
  ///
  /// A node whose language differs from the document's carries it on a `Span`,
  /// which is how PDF records a language change. Content in the document's own
  /// language needs nothing: the catalog already declares it.
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
  pub(crate) fn emit_context(
    &mut self,
    id: usize,
    parent: Affine,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let Some(context) = self.contexts.get(id) else {
      return Ok(());
    };

    let outer_filter = self.color_filter.clone();

    if let Some(node) = context
      .root()
      .and_then(|paint| self.root.node_at_path(&paint.path))
    {
      self.color_filter = self.composed_filter(outer_filter.as_deref(), &node.context.style.filter);
    }

    let outer_window = self.window;
    let (child_frame, root_state) = match context.root() {
      Some(paint) => self.emit_box(paint, parent, surface)?,
      None => (parent, BoxState::default()),
    };

    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(paint) => {
            // Skipping a node that paints outside the window only saves work;
            // the page's own clip would drop it anyway. A context's root is
            // never skipped this way, because the clip it opens decides what
            // its descendants are allowed to emit.
            if self.window.excludes_bounds(paint.paint_bounds) {
              continue;
            }
            let (_, state) = self.emit_box(paint, child_frame, surface)?;
            self.finish_box(state, surface);
          }
          PaintItemKind::Context(child) => {
            let excluded = self
              .contexts
              .get(*child)
              .is_some_and(|ctx| self.window.excludes_bounds(ctx.paint_bounds()));
            if !excluded {
              self.emit_context(*child, child_frame, surface)?;
            }
          }
        }
      }
    }
    self.finish_box(root_state, surface);
    self.window = outer_window;
    self.color_filter = outer_filter;
    Ok(())
  }

  /// Emits one node's background and own content. Returns the frame the node's
  /// children sit in and how many transforms were pushed onto the surface.
  fn emit_box(
    &mut self,
    paint: &NodePaint,
    parent: Affine,
    surface: &mut Surface,
  ) -> Result<(Affine, BoxState), PdfError> {
    let Some(node) = self.root.node_at_path(&paint.path) else {
      return Ok((parent, BoxState::default()));
    };
    let Ok(layout) = self.results.layout(paint.node_id) else {
      return Ok((parent, BoxState::default()));
    };

    let style = &node.context.style;
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
      surface
        .push_opacity(NormalizedF32::new(opacity.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE));
      pushed += 1;
    }

    let relative = parent.invert().unwrap_or(Affine::IDENTITY) * paint.transform;
    let (x, y) = if relative.only_translation() {
      (relative.x, relative.y)
    } else {
      let cols = relative.to_cols_array();

      surface.push_transform(&Transform::from_row(
        cols[0], cols[1], cols[2], cols[3], cols[4], cols[5],
      ));
      pushed += 1;
      (0.0, 0.0)
    };
    let frame = if relative.only_translation() {
      parent
    } else {
      parent * relative
    };
    // `box-decoration-break: clone`: the fragment of the box on this page
    // paints its own complete decorations (paint-only; cloned padding does not
    // reserve layout space). `slice` needs nothing — the page window slices
    // the full-box decorations, which is exactly the sliced rendering.
    let (deco_y, deco_size) = if style.box_decoration_break == BoxDecorationBreak::Clone
      && let Some((window_top, window_bottom)) = self.window.y
    {
      let top = y.max(window_top);
      let bottom = (y + layout.size.height).min(window_bottom);

      (
        top,
        Size {
          width: layout.size.width,
          height: (bottom - top).max(0.0),
        },
      )
    } else {
      (y, layout.size)
    };
    let border = BorderProperties::from_context(&node.context, deco_size, layout.border);

    // A mask covers the element and its descendants, so it is pushed with the
    // rest of the box's state and popped with it.
    if let Some(mask) = self.mask(node, layout.size, (x, y), surface) {
      surface.push_mask(mask);
      pushed += 1;
    }

    // `clip-path` clips the element itself, decorations included, so it goes on
    // before anything is painted.
    if let Some(shape) = &style.clip_path
      && let Some(commands) = clip_shape_commands(shape, &node.context, layout.size)
    {
      // A shape that resolves to no area clips everything away, so a missing
      // path becomes an empty region rather than no clip at all.
      let path = krilla_path(&commands, x, y).or_else(|| empty_path(x, y));

      if let Some(path) = path {
        let rule = match shape.fill_rule().unwrap_or(style.clip_rule) {
          CoreFillRule::EvenOdd => FillRule::EvenOdd,
          _ => FillRule::NonZero,
        };

        surface.push_clip_path(&path, &rule);
        pushed += 1;
      }
    }
    let shadows =
      self.filtered_shadows(BoxPainter::fragment(&node.context, layout, deco_size).shadows());
    let (inset, outer) = (shadows.inset, shadows.outer);

    let deco_layout = Layout {
      size: deco_size,
      ..layout
    };

    self.shadows(&outer, &border, deco_layout, (x, deco_y), surface, false);
    // `background-clip: border-area` paints the fills over the borders,
    // clipped to the border ring; every other clip paints them underneath.
    // `background-clip` picks the shape a background fills, never when it
    // paints: the border draws over the ring, as it does in Blink.
    self.emit_background(node, deco_layout, x, deco_y, surface);
    self.emit_background_layers(node, deco_layout, x, deco_y, surface);
    self.shadows(&inset, &border, deco_layout, (x, deco_y), surface, true);
    self.emit_borders(&border, x, deco_y, deco_size, surface);
    // Children and own content clip to the (rounded) padding box when overflow
    // is hidden; without radius a per-axis overflow leaves the visible axis
    // unbounded. Counted on its own: the outline paints outside this clip but
    // inside everything else the box pushed.
    let mut overflow_clip = 0;

    if style.clips_overflow() {
      // A clip keeps content off the page but not out of the text layer, so
      // what it cuts away must never be emitted. Only a translated frame maps
      // the box onto the window's axis.
      if relative.only_translation() {
        self.window.narrow(y, y + layout.size.height);
      }
      let clip_border = BorderProperties::from_context(&node.context, layout.size, layout.border);
      let path = if clip_border.is_zero() {
        overflow_clip_rect(style, layout, x, y)
      } else {
        let clip = ClipBox::padding_box(clip_border, layout);
        let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

        clip
          .border
          .append_mask_commands(&mut commands, clip.size, clip.offset);
        krilla_path(&commands, x, y)
      };
      if let Some(path) = path {
        surface.push_clip_path(&path, &FillRule::NonZero);
        overflow_clip += 1;
      }
    }

    let tagged = self.tagged && has_own_content(node);

    if tagged {
      if decorative_image(node) || self.inside_form_control(&paint.path) {
        surface.start_tagged(ContentTag::Artifact(Artifact::new(
          ArtifactType::Other,
          None,
        )));
      } else {
        let identifier = surface.start_tagged(self.content_tag(node));

        if let Some(tags) = self.tags() {
          tags
            .borrow_mut()
            .record(&self.tag_path(&paint.path), identifier);
        }
      }
    }
    self.emit_own_content(node, paint.node_id, layout, x, y, surface)?;
    if tagged {
      surface.end_tagged();
    }

    Ok((
      frame,
      BoxState {
        pushed,
        overflow_clip,
        // CSS 2.1 Appendix E paints the outline last. The caller pops only the
        // overflow clip first, so the outline lands above the content and
        // outside that clip, but still under the box's transform, opacity,
        // mask and blend.
        outline: self.pending_outline(node, deco_layout, deco_size, x, deco_y),
      },
    ))
  }

  /// Finishes a box: leaves its overflow clip, paints the outline above
  /// everything the box and its children drew, then pops the rest.
  fn finish_box(&self, state: BoxState, surface: &mut Surface) {
    pop_transforms(surface, state.overflow_clip);
    self.paint_outline(state.outline.as_ref(), surface);
    pop_transforms(surface, state.pushed);
  }

  fn emit_background(
    &self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let mut device = SurfaceDevice {
      surface,
      filter: self.color_filter.as_deref(),
      // An artifact opens only once something actually paints, because marked
      // content does not nest and an empty region would still have to close.
      artifact: self.tagged,
    };

    BoxPainter::new(&node.context, layout).background_color(CorePoint { x, y }, &mut device);
  }

  /// Paints `background-image` layers, bottom layer first, clipped to the
  /// `background-clip` box. Gradient layers paint as shadings; `url()` layers
  /// rasterize when the `images` feature is on. `background-origin` sets the
  /// positioning area the size and position resolve against; a repeating
  /// layer still tiles across the whole clip region.
  fn emit_background_layers(
    &self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let style = &node.context.style;
    let size = layout.size;
    let Some(images) = style.background_image.as_deref() else {
      return;
    };
    if !images.iter().any(BackgroundImage::paints) {
      return;
    }
    let Some(shape) = BoxPainter::new(&node.context, layout).background_clip_shape() else {
      return;
    };
    let clip = match &shape {
      FillShape::Rect(size) => {
        KrillaRect::from_xywh(x, y, size.width, size.height).and_then(rect_path)
      }
      _ => krilla_path(&shape.to_commands(), x, y),
    };
    let Some(clip) = clip else {
      return;
    };
    let rule = match shape.rule() {
      CoreFillRule::EvenOdd => FillRule::EvenOdd,
      _ => FillRule::NonZero,
    };
    let (origin_offset, area) = background_origin_area(style.background_origin, layout);
    let artifact = self.start_artifact(surface);

    surface.push_clip_path(&clip, &rule);
    for (index, image) in images.iter().enumerate().rev() {
      let placement = Placement::resolve(
        area,
        cycled(&style.background_size, index),
        cycled(&style.background_position, index),
        cycled(&style.background_repeat, index),
        layer_intrinsic(image, &node.context),
        &node.context,
      );
      let blend = cycled(&style.background_blend_mode, index);
      let blended = blend != BlendMode::Normal;

      if blended {
        surface.push_blend_mode(krilla_blend(blend));
      }
      let at = (x + origin_offset.x, y + origin_offset.y);

      if placement.tiles {
        self.tiled_layer(image, node, &placement, size, (x, y), at, surface);
      } else {
        self.background_layer(
          image,
          node,
          placement.tile,
          at.0 + placement.origin.0,
          at.1 + placement.origin.1,
          surface,
        );
      }
      if blended {
        surface.pop();
      }
    }
    surface.pop();
    if artifact {
      surface.end_tagged();
    }
  }

  /// Draws one tile into a pattern and fills the layer's area with it, so a
  /// repeated layer costs one shading instead of one per tile. The filled rect
  /// covers the paint box at `rect_at`; the first tile hangs off `anchor`,
  /// which `background-origin` may inset from the paint box.
  #[allow(clippy::too_many_arguments)]
  fn tiled_layer(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    placement: &Placement,
    size: Size<f32>,
    rect_at: (f32, f32),
    anchor: (f32, f32),
    surface: &mut Surface,
  ) {
    let stream = {
      let mut builder = surface.stream_builder();
      let mut tile = builder.surface();

      self.background_layer(image, node, placement.tile, 0.0, 0.0, &mut tile);
      tile.finish();
      builder.finish()
    };
    let Some(path) =
      KrillaRect::from_xywh(rect_at.0, rect_at.1, size.width, size.height).and_then(rect_path)
    else {
      return;
    };

    surface.set_fill(Some(Fill {
      paint: Pattern {
        stream,
        transform: Transform::from_translate(
          anchor.0 + placement.origin.0,
          anchor.1 + placement.origin.1,
        ),
        width: placement.step.0,
        height: placement.step.1,
      }
      .into(),
      opacity: NormalizedF32::ONE,
      rule: FillRule::NonZero,
    }));
    surface.draw_path(&path);
  }

  fn background_layer(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    size: Size<f32>,
    x: f32,
    y: f32,
    surface: &mut Surface,
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
        rasterized_image(&source, &node.context, (w, h), self.color_filter.as_deref()),
      ) else {
        return;
      };
      let Some(target) = KrillaSize::from_wh(w, h) else {
        return;
      };

      surface.push_transform(&Transform::from_translate(x, y));
      surface.draw_image(krilla_image, target);
      surface.pop();
      return;
    }
    let Some(paint) = self.gradient_paint(image, node, size, x, y) else {
      return;
    };
    let Some(path) = KrillaRect::from_xywh(x, y, w, h).and_then(rect_path) else {
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
  fn gradient_paint(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    size: Size<f32>,
    x: f32,
    y: f32,
  ) -> Option<Paint> {
    let (w, h) = (size.width, size.height);
    let sizing = &node.context.sizing;
    let current_color = node.context.current_color;

    let paint: Paint = match image {
      BackgroundImage::Linear(gradient) => {
        let tile =
          LinearGradientTile::new(gradient, w as u32, h as u32, sizing, current_color, false);
        let resolved = self.filtered_stops(resolve_stops_along_axis(
          &gradient.stops,
          tile.axis_length.max(1e-6),
          sizing,
          current_color,
        ));
        if resolved.is_empty() {
          return None;
        }
        let max_extent = tile.axis_length / 2.0;
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let point_at = |t: f32| {
          (
            cx + (t - max_extent) * tile.dir_x,
            cy + (t - max_extent) * tile.dir_y,
          )
        };
        let (t0, t1, base, span) = if gradient.repeating {
          let first = resolved.first().map_or(0.0, |s| s.position);
          let last = resolved.last().map_or(tile.axis_length, |s| s.position);
          (first, last, first, (last - first).max(1e-6))
        } else {
          (0.0, tile.axis_length, 0.0, tile.axis_length.max(1e-6))
        };
        let (x1, y1) = point_at(t0);
        let (x2, y2) = point_at(t1);

        KrillaLinearGradient {
          x1,
          y1,
          x2,
          y2,
          transform: Transform::identity(),
          spread_method: spread(gradient.repeating),
          stops: krilla_stops(&resolved, base, span),
          anti_alias: false,
        }
        .into()
      }
      BackgroundImage::Radial(gradient) => {
        let tile =
          RadialGradientTile::new(gradient, w as u32, h as u32, sizing, current_color, false);
        let resolved = self.filtered_stops(resolve_stops_along_axis(
          &gradient.stops,
          tile.radius_scale.max(1e-6),
          sizing,
          current_color,
        ));
        if resolved.is_empty() {
          return None;
        }
        let radius_x = tile.inv_radius_x.max(1e-6).recip();
        let radius_y = tile.inv_radius_y.max(1e-6).recip();
        let extent = tile.radius_scale.max(1e-6);
        // PDF radial shadings cannot repeat, so a repeating gradient expands
        // its period across the full radius instead of relying on the spread.
        let stops = if gradient.repeating {
          expanded_radial_stops(&resolved, extent)
        } else {
          krilla_stops(&resolved, 0.0, extent)
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
          transform: Transform::from_row(scale_x, 0.0, 0.0, scale_y, x + tile.cx, y + tile.cy),
          spread_method: SpreadMethod::Pad,
          stops,
          anti_alias: false,
        }
        .into()
      }
      BackgroundImage::Conic(gradient) => {
        let tile =
          ConicGradientTile::new(gradient, w as u32, h as u32, sizing, current_color, false);
        let lut_len = tile.color_lut.len();
        if lut_len == 0 {
          return None;
        }
        const SWEEP_STOPS: usize = 64;
        let stops = (0..=SWEEP_STOPS)
          .map(|i| {
            let t = i as f32 / SWEEP_STOPS as f32;
            let index =
              tile.lut_index_for_adjusted_angle_with_len(t * core::f32::consts::TAU, lut_len);
            let color = tile.color_lut[index].demultiply();

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
          transform: Transform::from_rotate_at(tile.start_rad.to_degrees() - 90.0, ccx, ccy),
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

  /// Paints one side of a box's shadows inside its own artifact sequence, so
  /// the fills stay out of the structure tree like the other decorations.
  fn shadows(
    &self,
    shadows: &[SizedShadow],
    border: &BorderProperties,
    layout: Layout,
    at: (f32, f32),
    surface: &mut Surface,
    inset: bool,
  ) {
    if shadows.is_empty() {
      return;
    }
    let artifact = self.start_artifact(surface);

    if inset {
      emit_inset_shadows(shadows, &ClipBox::padding_box(*border, layout), at, surface);
    } else {
      emit_outer_shadows(shadows, border, layout.size, at, surface);
    }
    if artifact {
      surface.end_tagged();
    }
  }

  /// A node's shadows resolved against its box, split into inset and outer.
  /// The shadow color goes through the subtree's `filter` like every other
  /// color the element paints.
  /// Applies this subtree's `filter` to each shadow colour.
  fn filtered_shadows(&self, shadows: BoxShadows) -> BoxShadows {
    let recolor = |shadow: SizedShadow| SizedShadow {
      color: Color(self.filtered(shadow.color)),
      ..shadow
    };

    BoxShadows {
      inset: shadows.inset.into_iter().map(recolor).collect(),
      outer: shadows.outer.into_iter().map(recolor).collect(),
    }
  }

  /// Builds the soft mask for `mask-image`, drawing its layers into their own
  /// stream. The layers are alpha masks, which is what `mask-mode` resolves to
  /// for an image source.
  ///
  /// The element's own `filter` stays off the mask: it already applies to the
  /// content the mask covers, and applying it to both would compound, so
  /// `opacity(0.5)` behind a mask would leave a quarter of the alpha.
  fn mask(
    &mut self,
    node: &RenderNode,
    size: Size<f32>,
    at: (f32, f32),
    surface: &mut Surface,
  ) -> Option<Mask> {
    let images = node.context.style.mask_image.as_deref()?;

    if !images.iter().any(BackgroundImage::paints) {
      return None;
    }
    let filter = self.color_filter.take();
    let style = &node.context.style;
    let stream = {
      let mut builder = surface.stream_builder();
      let mut content = builder.surface();

      for (index, image) in images.iter().enumerate().rev() {
        let placement = Placement::resolve(
          size,
          cycled(&style.mask_size, index),
          cycled(&style.mask_position, index),
          cycled(&style.mask_repeat, index),
          layer_intrinsic(image, &node.context),
          &node.context,
        );

        if placement.tiles {
          self.tiled_layer(image, node, &placement, size, at, at, &mut content);
        } else {
          self.background_layer(
            image,
            node,
            placement.tile,
            at.0 + placement.origin.0,
            at.1 + placement.origin.1,
            &mut content,
          );
        }
      }
      content.finish();
      builder.finish()
    };

    self.color_filter = filter;
    Some(Mask::new(stream, MaskType::Alpha))
  }

  /// A color as this subtree's `filter` leaves it.
  fn filtered(&self, color: Color) -> [u8; 4] {
    match &self.color_filter {
      Some(filter) => filter.apply(color.0),
      None => color.0,
    }
  }

  /// Gradient stops as this subtree's `filter` leaves them.
  fn filtered_stops<S>(&self, mut resolved: S) -> S
  where
    S: AsMut<[ResolvedGradientStop]>,
  {
    if let Some(filter) = &self.color_filter {
      for stop in resolved.as_mut() {
        stop.color = filter.apply_color(stop.color);
      }
    }
    resolved
  }

  /// Opens an artifact sequence around a decoration when tagging is on, so it
  /// stays out of the structure tree. Returns whether one was opened.
  fn start_artifact(&self, surface: &mut Surface) -> bool {
    if !self.tagged {
      return false;
    }
    surface.start_tagged(ContentTag::Artifact(Artifact::new(
      ArtifactType::Other,
      None,
    )));
    true
  }

  /// Fills the border ring: one even-odd fill for a uniform color, per-side
  /// trapezoids clipped to the ring otherwise.
  // ponytail: dashed/dotted/double render as solid; port the stroke-based
  // patterns from takumi-svg when someone needs them.
  /// Draws the CSS `outline` as a ring around the border box, expanded outward
  /// by `outline-offset + outline-width`. It does not affect layout and reuses
  /// the border machinery, like the other backends.
  /// The outline the box will paint once its own state is popped, or `None`
  /// when it paints none. A transparent outline is a fill nobody sees, so it
  /// is skipped to keep the content stream shorter.
  fn pending_outline(
    &self,
    node: &RenderNode,
    layout: Layout,
    size: Size<f32>,
    x: f32,
    y: f32,
  ) -> Option<PendingOutline> {
    if node
      .context
      .style
      .outline_color
      .resolve(node.context.current_color)
      .0[3]
      == 0
    {
      return None;
    }

    Some(PendingOutline {
      outline: BoxPainter::fragment(&node.context, layout, size).outline()?,
      x,
      y,
    })
  }

  fn paint_outline(&self, pending: Option<&PendingOutline>, surface: &mut Surface) {
    let Some(pending) = pending else {
      return;
    };

    self.emit_borders(
      &pending.outline.border,
      pending.x - pending.outline.grow,
      pending.y - pending.outline.grow,
      pending.outline.size,
      surface,
    );
  }

  fn emit_borders(
    &self,
    border: &BorderProperties,
    x: f32,
    y: f32,
    size: Size<f32>,
    surface: &mut Surface,
  ) {
    if !border.has_visible_sides() {
      return;
    }
    let mut ring = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    border.append_border_ring_commands(&mut ring, size);
    let Some(ring_path) = krilla_path(&ring, x, y) else {
      return;
    };

    // The device opens its own artifact per fill, so a border that paints
    // nothing leaves no empty region behind.
    if paint_border(
      border,
      size,
      CorePoint { x, y },
      &mut SurfaceDevice {
        surface,
        filter: self.color_filter.as_deref(),
        artifact: self.tagged,
      },
    ) {
      return;
    }
    let mut sides = border.painted_sides().peekable();

    if sides.peek().is_none() {
      return;
    }
    let artifact = self.start_artifact(surface);
    // A collapsed border's sides are squared rectangles already inside the
    // ring, and the clip's antialiased edge leaks the page where two cells
    // meet.
    let clipped = !border.collapsed;

    if clipped {
      surface.push_clip_path(&ring_path, &FillRule::EvenOdd);
    }

    for side in sides {
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
        if let Some(path) = krilla_path(&polygon, x, y) {
          surface.set_fill(Some(fill_from_rgba(self.filtered(band.color), 1.0)));
          surface.draw_path(&path);
        }
      }
    }
    if clipped {
      surface.pop();
    }

    if artifact {
      surface.end_tagged();
    }
  }

  fn emit_own_content(
    &mut self,
    node: &RenderNode,
    node_id: NodeId,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    if node.should_create_inline_layout() {
      return self.emit_node_text(node, node_id, layout, x, y, surface);
    }
    if node.has_anonymous_text_item_child() {
      return Ok(());
    }
    match node.node.as_ref().map(|n| &n.kind) {
      Some(NodeKind::Text(_)) => self.emit_node_text(node, node_id, layout, x, y, surface),
      #[cfg(feature = "images")]
      Some(NodeKind::Image(image)) => {
        self.emit_image(image, &node.context, layout, x, y, surface);
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
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let content = layout.content_box_size();
    let offset = layout.content_box_offset();
    let (bx, by, w, h) = (x + offset.x, y + offset.y, content.width, content.height);
    if w <= 0.0 || h <= 0.0 {
      return;
    }
    let Ok(source) = image.src.resolve(context) else {
      return;
    };

    let (iw, ih) = {
      let (width, height) = source.size(&context.sizing);
      if width <= 0.0 || height <= 0.0 {
        return;
      }
      (width, height)
    };
    let content = Size {
      width: w,
      height: h,
    };
    let placement = place_replaced(
      context,
      content,
      Size {
        width: iw,
        height: ih,
      },
    );
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
      match self.drawable(
        image_label(&image.src),
        rasterized_image(&source, context, (dw, dh), self.color_filter.as_deref()),
      ) {
        Some(image) => Some(image),
        None => return,
      }
    } else {
      None
    };
    let ix = bx + placement.offset.x;
    let iy = by + placement.offset.y;

    let Some(size) = KrillaSize::from_wh(dw, dh) else {
      return;
    };
    let overflows = placement.overflows(content);

    if overflows {
      let Some(path) = KrillaRect::from_xywh(bx, by, w, h).and_then(rect_path) else {
        return;
      };

      surface.push_clip_path(&path, &FillRule::NonZero);
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
    if overflows {
      surface.pop();
    }
  }

  /// Draws a text-bearing box's runs, from the pre-built inline map when the
  /// node is in it (content tree) or built on the fly (band trees).
  fn emit_node_text(
    &mut self,
    node: &RenderNode,
    node_id: NodeId,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    if let Some(prepared) = self.inline.and_then(|map| map.get(&node_id)) {
      let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);

      return self.draw_runs(
        node,
        &prepared.runs,
        &prepared.built,
        layout,
        x,
        y,
        &font_style,
        surface,
      );
    }
    let context = &node.context;
    let Some(items) = node_inline_items(node) else {
      return Ok(());
    };
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((built, runs)) = build_inline_runs(items, &font_style, context, layout)? else {
      return Ok(());
    };

    self.draw_runs(node, &runs, &built, layout, x, y, &font_style, surface)
  }

  #[allow(clippy::too_many_arguments)]
  fn draw_runs(
    &mut self,
    node: &RenderNode,
    runs: &InlineRunLayout,
    built: &BuiltInlineLayout<'_>,
    layout: Layout,
    x: f32,
    y: f32,
    font_style: &SizedFontStyle,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    // Inline-span backgrounds fill under every glyph of the formatting context.
    // A fragment paints only on the page that owns its line, like the glyph
    // pass, so a page cut leaves no background sliver on the neighbor page.
    for fragment in &runs.background_fragments {
      if self.window.disowns_line(y + fragment.baseline) {
        continue;
      }
      let Some(path) = krilla_path(&inline_background_path(fragment), x, y) else {
        continue;
      };

      surface.set_fill(Some(fill_from_rgba(
        self.filtered(fragment.color),
        fragment.opacity,
      )));
      surface.draw_path(&path);
    }

    // text-shadow paints below the glyphs, later-listed shadows lowest. PDF
    // has no blur operator, so a blurred text shadow draws sharp.
    for shadow in font_style.painted_text_shadows() {
      self.glyph_pass(
        runs,
        built,
        layout,
        x + shadow.offset_x,
        y + shadow.offset_y,
        Some(shadow.color),
        surface,
      );
    }
    let text_fills = self.text_clip_fills(node, layout, x, y, surface);

    for run in &runs.runs {
      let shaped = &run.glyph_run;
      if shaped.glyphs.is_empty() {
        continue;
      }
      let Some(font) = self.cached_font(shaped) else {
        continue;
      };
      if self.window_disowns_run(run, layout, y) {
        continue;
      }
      let offset = run.glyph_offset(layout);
      let decorations = run_decorations(
        shaped,
        &run.resolved_glyphs,
        layout,
        run.baseline_shift,
        run.transform(Affine::IDENTITY),
      );

      paint_run_decorations(
        &decorations,
        false,
        TextDecorationLines::empty(),
        CorePoint { x, y },
        &mut SurfaceDevice {
          surface,
          filter: self.color_filter.as_deref(),
          artifact: false,
        },
      );
      let run_text = built
        .text
        .get(shaped.text_range.clone())
        .unwrap_or_default();
      let glyphs = run_glyphs(
        shaped,
        run_text,
        &mut self.document.issues.borrow_mut().uncovered,
      );

      let color = shaped.brush.color;
      let fill = fill_from_rgba(self.filtered(color), shaped.brush.opacity);
      let origin = Point::from_xy(x + offset.x, y + offset.y);
      let oblique = self.push_oblique(shaped, origin, surface);

      // `background-clip: text` paints the background through the glyphs, under
      // the text's own (usually transparent) fill. Faux bold widens the glyph
      // itself, so the background has to fill the widened shape too.
      for background in &text_fills {
        surface.set_fill(Some(background.clone()));
        surface.set_stroke(background_stroke(shaped, background));
        // Outlined: text extraction keys on the text-showing operator, whatever
        // the rendering mode, so a second run of glyphs would put the text in
        // the text layer twice. Paths paint the same pixels and stay out of it.
        surface.draw_glyphs(
          origin,
          &glyphs,
          font.clone(),
          run_text,
          shaped.font_size,
          true,
        );
      }

      surface.set_fill(Some(fill.clone()));
      // `-webkit-text-stroke` strokes the glyph outlines around the fill, and
      // takes the run's own width over the faux bold one.
      let brush = &shaped.brush;

      surface.set_stroke(
        if brush.stroke_width > 0.0 && brush.stroke_color.0[3] != 0 {
          let stroke_fill = fill_from_rgba(self.filtered(brush.stroke_color), brush.opacity);

          Some(Stroke {
            paint: stroke_fill.paint,
            opacity: stroke_fill.opacity,
            width: brush.stroke_width,
            ..Stroke::default()
          })
        } else {
          synthetic_stroke(shaped, &fill)
        },
      );

      surface.draw_glyphs(origin, &glyphs, font, run_text, shaped.font_size, false);

      if oblique {
        surface.pop();
      }
      surface.set_stroke(None);
      paint_run_decorations(
        &decorations,
        true,
        TextDecorationLines::empty(),
        CorePoint { x, y },
        &mut SurfaceDevice {
          surface,
          filter: self.color_filter.as_deref(),
          artifact: false,
        },
      );
    }
    self.emit_inline_boxes(node, runs, built, layout, x, y, surface);
    Ok(())
  }

  /// Paints the inline layout's replaced boxes and nested container subtrees.
  #[allow(clippy::too_many_arguments)]
  fn emit_inline_boxes(
    &mut self,
    owner: &RenderNode,
    runs: &InlineRunLayout,
    built: &BuiltInlineLayout<'_>,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    // The caller opened a marked-content region for the text around these
    // boxes. Marked content does not nest, so each box closes it, takes a
    // region of its own, and hands it back.
    let owner_tagged = self.tagged && has_own_content(owner);

    for positioned in &runs.inline_boxes {
      let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(positioned.id as usize) else {
        continue;
      };
      let node = item.render_node;

      // An in-flow box belongs to the page that owns its line, like the glyph
      // runs beside it.
      if positioned.line_baseline.is_some_and(|baseline| {
        let absolute = y + layout.content_box_offset().y + baseline;
        self.window.disowns_line(absolute)
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

      // A container box runs its own emitter, which tags every node it walks.
      // A replaced one paints here, so it takes one region for the whole box.
      let box_tagged =
        cfg!(feature = "images") && self.tagged && matches!(paint, InlineBoxPaint::Replaced { .. });

      if owner_tagged {
        surface.end_tagged();
      }
      if let Some(path) = marker_target.as_ref() {
        let identifier = surface.start_tagged(self.content_tag(node));

        if let Some(tags) = self.tags() {
          tags.borrow_mut().record_label(path, identifier);
        }
      } else if box_tagged {
        self.start_tagged_node(node, surface);
      }
      // The box never reaches `emit_box`, so the state that would paint it
      // there is applied here: its own opacity, and its `filter` composed onto
      // the one the enclosing stacking contexts left.
      let opacity = node.context.style.opacity.0;
      let faded = opacity < 1.0;

      if faded {
        surface
          .push_opacity(NormalizedF32::new(opacity.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE));
      }
      let outer_filter = self.color_filter.clone();
      self.color_filter = self.composed_filter(outer_filter.as_deref(), &node.context.style.filter);

      let origin = (x + offset.x, y + offset.y);

      match paint {
        #[cfg(feature = "images")]
        InlineBoxPaint::Replaced {
          node,
          layout: box_layout,
        } => self.emit_inline_replaced(node, box_layout, origin, surface),
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
      if marker_tagged || box_tagged {
        surface.end_tagged();
      }
      if owner_tagged {
        self.start_tagged_node(owner, surface);
      }
    }
  }

  /// Paints a replaced inline box: its decorations, then its content.
  #[cfg(feature = "images")]
  fn emit_inline_replaced(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    origin: (f32, f32),
    surface: &mut Surface,
  ) {
    let (x, y) = origin;
    let border = BorderProperties::from_context(&node.context, layout.size, layout.border);
    let shadows = self.filtered_shadows(BoxPainter::new(&node.context, layout).shadows());
    let (inset, outer) = (shadows.inset, shadows.outer);

    self.shadows(&outer, &border, layout, (x, y), surface, false);
    self.emit_background(node, layout, x, y, surface);
    self.emit_background_layers(node, layout, x, y, surface);
    self.shadows(&inset, &border, layout, (x, y), surface, true);
    self.emit_borders(&border, x, y, layout.size, surface);

    if let Some(NodeKind::Image(image)) = node.node.as_ref().map(|source| &source.kind) {
      self.emit_image(image, &node.context, layout, x, y, surface);
    }
    self.paint_outline(
      self
        .pending_outline(node, layout, layout.size, x, y)
        .as_ref(),
      surface,
    );
  }

  /// Paints an inline-level container from the scene it carries. The box is not
  /// in the paint list, so it needs a stacking context of its own.
  fn emit_inline_subtree(
    &mut self,
    subtree: Box<InlineSubtree>,
    node: &RenderNode,
    origin: (f32, f32),
    surface: &mut Surface,
  ) {
    if subtree.size.height <= 0.0 {
      return;
    }
    let Ok(contexts) = build_stacking_contexts(
      &subtree.root,
      &subtree.results,
      NodeId::ROOT,
      Affine::IDENTITY,
      (Some(subtree.size.width), Some(subtree.size.height)),
    ) else {
      return;
    };
    // The subtree root is a clone of `node`, so the box's own path is the
    // prefix that puts the subtree's nodes back on the document tree.
    let mut box_path = Vec::new();
    let tagged = self.tagged && node_path(self.root, node, &mut box_path);
    let tag_prefix = self.tag_path(&box_path);
    let mut emitter = Emitter {
      root: &subtree.root,
      contexts: &contexts,
      results: &subtree.results,
      document: self.document,
      inline: None,
      window: Window::default(),
      tagged,
      tag_prefix,
      color_filter: self.color_filter.clone(),
    };
    let x = origin.0 + subtree.margin_offset.x;
    let y = origin.1 + subtree.margin_offset.y;

    surface.push_transform(&Transform::from_translate(x, y));
    let _ = emitter.emit_context(0, Affine::IDENTITY, surface);
    surface.pop();
  }

  /// Opens a marked-content region for a node the paint list never visited, so
  /// its content still reaches the structure tree.
  fn start_tagged_node(&self, node: &RenderNode, surface: &mut Surface) {
    if decorative_image(node) {
      surface.start_tagged(ContentTag::Artifact(Artifact::new(
        ArtifactType::Other,
        None,
      )));
      return;
    }
    let identifier = surface.start_tagged(self.content_tag(node));
    let mut path = Vec::new();

    if let Some(tags) = self.tags()
      && node_path(self.root, node, &mut path)
    {
      tags.borrow_mut().record(&self.tag_path(&path), identifier);
    }
  }

  /// Whether this box sits inside a form control. PDF/UA admits one child
  /// under a `Form` element, the widget annotation, so everything the control
  /// paints is an artifact.
  fn inside_form_control(&self, path: &[usize]) -> bool {
    self.document.form.is_some()
      && (0..=path.len()).any(|depth| {
        self
          .root
          .node_at_path(&path[..depth])
          .and_then(|node| node.node.as_ref())
          .and_then(Node::tag_name)
          .is_some_and(is_form_control)
      })
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

  /// Tag target for a generated marker: its nearest `display: list-item`
  /// ancestor, whose `Lbl` holds the label.
  fn marker_tag_target(&self, owner: &RenderNode) -> Option<Vec<usize>> {
    let mut owner_path = Vec::new();

    if !node_path(self.root, owner, &mut owner_path) {
      return None;
    }
    let mut current = self.root;
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

  /// One image layer drawn into a pattern, so glyphs can be filled with it.
  fn image_pattern(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    tile: Size<f32>,
    at: (f32, f32),
    surface: &mut Surface,
  ) -> Option<Paint> {
    let stream = {
      let mut builder = surface.stream_builder();
      let mut inner = builder.surface();

      self.background_layer(image, node, tile, 0.0, 0.0, &mut inner);
      inner.finish();
      builder.finish()
    };

    (tile.width > 0.0 && tile.height > 0.0).then(|| {
      Pattern {
        stream,
        transform: Transform::from_translate(at.0, at.1),
        width: tile.width,
        height: tile.height,
      }
      .into()
    })
  }

  /// The fills a `background-clip: text` box paints through its glyphs:
  /// the background color, then each gradient layer bottom-up, anchored to the
  /// box the way the box background would be. Empty for every other clip.
  fn text_clip_fills(
    &self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Vec<Fill> {
    let style = &node.context.style;

    if style.background_clip != BackgroundClip::Text {
      return Vec::new();
    }
    let mut fills = Vec::new();
    let color = style.background_color.resolve(node.context.current_color);

    if color.0[3] != 0 {
      fills.push(fill_from_rgba(self.filtered(color), 1.0));
    }
    let (origin_offset, area) = background_origin_area(style.background_origin, layout);

    for (index, image) in style
      .background_image
      .as_deref()
      .unwrap_or_default()
      .iter()
      .enumerate()
      .rev()
    {
      let placement = Placement::resolve(
        area,
        cycled(&style.background_size, index),
        cycled(&style.background_position, index),
        cycled(&style.background_repeat, index),
        layer_intrinsic(image, &node.context),
        &node.context,
      );
      // ponytail: one tile per layer; a repeating gradient behind text would
      // need a pattern paint here.
      let (tile_x, tile_y) = (
        x + origin_offset.x + placement.origin.0,
        y + origin_offset.y + placement.origin.1,
      );
      // An image layer has no paint of its own, so it draws into a pattern the
      // glyphs can be filled with, the way a tiled background already does.
      let paint = match image {
        BackgroundImage::Url(_) => {
          self.image_pattern(image, node, placement.tile, (tile_x, tile_y), surface)
        }
        _ => self.gradient_paint(image, node, placement.tile, tile_x, tile_y),
      };
      let Some(paint) = paint else {
        continue;
      };

      fills.push(Fill {
        paint,
        opacity: NormalizedF32::ONE,
        rule: FillRule::NonZero,
      });
    }
    fills
  }

  /// Draws every run's glyphs once, in `color` when set, with no decorations:
  /// the shadow passes under the real text.
  #[allow(clippy::too_many_arguments)]
  fn glyph_pass(
    &mut self,
    runs: &InlineRunLayout,
    built: &BuiltInlineLayout<'_>,
    layout: Layout,
    x: f32,
    y: f32,
    color: Option<Color>,
    surface: &mut Surface,
  ) {
    for run in &runs.runs {
      let shaped = &run.glyph_run;
      if shaped.glyphs.is_empty() {
        continue;
      }
      let Some(font) = self.cached_font(shaped) else {
        continue;
      };
      if self.window_disowns_run(run, layout, y) {
        continue;
      }
      let offset = run.glyph_offset(layout);
      let run_text = built
        .text
        .get(shaped.text_range.clone())
        .unwrap_or_default();
      let glyphs = run_glyphs(
        shaped,
        run_text,
        &mut self.document.issues.borrow_mut().uncovered,
      );

      let color = color.unwrap_or(shaped.brush.color);

      let fill = fill_from_rgba(self.filtered(color), shaped.brush.opacity);
      let origin = Point::from_xy(x + offset.x, y + offset.y);

      surface.set_fill(Some(fill.clone()));
      surface.set_stroke(synthetic_stroke(shaped, &fill));

      let oblique = self.push_oblique(shaped, origin, surface);

      surface.draw_glyphs(origin, &glyphs, font, run_text, shaped.font_size, false);

      if oblique {
        surface.pop();
      }
      surface.set_stroke(None);
    }
  }

  /// Shears the text about its baseline, the faux oblique the raster renderer
  /// applies to glyph outlines. Surface space runs y down, so the sign flips.
  /// Returns whether the transform was pushed.
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
  fn cached_font(&mut self, shaped: &ShapedRun) -> Option<Font> {
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

/// Intrinsic sizing of a `url()` layer, which `background-size` resolves
/// against. Gradients have none.
#[cfg(feature = "images")]
fn layer_intrinsic(
  image: &BackgroundImage,
  context: &RenderContext,
) -> Option<takumi_core::style::IntrinsicSizing> {
  let BackgroundImage::Url(url) = image else {
    return None;
  };
  let source = resolve_image(url, context).ok()?;

  Some(source.intrinsic_sizing().scale(&context.sizing))
}

#[cfg(not(feature = "images"))]
fn layer_intrinsic(
  _image: &BackgroundImage,
  _context: &takumi_core::context::RenderContext,
) -> Option<takumi_core::style::IntrinsicSizing> {
  None
}

/// The positioning area `background-size` and `-position` resolve against,
/// per `background-origin`: an offset into the border box and its size.
fn background_origin_area(origin: BackgroundOrigin, layout: Layout) -> (CorePoint<f32>, Size<f32>) {
  let inset = |left: f32, right: f32, top: f32, bottom: f32| {
    (
      CorePoint { x: left, y: top },
      Size {
        width: (layout.size.width - left - right).max(0.0),
        height: (layout.size.height - top - bottom).max(0.0),
      },
    )
  };
  let border = layout.border;
  let padding = layout.padding;

  match origin {
    BackgroundOrigin::PaddingBox => inset(border.left, border.right, border.top, border.bottom),
    BackgroundOrigin::ContentBox => inset(
      border.left + padding.left,
      border.right + padding.right,
      border.top + padding.top,
      border.bottom + padding.bottom,
    ),
    _ => (CorePoint::ZERO, layout.size),
  }
}

/// Whether the node draws own content (text or an image), i.e. whether a
/// tagged content sequence around it would be non-empty.
fn has_own_content(node: &RenderNode) -> bool {
  if node.should_create_inline_layout() {
    return true;
  }
  if node.has_anonymous_text_item_child() {
    return false;
  }
  match node.node.as_ref().map(|n| &n.kind) {
    Some(NodeKind::Text(_)) => true,
    #[cfg(feature = "images")]
    Some(NodeKind::Image(_)) => true,
    _ => false,
  }
}

/// The PDF surface as a [`PaintDevice`], so the shared painting code can drive
/// it without knowing about krilla.
struct SurfaceDevice<'s, 'a> {
  surface: &'s mut Surface<'a>,
  filter: Option<&'s ColorFilter>,
  /// Whether painted content needs an artifact region around it.
  artifact: bool,
}

impl PaintDevice for SurfaceDevice<'_, '_> {
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    // A pure translation folds into the path, which keeps the content stream
    // free of a `cm` pair for every background fill.
    let flat = transform.only_translation();
    let (x, y) = if flat {
      (transform.x, transform.y)
    } else {
      (0.0, 0.0)
    };
    let path = match shape {
      FillShape::Rect(size) => {
        KrillaRect::from_xywh(x, y, size.width, size.height).and_then(rect_path)
      }
      _ => krilla_path(&shape.to_commands(), x, y),
    };
    let Some(path) = path else {
      return;
    };

    if !flat {
      let cols = transform.to_cols_array();

      self.surface.push_transform(&Transform::from_row(
        cols[0], cols[1], cols[2], cols[3], cols[4], cols[5],
      ));
    }
    let color = match self.filter {
      Some(filter) => filter.apply(color.0),
      None => color.0,
    };

    if self.artifact {
      self
        .surface
        .start_tagged(ContentTag::Artifact(Artifact::new(
          ArtifactType::Other,
          None,
        )));
    }
    self.surface.set_fill(Some(Fill {
      rule: match shape.rule() {
        CoreFillRule::EvenOdd => FillRule::EvenOdd,
        _ => FillRule::NonZero,
      },
      ..fill_from_rgba(color, 1.0)
    }));
    self.surface.draw_path(&path);
    if !flat {
      self.surface.pop();
    }
    if self.artifact {
      self.surface.end_tagged();
    }
  }

  fn stroke_shape(&mut self, shape: &FillShape, stroke: &StrokeStyle, transform: Affine) {
    if stroke.color.0[3] == 0 || stroke.width <= 0.0 {
      return;
    }
    let flat = transform.only_translation();
    let (x, y) = if flat {
      (transform.x, transform.y)
    } else {
      (0.0, 0.0)
    };
    let Some(path) = krilla_path(&shape.to_commands(), x, y) else {
      return;
    };

    if !flat {
      let cols = transform.to_cols_array();

      self.surface.push_transform(&Transform::from_row(
        cols[0], cols[1], cols[2], cols[3], cols[4], cols[5],
      ));
    }
    let color = match self.filter {
      Some(filter) => filter.apply(stroke.color.0),
      None => stroke.color.0,
    };

    if self.artifact {
      self
        .surface
        .start_tagged(ContentTag::Artifact(Artifact::new(
          ArtifactType::Other,
          None,
        )));
    }
    self.surface.set_fill(None);
    self.surface.set_stroke(Some(Stroke {
      paint: fill_from_rgba(color, 1.0).paint,
      width: stroke.width,
      line_cap: match stroke.round_cap {
        true => LineCap::Round,
        false => LineCap::Butt,
      },
      dash: stroke.dash.map(|intervals| StrokeDash {
        array: intervals.to_vec(),
        offset: 0.0,
      }),
      ..Stroke::default()
    }));
    self.surface.draw_path(&path);
    self.surface.set_stroke(None);
    if !flat {
      self.surface.pop();
    }
    if self.artifact {
      self.surface.end_tagged();
    }
  }
}

/// Fills `path` with the child indices leading from `root` to `target`,
/// matched by identity. An inline box arrives as a bare node reference, so the
/// key the tag collector wants has to be recovered from the tree.
fn node_path(root: &RenderNode, target: &RenderNode, path: &mut Vec<usize>) -> bool {
  if std::ptr::eq(root, target) {
    return true;
  }
  for (index, child) in root.children.iter().flatten().enumerate() {
    path.push(index);

    if node_path(child, target, path) {
      return true;
    }
    path.pop();
  }
  false
}

/// Whether the node is an image explicitly marked decorative (`alt=""`), so
/// its content is emitted as an artifact instead of a `Figure` element.
fn decorative_image(node: &RenderNode) -> bool {
  node.node.as_ref().is_some_and(|source| {
    source.tag_name().is_some_and(|name| name == "img") && source.alt() == Some("")
  })
}

/// The stroke that fakes bold for a face with no weight of its own to reach,
/// at the width the raster renderer emboldens with. Filling and stroking keeps
/// the run as text, so it stays selectable.
/// The stroke that widens the glyph coverage a `background-clip: text` fill
/// paints through: `-webkit-text-stroke`, faux bold, or whichever is wider.
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
