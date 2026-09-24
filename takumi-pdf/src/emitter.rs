//! The scene walker that emits boxes, text and images onto a krilla surface.

use std::{cell::RefCell, collections::HashMap, ptr, rc::Rc};

#[cfg(feature = "images")]
use takumi_core::{
  context::RenderContext,
  layout::{
    node::{ImageData, ImageSourceInput, NodeKind, resolve_image},
    replaced::place_replaced,
  },
  resources::image::ImageSource,
};
use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, NodeId, Point as CorePoint, Size},
  layout::{
    background::background_origin_box,
    border::BorderProperties,
    clip::clip_shape_commands,
    decoration::{ClipBox, OutlineGeometry},
    inline::{
      BuiltInlineLayout, InlineRunLayout, PositionedInlineRun, ProcessedInlineSpan, ShapedRun,
    },
    inline_box::{InlineBoxPaint, InlineSubtree, resolve_inline_box},
    tree::{LayoutResults, NodeOrigin, RenderNode},
  },
  paint::ConicGradientTile,
  painter::{
    BoxPainter, BoxShadows, FillShape, PaintDevice, StrokeStyle, paint_border,
    paint_run_decorations,
  },
  scene::{NodePaint, PaintItemKind, SceneRequest, StackingContextNode, build_scene},
  shadow::SizedShadow,
  style::{
    Affine, BackgroundClip, BackgroundImage, BackgroundOrigin, BlendMode, BoxDecorationBreak,
    Color, ComputedStyle, Display, Filter, Isolation, Lang, ResolvedGradientStop,
    TextDecorationLines,
  },
};

#[cfg(feature = "images")]
use crate::krilla::{geom::Size as KrillaSize, image::Image as KrillaImage};
#[cfg(feature = "images")]
use crate::paint::rasterized_image;
#[cfg(all(feature = "svg", feature = "images"))]
use crate::svg;
use crate::{
  background::{LayerLists, Placement, cycled},
  filter::{ColorFilter, filtered, unsupported_filter},
  glyph::{PdfGlyph, Uncovered, run_glyphs},
  inline::{InlineMap, visit_inline_layout},
  krilla::{
    Data,
    geom::{Path as KrillaPath, Point, Rect as KrillaRect, Transform},
    mask::{Mask, MaskType},
    num::NormalizedF32,
    paint::{
      Fill, FillRule, LineCap, LinearGradient as KrillaLinearGradient, Paint, Pattern,
      RadialGradient as KrillaRadialGradient, SpreadMethod, Stroke, StrokeDash, SweepGradient,
    },
    surface::Surface,
    tagging::{ContentTag, SpanTag},
    text::{Font, Tag},
  },
  options::{PT_PER_PX, PdfError},
  paint::{
    clip_box_path, draw_stream, empty_path, expanded_radial_stops, fill_from_rgba, krilla_blend,
    krilla_fill_rule, krilla_path, krilla_stop, krilla_stops, krilla_transform, normalized,
    overflow_clip_rect, pop_transforms, rect_path, shape_path, spread,
  },
  shadow::{emit_inset_shadows, emit_outer_shadows},
  tags::{ARTIFACT, TagCollector},
  tree::OwnContent,
  window::Window,
};

/// What a box left on the surface for its caller to unwind.
#[derive(Default)]
struct BoxState {
  /// Transforms, clips and layers to pop once the box and its children are done.
  pushed: usize,
  /// The `overflow` clip, popped before the outline so the outline escapes it.
  overflow_clip: usize,
  /// The outline, painted between the two pops.
  outline: Option<PendingOutline>,
}

/// An outline waiting for its box's state to be popped.
struct PendingOutline {
  outline: OutlineGeometry,
  x: f32,
  y: f32,
}

/// Blob identity, collection index, and the variation coordinates the run was shaped at.
type FontKey = (u64, u32, Vec<([u8; 4], u32)>);

/// Krilla fonts embedded so far, one per distinct instance.
type FontMap = HashMap<FontKey, Font>;

