//! The scene walker that emits boxes, text and images onto a krilla surface.

use std::{cell::RefCell, collections::HashMap};

#[cfg(feature = "images")]
use crate::krilla::geom::Size as KrillaSize;
use crate::krilla::{
  Data,
  geom::{Point, Rect as KrillaRect, Transform},
  num::NormalizedF32,
  paint::{
    Fill, FillRule, LinearGradient as KrillaLinearGradient, Paint, Pattern,
    RadialGradient as KrillaRadialGradient, SpreadMethod, SweepGradient,
  },
  surface::Surface,
  tagging::{Artifact, ArtifactType, ContentTag},
  text::{Font, GlyphId},
};
#[cfg(feature = "images")]
use takumi_core::{
  context::RenderContext, layout::node::ImageData, resources::image::ImageSource, style::ObjectFit,
};
use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, Point as CorePoint, Size},
  layout::{
    border::{BorderProperties, BorderSide},
    clip::clip_shape_commands,
    decoration::ClipBox,
    inline::{BuiltInlineLayout, InlineRunLayout, ShapedRun, run_decorations},
    node::NodeKind,
    tree::{LayoutResults, RenderNode},
  },
  paint::{ConicGradientTile, LinearGradientTile, RadialGradientTile, resolve_stops_along_axis},
  scene::{NodePaint, PaintItemKind, StackingContextNode},
  style::{
    Affine, BackgroundImage, BlendMode, BoxDecorationBreak, BreakBetween, BreakInside,
    FillRule as CoreFillRule, Isolation,
  },
};

use crate::background::{Placement, cycled, place};
use crate::glyph::{PdfGlyph, glyph_text_spans};
use crate::inline::{InlineMap, build_inline_runs, inline_key, node_inline_items, text_line_atoms};
use crate::options::PdfError;
use crate::pagination::Atom;
use crate::paint::{
  draw_decoration, empty_path, expanded_radial_stops, fill_from_rgba, krilla_blend, krilla_path,
  krilla_stop, krilla_stops, overflow_clip_rect, pop_transforms, rect_path, spread,
};
#[cfg(feature = "images")]
use crate::paint::{position_axis, rasterized_image};
#[cfg(all(feature = "svg", feature = "images"))]
use crate::svg;
use crate::tags::TagCollector;

/// Scene walker state: the render tree, the stacking-context scene, and a cache
/// of krilla fonts keyed by the backing blob identity.
pub(crate) type FontMap = HashMap<(u64, u32), Font>;

pub(crate) struct Emitter<'a> {
  pub(crate) root: &'a RenderNode,
  pub(crate) contexts: &'a [StackingContextNode],
  pub(crate) results: &'a LayoutResults,
  pub(crate) fonts: &'a mut FontMap,
  /// Pre-built inline layouts for the content tree; band trees build on the
  /// fly.
  pub(crate) inline: Option<&'a InlineMap<'a>>,
  /// Vertical content window `[top, bottom)` of the page being emitted;
  /// paint wholly outside it is skipped so clipped-away content never reaches
  /// the content stream (or text extraction).
  pub(crate) window: Option<(f32, f32)>,
  /// Text-line ownership window: `[this page's cut, next page's cut)`. Wider
  /// than `window` at the edges (first page reaches up to −∞, last to +∞) and
  /// narrower at the bottom when a cut lands above the page's full height, so
  /// every line is emitted on exactly one page.
  pub(crate) line_window: Option<(f32, f32)>,
  /// Records a marked-content identifier per source node while drawing, for
  /// the structure tree built after emission.
  pub(crate) tags: Option<&'a RefCell<TagCollector>>,
}

impl Emitter<'_> {
  fn window_excludes(&self, top: f32, bottom: f32) -> bool {
    self
      .window
      .is_some_and(|(y0, y1)| bottom <= y0 || top >= y1)
  }

  /// Whether a text line at `baseline` belongs to another page. Ownership is
  /// keyed on the baseline (always inside the line's own box, unlike the font
  /// ascent band, which can poke above the container a forced break cut at) and
  /// half-open, so each line is emitted exactly once.
  fn window_disowns_line(&self, baseline: f32) -> bool {
    self
      .line_window
      .is_some_and(|(y0, y1)| baseline < y0 || baseline >= y1)
  }

  fn window_excludes_bounds(&self, bounds: Option<takumi_core::scene::SceneBounds>) -> bool {
    bounds.is_some_and(|b| self.window_excludes(b.top as f32, b.bottom as f32))
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

    let (child_frame, root_pushed) = match context.root() {
      Some(paint) => self.emit_box(paint, parent, surface)?,
      None => (parent, 0),
    };

    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(paint) => {
            let (_, pushed) = self.emit_box(paint, child_frame, surface)?;
            pop_transforms(surface, pushed);
          }
          PaintItemKind::Context(child) => {
            let excluded = self
              .contexts
              .get(*child)
              .is_some_and(|ctx| self.window_excludes_bounds(ctx.paint_bounds()));
            if !excluded {
              self.emit_context(*child, child_frame, surface)?;
            }
          }
        }
      }
    }
    pop_transforms(surface, root_pushed);
    Ok(())
  }

  /// Emits one node's background and own content. Returns the frame the node's
  /// children sit in and how many transforms were pushed onto the surface.
  fn emit_box(
    &mut self,
    paint: &NodePaint,
    parent: Affine,
    surface: &mut Surface,
  ) -> Result<(Affine, usize), PdfError> {
    let Some(node) = self.root.node_at_path(&paint.path) else {
      return Ok((parent, 0));
    };
    let Ok(layout) = self.results.layout(paint.node_id) else {
      return Ok((parent, 0));
    };
    if self.window_excludes_bounds(paint.paint_bounds) {
      return Ok((parent, 0));
    }

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
      && let Some((window_top, window_bottom)) = self.window
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

    // `clip-path` clips the element itself, decorations included, so it goes on
    // before anything is painted.
    if let Some(shape) = &style.clip_path {
      let commands = clip_shape_commands(shape, &node.context, layout.size);
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
    self.emit_background(node, &border, deco_size, x, deco_y, surface);
    self.emit_background_layers(node, &border, deco_size, x, deco_y, surface);
    self.emit_borders(&border, x, deco_y, deco_size, surface);

    // Children and own content clip to the (rounded) padding box when overflow
    // is hidden; without radius a per-axis overflow leaves the visible axis
    // unbounded.
    if style.clips_overflow() {
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
        pushed += 1;
      }
    }

    let tagged = self.tags.is_some() && has_own_content(node);

    if tagged {
      if decorative_image(node) {
        surface.start_tagged(ContentTag::Artifact(Artifact::new(
          ArtifactType::Other,
          None,
        )));
      } else {
        let identifier = surface.start_tagged(ContentTag::Other);

        if let Some(tags) = self.tags {
          tags.borrow_mut().record(&paint.path, identifier);
        }
      }
    }
    self.emit_own_content(node, layout, x, y, surface)?;
    if tagged {
      surface.end_tagged();
    }
    Ok((frame, pushed))
  }

  fn emit_background(
    &self,
    node: &RenderNode,
    border: &BorderProperties,
    size: Size<f32>,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let color = node
      .context
      .style
      .background_color
      .resolve(node.context.current_color);
    if color.0[3] == 0 {
      return;
    }
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

    border.append_mask_commands(&mut commands, size, CorePoint::ZERO);
    let Some(path) = krilla_path(&commands, x, y) else {
      return;
    };

    let artifact = self.start_artifact(surface);

    surface.set_fill(Some(fill_from_rgba(color.0, 1.0)));
    surface.draw_path(&path);
    if artifact {
      surface.end_tagged();
    }
  }

  /// Paints `background-image` gradient layers, bottom layer first, clipped to
  /// the rounded border box.
  // ponytail: url() layers are not resolved yet, and the positioning area is
  // the border box regardless of `background-origin`.
  fn emit_background_layers(
    &self,
    node: &RenderNode,
    border: &BorderProperties,
    size: Size<f32>,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let style = &node.context.style;
    let Some(images) = style.background_image.as_deref() else {
      return;
    };
    if !images.iter().any(|image| {
      matches!(
        image,
        BackgroundImage::Linear(_) | BackgroundImage::Radial(_) | BackgroundImage::Conic(_)
      )
    }) {
      return;
    }
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

    border.append_mask_commands(&mut commands, size, CorePoint::ZERO);
    let Some(clip) = krilla_path(&commands, x, y) else {
      return;
    };
    let artifact = self.start_artifact(surface);

    surface.push_clip_path(&clip, &FillRule::NonZero);
    for (index, image) in images.iter().enumerate().rev() {
      let placement = place(
        size,
        cycled(&style.background_size, index),
        cycled(&style.background_position, index),
        cycled(&style.background_repeat, index),
        &node.context,
      );

      if placement.tiles {
        self.tiled_layer(image, node, &placement, size, (x, y), surface);
      } else {
        self.background_layer(
          image,
          node,
          placement.tile,
          x + placement.origin.0,
          y + placement.origin.1,
          surface,
        );
      }
    }
    surface.pop();
    if artifact {
      surface.end_tagged();
    }
  }

  /// Draws one tile into a pattern and fills the layer's area with it, so a
  /// repeated layer costs one shading instead of one per tile.
  fn tiled_layer(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    placement: &Placement,
    size: Size<f32>,
    at: (f32, f32),
    surface: &mut Surface,
  ) {
    let (x, y) = at;
    let stream = {
      let mut builder = surface.stream_builder();
      let mut tile = builder.surface();

      self.background_layer(image, node, placement.tile, 0.0, 0.0, &mut tile);
      tile.finish();
      builder.finish()
    };
    let Some(path) = KrillaRect::from_xywh(x, y, size.width, size.height).and_then(rect_path)
    else {
      return;
    };

    surface.set_fill(Some(Fill {
      paint: Pattern {
        stream,
        transform: Transform::from_translate(x + placement.origin.0, y + placement.origin.1),
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
    let sizing = &node.context.sizing;
    let current_color = node.context.current_color;

    let paint: Paint = match image {
      BackgroundImage::Linear(gradient) => {
        let tile = LinearGradientTile::new(gradient, w as u32, h as u32, sizing, current_color);
        let resolved = resolve_stops_along_axis(
          &gradient.stops,
          tile.axis_length.max(1e-6),
          sizing,
          current_color,
        );
        if resolved.is_empty() {
          return;
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
        let tile = RadialGradientTile::new(gradient, w as u32, h as u32, sizing, current_color);
        let resolved = resolve_stops_along_axis(
          &gradient.stops,
          tile.radius_scale.max(1e-6),
          sizing,
          current_color,
        );
        if resolved.is_empty() {
          return;
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
        let tile = ConicGradientTile::new(gradient, w as u32, h as u32, sizing, current_color);
        let lut_len = tile.color_lut.len();
        if lut_len == 0 {
          return;
        }
        const SWEEP_STOPS: usize = 64;
        let stops = (0..=SWEEP_STOPS)
          .map(|i| {
            let t = i as f32 / SWEEP_STOPS as f32;
            let index =
              tile.lut_index_for_adjusted_angle_with_len(t * core::f32::consts::TAU, lut_len);
            let color = tile.color_lut[index].demultiply();

            krilla_stop(t, [color.red(), color.green(), color.blue(), color.alpha()])
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
      BackgroundImage::Url(_) | BackgroundImage::None => return,
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

  /// Opens an artifact sequence around a decoration when tagging is on, so it
  /// stays out of the structure tree. Returns whether one was opened.
  fn start_artifact(&self, surface: &mut Surface) -> bool {
    if self.tags.is_none() {
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

    if let Some(color) = border.has_uniform_visible_color() {
      if color.0[3] != 0 {
        let artifact = self.start_artifact(surface);

        surface.set_fill(Some(Fill {
          rule: FillRule::EvenOdd,
          ..fill_from_rgba(color.0, 1.0)
        }));
        surface.draw_path(&ring_path);
        if artifact {
          surface.end_tagged();
        }
      }
      return;
    }
    let artifact = self.start_artifact(surface);

    surface.push_clip_path(&ring_path, &FillRule::EvenOdd);
    for (side, width, color, style) in [
      (
        BorderSide::Top,
        border.width.top,
        border.color.top,
        border.style.top,
      ),
      (
        BorderSide::Right,
        border.width.right,
        border.color.right,
        border.style.right,
      ),
      (
        BorderSide::Bottom,
        border.width.bottom,
        border.color.bottom,
        border.style.bottom,
      ),
      (
        BorderSide::Left,
        border.width.left,
        border.color.left,
        border.style.left,
      ),
    ] {
      if width <= 0.0 || color.0[3] == 0 || !style.is_rendered() {
        continue;
      }
      let mut polygon = Vec::new();

      border.append_side_clip_polygon_commands_at(side, &mut polygon, size, CorePoint::ZERO);
      if let Some(path) = krilla_path(&polygon, x, y) {
        surface.set_fill(Some(fill_from_rgba(color.0, 1.0)));
        surface.draw_path(&path);
      }
    }
    surface.pop();
    if artifact {
      surface.end_tagged();
    }
  }

  fn emit_own_content(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    if node.should_create_inline_layout() {
      return self.emit_node_text(node, layout, x, y, surface);
    }
    if node.has_anonymous_text_item_child() {
      return Ok(());
    }
    match node.node.as_ref().map(|n| &n.kind) {
      Some(NodeKind::Text(_)) => self.emit_node_text(node, layout, x, y, surface),
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
  // ponytail: pixels upload as un-premultiplied RGBA8, so JPEG bytes re-encode
  // as flate; add DCT passthrough when PDF size from photos matters.
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
    let scale = match context.style.object_fit {
      ObjectFit::Contain => (w / iw).min(h / ih),
      ObjectFit::Cover => (w / iw).max(h / ih),
      ObjectFit::ScaleDown => (w / iw).min(h / ih).min(1.0),
      ObjectFit::None => 1.0,
      _ => 0.0,
    };
    let (dw, dh) = if scale == 0.0 {
      (w, h)
    } else {
      (iw * scale, ih * scale)
    };
    // SVG sources embed as vector ops; everything else rasterizes.
    #[cfg(feature = "svg")]
    let vector = if let ImageSource::Svg(svg) = &source {
      let (svg_width, svg_height) = svg.dimensions();
      if svg_width <= 0.0 || svg_height <= 0.0 {
        return;
      }
      // Fallback rasters (filters, embedded bitmaps) keep the old 2x density.
      let raster_scale = 2.0 * (dw / svg_width).max(dh / svg_height);

      Some((svg.vector_ops(raster_scale), svg_width, svg_height))
    } else {
      None
    };
    #[cfg(not(feature = "svg"))]
    let vector: Option<((), f32, f32)> = None;

    let krilla_image = if vector.is_none() {
      match rasterized_image(&source, context, (dw, dh)) {
        Some(image) => Some(image),
        None => return,
      }
    } else {
      None
    };
    let position = context.style.object_position.0;
    let ix = bx + position_axis(position.x, context, w - dw);
    let iy = by + position_axis(position.y, context, h - dh);

    let Some(size) = KrillaSize::from_wh(dw, dh) else {
      return;
    };
    let overflows = dw > w + 0.5 || dh > h + 0.5;

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
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    if let Some(prepared) = self.inline.and_then(|map| map.get(&inline_key(node))) {
      return self.draw_runs(&prepared.runs, &prepared.built, layout, x, y, surface);
    }
    let context = &node.context;
    let Some(items) = node_inline_items(node) else {
      return Ok(());
    };
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((built, runs)) = build_inline_runs(items, &font_style, context, layout)? else {
      return Ok(());
    };

    self.draw_runs(&runs, &built, layout, x, y, surface)
  }

  fn draw_runs(
    &mut self,
    runs: &InlineRunLayout,
    built: &BuiltInlineLayout<'_>,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    for run in &runs.runs {
      let shaped = &run.glyph_run;
      if shaped.glyphs.is_empty() {
        continue;
      }
      let Some(font) = self.cached_font(shaped) else {
        continue;
      };
      let offset = run.glyph_offset(layout);
      if let Some(glyph) = shaped.glyphs.first() {
        let baseline = y + offset.y + glyph.y;
        if self.window_disowns_line(baseline) {
          continue;
        }
      }
      let decorations = run_decorations(
        shaped,
        layout,
        run.baseline_shift,
        run.transform(Affine::IDENTITY),
      );

      for decoration in decorations.iter().filter(|d| !d.over) {
        draw_decoration(surface, decoration, x, y);
      }
      let run_text = built
        .text
        .get(shaped.text_range.clone())
        .unwrap_or_default();
      let spans = glyph_text_spans(shaped, run_text);

      let glyphs: Vec<PdfGlyph> = shaped
        .glyphs
        .iter()
        .zip(spans)
        .map(|(glyph, range)| PdfGlyph {
          id: GlyphId::new(glyph.id),
          x_offset: glyph.x / shaped.font_size,
          y_offset: -glyph.y / shaped.font_size,
          range,
        })
        .collect();

      let color = shaped.brush.color;

      surface.set_fill(Some(fill_from_rgba(color.0, shaped.brush.opacity)));
      surface.draw_glyphs(
        Point::from_xy(x + offset.x, y + offset.y),
        &glyphs,
        font,
        run_text,
        shaped.font_size,
        false,
      );
      for decoration in decorations.iter().filter(|d| d.over) {
        draw_decoration(surface, decoration, x, y);
      }
    }
    Ok(())
  }

  /// A krilla font for a run's backing blob, cached by the blob's stable id.
  /// Copies the blob into the cache once per distinct font.
  fn cached_font(&mut self, shaped: &ShapedRun) -> Option<Font> {
    let key = (shaped.font_id(), shaped.font_index);

    if let Some(font) = self.fonts.get(&key) {
      return Some(font.clone());
    }
    let font = Font::new(Data::from(shaped.font_data().to_vec()), shaped.font_index)?;

    self.fonts.insert(key, font.clone());
    Some(font)
  }
}

impl Emitter<'_> {
  /// Mirrors [`Self::emit_context`] but records unsplittable vertical extents
  /// instead of painting.
  pub(crate) fn collect_atoms(
    &mut self,
    id: usize,
    parent: Affine,
    atoms: &mut Vec<Atom>,
    forced: &mut Vec<f32>,
  ) -> Result<(), PdfError> {
    let Some(context) = self.contexts.get(id) else {
      return Ok(());
    };

    let child_frame = match context.root() {
      Some(paint) => self.collect_box_atoms(paint, parent, atoms, forced)?,
      None => parent,
    };

    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(paint) => {
            self.collect_box_atoms(paint, child_frame, atoms, forced)?;
          }
          PaintItemKind::Context(child) => {
            self.collect_atoms(*child, child_frame, atoms, forced)?;
          }
        }
      }
    }
    Ok(())
  }

  /// Records one node's atoms and returns the frame its children sit in. A
  /// node painted under a non-translation transform becomes a single atom
  /// spanning its device bounds — windowing through a rotation would distort.
  fn collect_box_atoms(
    &mut self,
    paint: &NodePaint,
    parent: Affine,
    atoms: &mut Vec<Atom>,
    forced: &mut Vec<f32>,
  ) -> Result<Affine, PdfError> {
    let Some(node) = self.root.node_at_path(&paint.path) else {
      return Ok(parent);
    };
    let Ok(layout) = self.results.layout(paint.node_id) else {
      return Ok(parent);
    };

    let relative = parent.invert().unwrap_or(Affine::IDENTITY) * paint.transform;
    if !relative.only_translation() {
      if let Some(bounds) = paint.paint_bounds {
        atoms.push((bounds.top as f32, bounds.bottom as f32));
      }
      return Ok(parent * relative);
    }
    let y = relative.y;
    let style = &node.context.style;

    if style.break_before == BreakBetween::Page {
      forced.push(y);
    }
    if style.break_after == BreakBetween::Page {
      forced.push(y + layout.size.height);
    }
    if style.break_inside == BreakInside::Avoid {
      atoms.push((y, y + layout.size.height));
    }

    if node.should_create_inline_layout() {
      self.collect_text_atoms(node, layout, y, atoms)?;
    } else if !node.has_anonymous_text_item_child() {
      match node.node.as_ref().map(|n| &n.kind) {
        Some(NodeKind::Text(_)) => {
          self.collect_text_atoms(node, layout, y, atoms)?;
        }
        Some(NodeKind::Image(_)) => {
          atoms.push((y, y + layout.size.height));
        }
        _ => {}
      }
    }
    Ok(parent)
  }

  /// One atom per text line: the union of each run's ascent-to-descent band.
  fn collect_text_atoms(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    y: f32,
    atoms: &mut Vec<Atom>,
  ) -> Result<(), PdfError> {
    if let Some(prepared) = self.inline.and_then(|map| map.get(&inline_key(node))) {
      text_line_atoms(&prepared.runs, layout, y, atoms);
      return Ok(());
    }
    let context = &node.context;
    let Some(items) = node_inline_items(node) else {
      return Ok(());
    };
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((_, runs)) = build_inline_runs(items, &font_style, context, layout)? else {
      return Ok(());
    };

    text_line_atoms(&runs, layout, y, atoms);
    Ok(())
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

/// Whether the node is an image explicitly marked decorative (`alt=""`), so
/// its content is emitted as an artifact instead of a `Figure` element.
fn decorative_image(node: &RenderNode) -> bool {
  node.node.as_ref().is_some_and(|source| {
    source.tag_name().is_some_and(|name| name == "img") && source.alt() == Some("")
  })
}