pub(crate) struct Emitter<'a> {
  pub(crate) root: &'a RenderNode,
  pub(crate) contexts: &'a [StackingContextNode],
  pub(crate) results: &'a LayoutResults,
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

  /// Emits one node's background and own content.
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
    let mut pushed = push_compositing(style, surface);
    let relative = parent.invert().unwrap_or(Affine::IDENTITY) * paint.transform;
    let (x, y, frame) = if relative.only_translation() {
      (relative.x, relative.y, parent)
    } else {
      surface.push_transform(&krilla_transform(relative.to_cols_array()));
      pushed += 1;
      (0.0, 0.0, parent * relative)
    };
    let (deco_y, deco_size) = self.decoration_window(style, y, layout.size);
    let deco_layout = Layout {
      size: deco_size,
      ..layout
    };

    pushed += self.push_mask_and_clip(node, layout, x, y, surface);
    self.emit_decorations(node, deco_layout, x, deco_y, surface);

    // Children and own content clip to the (rounded) padding box when overflow
    // is hidden; without radius a per-axis overflow leaves the visible axis
    // unbounded. Counted on its own: the outline paints outside this clip but
    // inside everything else the box pushed.
    let overflow_clip = if style.clips_overflow() {
      self.push_overflow_clip(node, layout, relative, x, y, surface)
    } else {
      0
    };

    self.emit_tagged_content(node, paint, layout, x, y, surface)?;

    Ok((
      frame,
      BoxState {
        pushed,
        overflow_clip,
        // CSS 2.1 Appendix E paints the outline last. The caller pops only the
        // overflow clip first, so the outline lands above the content and
        // outside that clip, but still under the box's transform, opacity,
        // mask and blend.
        outline: self.pending_outline(node, deco_layout, x, deco_y),
      },
    ))
  }

  /// `box-decoration-break: clone`: the fragment of the box on this page
  /// paints its own complete decorations (paint-only; cloned padding does not
  /// reserve layout space). `slice` needs nothing: the page window slices the
  /// full-box decorations, which is exactly the sliced rendering.
  fn decoration_window(&self, style: &ComputedStyle, y: f32, size: Size<f32>) -> (f32, Size<f32>) {
    if style.box_decoration_break == BoxDecorationBreak::Clone
      && let Some((window_top, window_bottom)) = self.window.y
    {
      let top = y.max(window_top);
      let bottom = (y + size.height).min(window_bottom);

      return (
        top,
        Size {
          width: size.width,
          height: (bottom - top).max(0.0),
        },
      );
    }

    (y, size)
  }

  /// Pushes the box's mask and `clip-path`, returning how many states went on.
  /// The mask covers the element and its descendants; `clip-path` clips the
  /// element itself, decorations included, so both go on before any paint.
  fn push_mask_and_clip(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> usize {
    let style = &node.context.style;
    let mut pushed = 0;

    if let Some(mask) = self.mask(node, layout.size, x, y, surface) {
      surface.push_mask(mask);
      pushed += 1;
    }

    if let Some(shape) = &style.clip_path
      && let Some(commands) = clip_shape_commands(shape, &node.context, layout.size)
    {
      // A shape that resolves to no area clips everything away, so a missing
      // path becomes an empty region rather than no clip at all.
      let path = krilla_path(&commands, x, y).or_else(|| empty_path(x, y));

      if let Some(path) = path {
        surface.push_clip_path(
          &path,
          &krilla_fill_rule(shape.fill_rule().unwrap_or(style.clip_rule)),
        );
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
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let painter = BoxPainter::new(&node.context, layout);
    let border = painter.border();
    let shadows = self.filtered_shadows(painter.shadows());

    if !shadows.outer.is_empty() {
      self.in_artifact(surface, |surface| {
        emit_outer_shadows(&shadows.outer, border, layout.size, (x, y), surface);
      });
    }
    painter.background_color(CorePoint { x, y }, &mut self.device(surface, self.tagged));
    self.emit_background_layers(node, &painter, layout, x, y, surface);
    if !shadows.inset.is_empty() {
      self.in_artifact(surface, |surface| {
        emit_inset_shadows(
          &shadows.inset,
          &ClipBox::padding_box(*border, layout),
          (x, y),
          surface,
        );
      });
    }
    self.emit_borders(border, x, y, layout.size, surface);
  }

  /// Clips children and own content to the padding box, returning how many
  /// states went on. A clip keeps content off the page but not out of the
  /// text layer, so what it cuts away must never be emitted; only a
  /// translated frame maps the box onto the window's axis.
  fn push_overflow_clip(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    relative: Affine,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> usize {
    if relative.only_translation() {
      self.window.narrow(y, y + layout.size.height);
    }
    let clip_border = BorderProperties::from_context(&node.context, layout.size, layout.border);
    let path = if clip_border.is_zero() {
      overflow_clip_rect(&node.context.style, layout, x, y)
    } else {
      clip_box_path(ClipBox::padding_box(clip_border, layout), x, y)
    };
    let Some(path) = path else {
      return 0;
    };

    surface.push_clip_path(&path, &FillRule::NonZero);
    1
  }

  /// Emits the box's own content inside its structure tag when tagging is on.
  fn emit_tagged_content(
    &mut self,
    node: &RenderNode,
    paint: &NodePaint,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let tagged = self.tagged && OwnContent::of(node).draws();

    if tagged {
      self.start_node_region(node, Some(&paint.path), surface);
    }
    self.emit_own_content(node, paint.node_id, layout, x, y, surface)?;
    if tagged {
      surface.end_tagged();
    }

    Ok(())
  }

  /// Finishes a box: leaves its overflow clip, paints the outline above
  /// everything the box and its children drew, then pops the rest.
  fn finish_box(&self, state: BoxState, surface: &mut Surface) {
    pop_transforms(surface, state.overflow_clip);
    self.paint_outline(state.outline.as_ref(), surface);
    pop_transforms(surface, state.pushed);
  }

  /// Paints `background-image` layers, bottom layer first, clipped to the
  /// `background-clip` box. Gradient layers paint as shadings; `url()` layers
  /// rasterize when the `images` feature is on. `background-origin` sets the
  /// positioning area the size and position resolve against; a repeating
  /// layer still tiles across the whole clip region.
  fn emit_background_layers(
    &self,
    node: &RenderNode,
    painter: &BoxPainter<'_>,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let style = &node.context.style;
    let Some(images) = style.background_image.as_deref() else {
      return;
    };
    if !images.iter().any(BackgroundImage::paints) {
      return;
    }
    let Some(shape) = painter.background_clip_shape() else {
      return;
    };
    let Some(clip) = shape_path(&shape, x, y) else {
      return;
    };
    let (origin_offset, area) = background_origin_area(style.background_origin, layout);
    let layers = LayerLists::background(style);

    self.in_artifact(surface, |surface| {
      surface.push_clip_path(&clip, &krilla_fill_rule(shape.rule()));
      for (index, image) in images.iter().enumerate().rev() {
        let placement = layers.placement(index, image, area, &node.context);
        let blend = cycled(&style.background_blend_mode, index);
        let blended = blend != BlendMode::Normal;

        if blended {
          surface.push_blend_mode(krilla_blend(blend));
        }
        self.layer(
          image,
          node,
          &placement,
          layout.size,
          (x, y),
          (x + origin_offset.x, y + origin_offset.y),
          surface,
          Transform::from_scale(PT_PER_PX, PT_PER_PX),
        );
        if blended {
          surface.pop();
        }
      }
      surface.pop();
    });
  }

  /// Draws one layer anchored at `anchor`. A tiling layer draws one tile into
  /// a pattern and fills the `size` rect at `rect_at` with it, so a repeated
  /// layer costs one shading instead of one per tile; `tile_space` is the
  /// space its tile draws in.
  #[allow(clippy::too_many_arguments)]
  fn layer(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    placement: &Placement,
    size: Size<f32>,
    rect_at: (f32, f32),
    anchor: (f32, f32),
    surface: &mut Surface,
    tile_space: Transform,
  ) {
    if !placement.tiles {
      self.background_layer(
        image,
        node,
        placement.tile,
        (anchor.0 + placement.origin.0, anchor.1 + placement.origin.1),
        surface,
        Transform::identity(),
      );
      return;
    }
    let stream = draw_stream(surface, |tile| {
      self.background_layer(image, node, placement.tile, (0.0, 0.0), tile, tile_space);
    });
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
    at: (f32, f32),
    surface: &mut Surface,
    pattern_space: Transform,
  ) {
    let (x, y) = at;
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
    let Some(paint) = self.gradient_paint(image, node, size, x, y, pattern_space) else {
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
  ///
  /// PDF 32000-1 8.7.3.1 resolves a pattern matrix against the default space of the stream the
  /// pattern is used in, which a nested stream does not inherit; `pattern_space` carries what
  /// krilla no longer composes.
  fn gradient_paint(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    size: Size<f32>,
    x: f32,
    y: f32,
    pattern_space: Transform,
  ) -> Option<Paint> {
    let (w, h) = (size.width, size.height);
    let sizing = &node.context.sizing;
    let current_color = node.context.current_color;

    let paint: Paint = match image {
      BackgroundImage::Linear(gradient) => {
        let mut geometry = gradient.resolve_geometry(w as u32, h as u32, sizing, current_color);
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
        let mut geometry = gradient.resolve_geometry(w as u32, h as u32, sizing, current_color);
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

  /// The shadows in the colors this subtree's `filter` leaves them.
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

  /// Builds the soft mask for `mask-image`, drawing its layers into their own stream.
  fn mask(
    &mut self,
    node: &RenderNode,
    size: Size<f32>,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Option<Mask> {
    let images = node.context.style.mask_image.as_deref()?;

    if !images.iter().any(BackgroundImage::paints) {
      return None;
    }
    let filter = self.color_filter.take();
    let layers = LayerLists::mask(&node.context.style);
    let stream = draw_stream(surface, |content| {
      for (index, image) in images.iter().enumerate().rev() {
        let placement = layers.placement(index, image, size, &node.context);

        self.layer(
          image,
          node,
          &placement,
          size,
          (x, y),
          (x, y),
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
    }
  }

  /// The CSS `outline` the box will paint once its own state is popped, a ring
  /// around the border box expanded outward by `outline-offset +
  /// outline-width`. A transparent outline is a fill nobody sees, so it is
  /// skipped to keep the content stream shorter.
  fn pending_outline(
    &self,
    node: &RenderNode,
    layout: Layout,
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
      outline: BoxPainter::new(&node.context, layout).outline()?,
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

  /// Fills the border ring: one even-odd fill for a uniform color, per-side
  /// trapezoids clipped to the ring otherwise.
  // ponytail: dashed/dotted/double render as solid; port the stroke-based
  // patterns from takumi-svg when someone needs them.
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
      &mut self.device(surface, self.tagged),
    ) {
      return;
    }
    let mut sides = border.painted_sides().peekable();

    if sides.peek().is_none() {
      return;
    }
    // A collapsed border's sides are squared rectangles already inside the
    // ring, and the clip's antialiased edge leaks the page where two cells
    // meet.
    let clipped = !border.collapsed;

    self.in_artifact(surface, |surface| {
      if clipped {
        surface.push_clip_path(&ring_path, &FillRule::EvenOdd);
      }
      for side in sides {
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
          if let Some(path) = krilla_path(&polygon, x, y) {
            surface.set_fill(Some(fill_from_rgba(self.filtered(band.color), 1.0)));
            surface.draw_path(&path);
          }
        }
      }
      if clipped {
        surface.pop();
      }
    });
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
    match OwnContent::of(node) {
      OwnContent::Text => self.emit_node_text(node, node_id, layout, x, y, surface),
      #[cfg(feature = "images")]
      OwnContent::Image(image) => {
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
    let (iw, ih) = source.size(&context.sizing);

    if iw <= 0.0 || ih <= 0.0 {
      return;
    }
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
      let Some(image) = self.drawable(
        image_label(&image.src),
        rasterized_image(&source, context, (dw, dh), self.color_filter.as_deref()),
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
    // A replaced element is trimmed to its content edge curve, so a corner radius clips the
    // image whether or not it overflows.
    let clip_border = BorderProperties::from_context(context, layout.size, layout.border);
    let clip_path = if clip_border.is_zero() {
      placement
        .overflows(content)
        .then(|| KrillaRect::from_xywh(bx, by, w, h).and_then(rect_path))
        .flatten()
    } else {
      clip_box_path(ClipBox::content_box(clip_border, layout), x, y)
    };

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
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    visit_inline_layout(
      self.inline,
      node,
      node_id,
      layout,
      |built, runs, font_style| {
        self.draw_runs(node, runs, built, layout, x, y, font_style, surface);
      },
    )?;
    Ok(())
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
  ) {
    // Inline-span backgrounds fill under every glyph of the formatting context.
    // A fragment paints only on the page that owns its line, like the glyph
    // pass, so a page cut leaves no background sliver on the neighbor page.
    for fragment in &runs.background_fragments {
      if self.window.disowns_line(y + fragment.baseline) {
        continue;
      }
      let Some(path) = krilla_path(&fragment.path(), x, y) else {
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
        x,
        y,
        (shadow.offset_x, shadow.offset_y),
        Some(shadow.color),
        surface,
      );
    }
    let text_fills = self.text_clip_fills(node, layout, x, y, surface);

    for run in &runs.runs {
      let Some(GlyphRun {
        font,
        text,
        glyphs,
        origin,
      }) = self.glyph_run(run, built, layout, x, y, y)
      else {
        continue;
      };
      let shaped = &run.glyph_run;
      let decorations = shaped.decorations(
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
        &mut self.device(surface, false),
      );
      let fill = fill_from_rgba(self.filtered(shaped.brush.color), shaped.brush.opacity);
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
        surface.draw_glyphs(origin, &glyphs, font.clone(), text, shaped.font_size, true);
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

      surface.draw_glyphs(origin, &glyphs, font, text, shaped.font_size, false);

      if oblique {
        surface.pop();
      }
      surface.set_stroke(None);
      paint_run_decorations(
        &decorations,
        true,
        TextDecorationLines::empty(),
        CorePoint { x, y },
        &mut self.device(surface, false),
      );
    }
    self.emit_inline_boxes(node, runs, built, layout, x, y, surface);
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
    let owner_tagged = self.tagged && OwnContent::of(owner).draws();

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

      let (box_x, box_y) = (x + offset.x, y + offset.y);

      match paint {
        #[cfg(feature = "images")]
        InlineBoxPaint::Replaced {
          node,
          layout: box_layout,
        } => self.emit_inline_replaced(
          node,
          box_layout,
          box_x,
          box_y,
          box_tagged && !box_wrapped,
          surface,
        ),
        #[cfg(not(feature = "images"))]
        InlineBoxPaint::Replaced { .. } => {}
        InlineBoxPaint::Container(subtree) => {
          self.emit_inline_subtree(subtree, node, box_x, box_y, surface)
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
    layout: Layout,
    x: f32,
    y: f32,
    tagged: bool,
    surface: &mut Surface,
  ) {
    self.emit_decorations(node, layout, x, y, surface);
    if tagged {
      self.start_tagged_node(node, surface);
    }
    if let Some(NodeKind::Image(image)) = node.node.as_ref().map(|source| &source.kind) {
      self.emit_image(image, &node.context, layout, x, y, surface);
    }
    if tagged {
      surface.end_tagged();
    }
    self.paint_outline(self.pending_outline(node, layout, x, y).as_ref(), surface);
  }

  /// Paints an inline-level container from the scene it carries.
  fn emit_inline_subtree(
    &mut self,
    subtree: Box<InlineSubtree>,
    node: &RenderNode,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    if subtree.size.height <= 0.0 {
      return;
    }
    let Ok(contexts) = build_scene(SceneRequest {
      root: &subtree.root,
      layout_results: &subtree.results,
      transform: Affine::IDENTITY,
      container_size: subtree.size.map(Some),
      paint_bounds: true,
    }) else {
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
    surface.push_transform(&Transform::from_translate(
      x + subtree.margin_offset.x,
      y + subtree.margin_offset.y,
    ));
    let _ = emitter.emit_context(0, Affine::IDENTITY, surface);
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
    let mut path = Vec::new();
    let found = node_path(self.root, node, &mut path);

    self.start_node_region(node, found.then_some(path.as_slice()), surface);
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
    let stream = draw_stream(surface, |inner| {
      self.background_layer(image, node, tile, (0.0, 0.0), inner, Transform::identity());
    });

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

  /// The fills painted through a `background-clip: text` box's glyphs.
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
    let layers = LayerLists::background(style);

    for (index, image) in style
      .background_image
      .as_deref()
      .unwrap_or_default()
      .iter()
      .enumerate()
      .rev()
    {
      let placement = layers.placement(index, image, area, &node.context);
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
        _ => self.gradient_paint(
          image,
          node,
          placement.tile,
          tile_x,
          tile_y,
          Transform::identity(),
        ),
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

  /// Draws every run's glyphs once, moved by `shift` and in `color` when set, with no
  /// decorations: the shadow passes under the real text. A run's page follows its unshifted line.
  #[allow(clippy::too_many_arguments)]
  fn glyph_pass(
    &mut self,
    runs: &InlineRunLayout,
    built: &BuiltInlineLayout<'_>,
    layout: Layout,
    x: f32,
    y: f32,
    shift: (f32, f32),
    color: Option<Color>,
    surface: &mut Surface,
  ) {
    for run in &runs.runs {
      let Some(GlyphRun {
        font,
        text,
        glyphs,
        origin,
      }) = self.glyph_run(run, built, layout, x + shift.0, y + shift.1, y)
      else {
        continue;
      };
      let shaped = &run.glyph_run;
      let fill = fill_from_rgba(
        self.filtered(color.unwrap_or(shaped.brush.color)),
        shaped.brush.opacity,
      );

      surface.set_fill(Some(fill.clone()));
      surface.set_stroke(synthetic_stroke(shaped, &fill));

      let oblique = self.push_oblique(shaped, origin, surface);

      surface.draw_glyphs(origin, &glyphs, font, text, shaped.font_size, false);

      if oblique {
        surface.pop();
      }
      surface.set_stroke(None);
    }
  }

  /// A run this page draws at `(x, y)`, or `None` when it has no glyphs, no
  /// font, or its line at `line_y` belongs to another page.
  fn glyph_run<'r>(
    &mut self,
    run: &PositionedInlineRun,
    built: &'r BuiltInlineLayout<'_>,
    layout: Layout,
    x: f32,
    y: f32,
    line_y: f32,
  ) -> Option<GlyphRun<'r>> {
    let shaped = &run.glyph_run;

    if shaped.glyphs.is_empty() {
      return None;
    }
    let font = self.cached_font(shaped)?;
    let offset = run.glyph_offset(layout);

    if shaped
      .glyphs
      .first()
      .is_some_and(|glyph| self.window.disowns_line(line_y + offset.y + glyph.y))
    {
      return None;
    }
    let text = built
      .text
      .get(shaped.text_range.clone())
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

/// The positioning area `background-origin` selects, never negative.
fn background_origin_area(origin: BackgroundOrigin, layout: Layout) -> (CorePoint<f32>, Size<f32>) {
  let area = background_origin_box(origin, layout);

  (
    area.offset,
    Size {
      width: area.size.width.max(0.0),
      height: area.size.height.max(0.0),
    },
  )
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
}

impl SurfaceDevice<'_, '_> {
  /// Draws the path `build` makes at `transform`'s translation, with the rest
  /// of `transform` pushed around it. A pure translation folds into the path,
  /// which keeps the content stream free of a `cm` pair for every fill.
  fn draw(
    &mut self,
    transform: Affine,
    build: impl FnOnce(f32, f32) -> Option<KrillaPath>,
    paint: impl FnOnce(&mut Surface, &KrillaPath),
  ) {
    let flat = transform.only_translation();
    let (x, y) = if flat {
      (transform.x, transform.y)
    } else {
      (0.0, 0.0)
    };
    let Some(path) = build(x, y) else {
      return;
    };

    if !flat {
      self
        .surface
        .push_transform(&krilla_transform(transform.to_cols_array()));
    }
    if self.artifact {
      self.surface.start_tagged(ARTIFACT);
    }
    paint(self.surface, &path);
    if !flat {
      self.surface.pop();
    }
    if self.artifact {
      self.surface.end_tagged();
    }
  }
}

impl PaintDevice for SurfaceDevice<'_, '_> {
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    let fill = Fill {
      rule: krilla_fill_rule(shape.rule()),
      ..fill_from_rgba(filtered(self.filter, color), 1.0)
    };

    self.draw(
      transform,
      |x, y| shape_path(shape, x, y),
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
    let stroke = Stroke {
      paint: fill_from_rgba(filtered(self.filter, stroke.color), 1.0).paint,
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
      |x, y| krilla_path(&shape.to_commands(), x, y),
      |surface, path| {
        surface.set_fill(None);
        surface.set_stroke(Some(stroke));
        surface.draw_path(path);
        surface.set_stroke(None);
      },
    );
  }
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

/// Fills `path` with the child indices leading from `root` to `target`, matched by identity.
fn node_path(root: &RenderNode, target: &RenderNode, path: &mut Vec<usize>) -> bool {
  if ptr::eq(root, target) {
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
