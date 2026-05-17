use parley::{InlineBoxKind, PositionedLayoutItem};
use taffy::{AvailableSpace, Layout, NodeId, Point, TaffyError, geometry::Size};
use tiny_skia::Pixmap;
use tiny_skia::PixmapMut;

use crate::layout::inline::{
  InlineLayoutMode, InlineLayoutRequest, get_parent_font_metrics, resolve_inline_line_metrics,
  resolve_inline_line_states, resolve_visual_inline_box, text_fit_line_alignment_correction,
};
use crate::{
  Error, Result,
  layout::{
    inline::{collect_inline_items, create_inline_layout, resolve_inline_max_height},
    style::{
      Affine, BackgroundImage, BlendMode, Color, ComputedStyle, Display, Filter, SpacePair,
      apply_backdrop_filter, apply_filters_to_pixmap,
    },
    tree::{LayoutResults, RenderNode},
  },
  rendering::{
    BlurType, BorderProperties, Canvas, CanvasSubcanvas, CanvasViewport, NodeMaskAction, Placement,
    Sizing, blend_pixel, draw_debug_border, get_node_mut_by_path, prepare_node_mask,
    scale_text_fit_x, transformed_rect_extents,
  },
};

pub(crate) fn blend_pixmap_software(
  dst: &mut Pixmap,
  src: &Pixmap,
  mode: BlendMode,
  offset: Point<i32>,
  opacity: f32,
) {
  if opacity <= 0.0 {
    return;
  }

  let Some(OverlapRegion {
    dst_left,
    dst_top,
    src_left,
    src_top,
    width,
    height,
  }) = overlapping_region(dst, src, offset)
  else {
    return;
  };

  let dst_width = dst.width() as usize;
  let src_width = src.width() as usize;
  let dst_pixels = dst.pixels_mut();
  let src_pixels = src.pixels();
  for row in 0..height {
    let dst_row = (dst_top + row) * dst_width;
    let src_row = (src_top + row) * src_width;
    for col in 0..width {
      let dst_pixel = &mut dst_pixels[dst_row + dst_left + col];
      let src_pixel = src_pixels[src_row + src_left + col];
      let s = src_pixel.demultiply();
      let d = dst_pixel.demultiply();
      let mut out = image::Rgba([d.red(), d.green(), d.blue(), d.alpha()]);
      let top = image::Rgba([
        s.red(),
        s.green(),
        s.blue(),
        ((s.alpha() as f32) * opacity).clamp(0.0, 255.0) as u8,
      ]);
      blend_pixel(&mut out, top, mode);
      *dst_pixel = Color(out.0).into();
    }
  }
}

fn overlapping_region(dst: &Pixmap, src: &Pixmap, offset: Point<i32>) -> Option<OverlapRegion> {
  let dst_left = offset.x.max(0) as usize;
  let dst_top = offset.y.max(0) as usize;
  let src_left = (-offset.x).max(0) as usize;
  let src_top = (-offset.y).max(0) as usize;
  let width = (dst.width() as i32 - offset.x.max(0))
    .min(src.width() as i32 - src_left as i32)
    .max(0) as usize;
  let height = (dst.height() as i32 - offset.y.max(0))
    .min(src.height() as i32 - src_top as i32)
    .max(0) as usize;

  if width == 0 || height == 0 {
    return None;
  }

  Some(OverlapRegion {
    dst_left,
    dst_top,
    src_left,
    src_top,
    width,
    height,
  })
}

struct OverlapRegion {
  dst_left: usize,
  dst_top: usize,
  src_left: usize,
  src_top: usize,
  width: usize,
  height: usize,
}

pub(crate) fn apply_transform(
  transform: &mut Affine,
  style: &ComputedStyle,
  border_box: Size<f32>,
  sizing: &Sizing,
) {
  let origin = style.transform_origin.to_point(sizing, border_box);

  // CSS Transforms Level 2 order: T(origin) * translate * rotate * scale * transform * T(-origin)
  // Ref: https://www.w3.org/TR/css-transforms-2/#ctm

  let mut local = Affine::translation(origin.x, origin.y);

  if style.translate != SpacePair::default() {
    local *= Affine::translation(
      style.translate.x.to_px(sizing, border_box.width),
      style.translate.y.to_px(sizing, border_box.height),
    );
  }

  if let Some(rotate) = style.rotate {
    local *= Affine::rotation(rotate);
  }

  if style.scale != SpacePair::default() {
    local *= Affine::scale(style.scale.x.0, style.scale.y.0);
  }

  if let Some(node_transform) = &style.transform {
    local *= Affine::from_transforms(node_transform.iter(), sizing, border_box);
  }

  local *= Affine::translation(-origin.x, -origin.y);

  *transform *= local;
}

fn get_node_by_path<'a, 'g>(
  root: &'a RenderNode<'g>,
  path: &[usize],
) -> Option<&'a RenderNode<'g>> {
  let mut current = root;
  for &index in path {
    let children = current.children.as_deref()?;
    current = children.get(index)?;
  }
  Some(current)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OrderedChild {
  pub(crate) render_index: usize,
  pub(crate) node_id: NodeId,
}

pub(crate) fn collect_layout_children(
  layout_results: &LayoutResults,
  node_id: NodeId,
  render_children: &[RenderNode<'_>],
) -> Result<Vec<OrderedChild>> {
  let layout_children = layout_results.children(node_id)?;
  let child_count = render_children.len().min(layout_children.len());
  let mut ordered_children = Vec::with_capacity(child_count);
  for (render_index, node_id) in layout_children
    .iter()
    .copied()
    .take(child_count)
    .enumerate()
  {
    ordered_children.push(OrderedChild {
      render_index,
      node_id,
    });
  }

  Ok(ordered_children)
}

#[derive(Clone)]
struct NodePaint {
  path: Vec<usize>,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
  paint_bounds: Option<SceneBounds>,
}

#[derive(Clone, Copy)]
struct SceneBounds {
  left: usize,
  top: usize,
  right: usize,
  bottom: usize,
}

impl SceneBounds {
  fn intersects_viewport(self, viewport: CanvasViewport) -> bool {
    let viewport_left = viewport.origin.x as i32;
    let viewport_top = viewport.origin.y as i32;
    let viewport_right = viewport.right();
    let viewport_bottom = viewport.bottom();

    let left = self.left as i32;
    let top = self.top as i32;
    let right = self.right as i32;
    let bottom = self.bottom as i32;

    right > viewport_left && bottom > viewport_top && left < viewport_right && top < viewport_bottom
  }
}

enum DeferredNodeRender {
  Deferred {
    path: Vec<usize>,
    has_constraint: bool,
    isolated_canvas: Option<CanvasSubcanvas>,
    filter_bounds: Option<SceneBounds>,
  },
  SkipRendering,
}

#[derive(Clone)]
enum PaintItemKind {
  Node(NodePaint),
  Context(usize),
}

#[derive(Clone)]
struct PaintItem {
  kind: PaintItemKind,
  z_index: i32,
  source_order: usize,
}

#[derive(Clone, Copy)]
enum PaintBucket {
  Negative,
  InFlow,
  AutoZero,
  Positive,
}

#[derive(Default)]
struct StackingBuckets {
  negative: Vec<PaintItem>,
  inflow: Vec<PaintItem>,
  auto_zero: Vec<PaintItem>,
  positive: Vec<PaintItem>,
}

impl StackingBuckets {
  fn push(&mut self, bucket: PaintBucket, item: PaintItem) {
    match bucket {
      PaintBucket::Negative => self.negative.push(item),
      PaintBucket::InFlow => self.inflow.push(item),
      PaintBucket::AutoZero => self.auto_zero.push(item),
      PaintBucket::Positive => self.positive.push(item),
    }
  }

  fn sort(&mut self) {
    self.negative.sort_by(|left, right| {
      left
        .z_index
        .cmp(&right.z_index)
        .then_with(|| left.source_order.cmp(&right.source_order))
    });
    self.inflow.sort_by_key(|item| item.source_order);
    self.auto_zero.sort_by_key(|item| item.source_order);
    self.positive.sort_by(|left, right| {
      left
        .z_index
        .cmp(&right.z_index)
        .then_with(|| left.source_order.cmp(&right.source_order))
    });
  }

  fn in_paint_order(&self) -> [&[PaintItem]; 4] {
    [
      &self.negative,
      &self.inflow,
      &self.auto_zero,
      &self.positive,
    ]
  }
}

pub(crate) struct StackingContextNode {
  root: Option<NodePaint>,
  buckets: StackingBuckets,
  paint_bounds: Option<SceneBounds>,
}

impl StackingContextNode {
  fn with_root(root: Option<NodePaint>) -> Self {
    Self {
      root,
      buckets: StackingBuckets::default(),
      paint_bounds: None,
    }
  }

  fn push_item(
    &mut self,
    bucket: PaintBucket,
    kind: PaintItemKind,
    z_index: i32,
    source_order: usize,
  ) {
    self.buckets.push(
      bucket,
      PaintItem {
        kind,
        z_index,
        source_order,
      },
    );
  }
}

struct StackingContextBuildVisit {
  path: Vec<usize>,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
  context_id: usize,
  parent_display: Option<Display>,
  is_root: bool,
}

fn classify_bucket(style: &ComputedStyle, is_flex_or_grid_item: bool) -> (PaintBucket, i32) {
  let z_index = style.z_index.painting_order_value();
  let participates = style.participates_in_positioned_paint_bucket(is_flex_or_grid_item);

  if !participates {
    return (PaintBucket::InFlow, 0);
  }

  if z_index < 0 {
    return (PaintBucket::Negative, z_index);
  }
  if z_index > 0 {
    return (PaintBucket::Positive, z_index);
  }
  (PaintBucket::AutoZero, 0)
}

pub(crate) fn build_stacking_contexts<'g>(
  root: &RenderNode<'g>,
  layout_results: &LayoutResults,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
) -> Result<Vec<StackingContextNode>> {
  let mut contexts = vec![StackingContextNode::with_root(None)];
  let mut source_order = 0usize;
  let mut visits = vec![StackingContextBuildVisit {
    path: Vec::new(),
    node_id,
    transform,
    container_size,
    context_id: 0,
    parent_display: None,
    is_root: true,
  }];

  while let Some(visit) = visits.pop() {
    let Some(current) = get_node_by_path(root, &visit.path) else {
      return Err(Error::LayoutError(TaffyError::InvalidInputNode(
        visit.node_id,
      )));
    };
    let layout = *layout_results.layout(visit.node_id)?;
    if current.context.style.is_invisible() {
      continue;
    }

    let mut current_transform = visit.transform;
    current_transform *= Affine::translation(layout.location.x, layout.location.y);
    apply_transform(
      &mut current_transform,
      &current.context.style,
      layout.size,
      &current.context.sizing,
    );
    if !current_transform.is_invertible() {
      continue;
    }

    let node_paint = NodePaint {
      path: visit.path.clone(),
      node_id: visit.node_id,
      transform: current_transform,
      container_size: visit.container_size,
      paint_bounds: compute_node_paint_bounds(current, layout, current_transform),
    };

    let is_flex_or_grid_item = visit.parent_display.is_some_and(|display| {
      matches!(
        display,
        Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid
      )
    });

    let creates_context = if visit.is_root {
      true
    } else {
      current.context.style.creates_stacking_context(
        layout.size,
        &current.context.sizing,
        is_flex_or_grid_item,
      ) || current
        .context
        .style
        .resolve_overflows()
        .should_clip_content()
    };

    let mut active_context_id = visit.context_id;
    if visit.is_root {
      contexts[0].root = Some(node_paint);
    } else {
      let (bucket, z_index) = classify_bucket(&current.context.style, is_flex_or_grid_item);
      if creates_context {
        let context_id = contexts.len();
        contexts.push(StackingContextNode::with_root(Some(node_paint)));
        contexts[visit.context_id].push_item(
          bucket,
          PaintItemKind::Context(context_id),
          z_index,
          source_order,
        );
        active_context_id = context_id;
      } else {
        contexts[visit.context_id].push_item(
          bucket,
          PaintItemKind::Node(node_paint),
          z_index,
          source_order,
        );
      }
      source_order += 1;
    }

    let Some(children) = current.children.as_deref() else {
      continue;
    };

    if current.should_create_inline_layout() {
      continue;
    }

    let layout_children = collect_layout_children(layout_results, visit.node_id, children)?;
    let child_container_size = Size {
      width: Some(layout.content_box_width()),
      height: Some(layout.content_box_height()),
    };

    for child in layout_children.into_iter().rev() {
      let mut child_path = visit.path.clone();
      child_path.push(child.render_index);
      visits.push(StackingContextBuildVisit {
        path: child_path,
        node_id: child.node_id,
        transform: current_transform,
        container_size: child_container_size,
        context_id: active_context_id,
        parent_display: Some(current.context.style.display),
        is_root: false,
      });
    }
  }

  for context in &mut contexts {
    context.buckets.sort();
  }

  for context_id in (0..contexts.len()).rev() {
    let mut paint_bounds = contexts[context_id]
      .root
      .as_ref()
      .and_then(|node| node.paint_bounds);
    for bucket in contexts[context_id].buckets.in_paint_order() {
      for item in bucket {
        let item_bounds = match &item.kind {
          PaintItemKind::Node(node_paint) => node_paint.paint_bounds,
          PaintItemKind::Context(child_context_id) => contexts[*child_context_id].paint_bounds,
        };
        paint_bounds = merge_bounds(paint_bounds, item_bounds);
      }
    }
    contexts[context_id].paint_bounds = paint_bounds;
  }

  Ok(contexts)
}

fn compute_node_paint_bounds(
  node: &RenderNode<'_>,
  layout: Layout,
  transform: Affine,
) -> Option<SceneBounds> {
  let mut bounds = bounds_for_rect(layout.size, transform);
  if !has_inline_paint_content(node) {
    return bounds;
  }

  let font_style = node.context.style.to_sized_font_style(&node.context);
  if font_style.sizing.font_size == 0.0 {
    return bounds;
  }

  let available_space = Size {
    width: AvailableSpace::Definite(layout.content_box_width()),
    height: AvailableSpace::Definite(layout.content_box_height()),
  };
  let max_height = resolve_inline_max_height(&font_style, layout.content_box_height());

  let built = create_inline_layout(InlineLayoutRequest {
    items: collect_inline_items(node),
    available_space,
    max_width: layout.content_box_width(),
    max_height,
    style: &font_style,
    global: node.context.global,
    mode: InlineLayoutMode::Measure,
  });
  let inline_transform = Affine::translation(
    layout.border.left + layout.padding.left,
    layout.border.top + layout.padding.top,
  ) * transform;
  let parent_font_metrics = get_parent_font_metrics(&built.layout);

  let line_vertical_metrics = resolve_inline_line_metrics(
    &built.layout,
    &built.spans,
    parent_font_metrics,
    &built.line_scales,
  );
  let line_states = resolve_inline_line_states(
    &built.layout,
    &built.spans,
    parent_font_metrics,
    &built.line_scales,
  );
  for (line_index, line) in built.layout.lines().enumerate() {
    let baseline_shift = line_states[line_index].baseline_shift;
    let resolved_metrics = line_vertical_metrics[line_index];
    let line_scale = built.line_scales.get(line_index).copied().unwrap_or(1.0);
    let (line_scale_origin_x, line_alignment_correction) =
      text_fit_line_alignment_correction(&line, line_scale, layout.content_box_size().width);
    let line_scale_origin = Point {
      x: line_scale_origin_x,
      y: resolved_metrics.resolved_baseline,
    };
    let mut static_inline_prefix = 0.0_f32;
    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let metrics = glyph_run.run().metrics();
          let mut glyph_origin = Point {
            x: glyph_run.offset(),
            y: glyph_run.baseline() + baseline_shift - metrics.ascent,
          };
          let mut glyph_size = Size {
            width: glyph_run.advance(),
            height: metrics.ascent + metrics.descent,
          };
          if (line_scale - 1.0).abs() > f32::EPSILON {
            glyph_origin = Point {
              x: scale_text_fit_x(
                glyph_origin.x,
                line_scale_origin.x,
                line_scale,
                static_inline_prefix,
                line_alignment_correction,
              ),
              y: line_scale_origin.y + (glyph_origin.y - line_scale_origin.y) * line_scale,
            };
            glyph_size.width *= line_scale;
            glyph_size.height *= line_scale;
          }
          let glyph_transform =
            Affine::translation(glyph_origin.x, glyph_origin.y) * inline_transform;
          bounds = merge_bounds(bounds, bounds_for_rect(glyph_size, glyph_transform));
        }
        PositionedLayoutItem::InlineBox(inline_box) => {
          if inline_box.kind != InlineBoxKind::InFlow {
            continue;
          }
          let Some(inline_box) =
            resolve_visual_inline_box(inline_box, Some(line_states[line_index]), &built.spans)
          else {
            continue;
          };
          let inline_box_x = scale_text_fit_x(
            inline_box.x,
            line_scale_origin_x,
            line_scale,
            static_inline_prefix,
            line_alignment_correction,
          );
          static_inline_prefix += inline_box.width;

          bounds = merge_bounds(
            bounds,
            bounds_for_rect(
              Size {
                width: inline_box.width,
                height: inline_box.height,
              },
              Affine::translation(inline_box_x, inline_box.y) * inline_transform,
            ),
          );
        }
      }
    }
  }

  for inline_box in built.custom_inline_boxes {
    bounds = merge_bounds(
      bounds,
      bounds_for_rect(
        Size {
          width: inline_box.width,
          height: inline_box.height,
        },
        Affine::translation(inline_box.x, inline_box.y) * inline_transform,
      ),
    );
  }

  bounds
}

fn has_inline_paint_content(node: &RenderNode<'_>) -> bool {
  node.should_create_inline_layout()
    || node.anonymous_text_content.is_some()
    || node.children.as_ref().is_some_and(|children| {
      children
        .iter()
        .any(|child| child.anonymous_text_content.is_some())
    })
}

fn bounds_for_rect(size: Size<f32>, transform: Affine) -> Option<SceneBounds> {
  let (min_x, min_y, max_x, max_y) = transformed_rect_extents(Point::ZERO, size, transform)?;
  let left = (min_x.floor() as i32).max(0) as usize;
  let top = (min_y.floor() as i32).max(0) as usize;
  let right = (max_x.ceil() as i32).max(0) as usize;
  let bottom = (max_y.ceil() as i32).max(0) as usize;

  if left >= right || top >= bottom {
    return None;
  }

  Some(SceneBounds {
    left,
    top,
    right,
    bottom,
  })
}

fn merge_bounds(left: Option<SceneBounds>, right: Option<SceneBounds>) -> Option<SceneBounds> {
  match (left, right) {
    (Some(left), Some(right)) => Some(SceneBounds {
      left: left.left.min(right.left),
      top: left.top.min(right.top),
      right: left.right.max(right.right),
      bottom: left.bottom.max(right.bottom),
    }),
    (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
    (None, None) => None,
  }
}

fn finish_node_render<'g>(
  node: &mut RenderNode<'g>,
  canvas: &mut Canvas,
  has_constraint: bool,
  isolated_canvas: Option<CanvasSubcanvas>,
  filter_bounds: Option<SceneBounds>,
) -> Result<()> {
  if !node.context.style.filter.is_empty() {
    let viewport = canvas.viewport();
    let filter_padding = filter_padding(
      &node.context.style.filter,
      &node.context.sizing,
      node.context.transform,
    );
    let filter_region = filter_bounds.and_then(|bounds| {
      let left = (bounds.left as i32 - filter_padding).max(viewport.origin.x as i32);
      let top = (bounds.top as i32 - filter_padding).max(viewport.origin.y as i32);
      let right = (bounds.right as i32 + filter_padding).min(viewport.right());
      let bottom = (bounds.bottom as i32 + filter_padding).min(viewport.bottom());

      (left < right && top < bottom).then_some(Placement {
        left: left - viewport.origin.x as i32,
        top: top - viewport.origin.y as i32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
      })
    });

    if let Some(region) = filter_region {
      let row_bytes = region.width as usize * 4;
      let region_len = row_bytes * region.height as usize;
      let mut region_raw = canvas.buffer_pool.acquire(region_len);

      canvas.with_pixmap_ref_and_pool(|pixmap, _| {
        let canvas_width = pixmap.width() as usize;
        let canvas_raw: &[u8] = bytemuck::cast_slice(pixmap.pixels());
        for (y, dest_row) in region_raw.chunks_exact_mut(row_bytes).enumerate() {
          let src_y = region.top as usize + y;
          let src_start = (src_y * canvas_width + region.left as usize) * 4;
          dest_row.copy_from_slice(&canvas_raw[src_start..src_start + row_bytes]);
        }
      });

      let Some(mut region_pixmap) =
        PixmapMut::from_bytes(&mut region_raw, region.width, region.height)
      else {
        canvas.buffer_pool.release(region_raw);
        return Ok(());
      };

      apply_filters_to_pixmap(
        &mut region_pixmap,
        &node.context.sizing,
        node.context.current_color,
        &mut canvas.buffer_pool,
        node.context.style.filter.iter(),
      )?;

      canvas.with_pixmap_and_pool(|pixmap, _| {
        let canvas_width = pixmap.width() as usize;
        let canvas_raw: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
        for (y, src_row) in region_raw.chunks_exact(row_bytes).enumerate() {
          let dst_y = region.top as usize + y;
          let dst_start = (dst_y * canvas_width + region.left as usize) * 4;
          canvas_raw[dst_start..dst_start + row_bytes].copy_from_slice(src_row);
        }
      });

      canvas.buffer_pool.release(region_raw);
    } else {
      canvas.with_pixmap_and_pool(|pixmap, pool| {
        let mut pixmap_mut = pixmap.as_mut();
        apply_filters_to_pixmap(
          &mut pixmap_mut,
          &node.context.sizing,
          node.context.current_color,
          pool,
          node.context.style.filter.iter(),
        )
      })?;
    }
  }

  if let Some(isolated_canvas) = isolated_canvas {
    if has_constraint {
      canvas.pop_mask();
    }
    canvas.composite_subcanvas(
      isolated_canvas,
      node.context.style.mix_blend_mode,
      node.context.style.opacity.0,
    );
  } else if has_constraint {
    canvas.pop_mask();
  }

  Ok(())
}

fn filter_padding(filters: &[Filter], sizing: &Sizing, transform: Affine) -> i32 {
  let transform_scale = affine_max_scale(transform);
  filters
    .iter()
    .map(|filter| match filter {
      Filter::Blur(radius) => {
        (radius.to_px(sizing, 1.0) * BlurType::Filter.extent_multiplier() * transform_scale).ceil()
          as i32
      }
      Filter::DropShadow(shadow) => {
        let blur_spread = shadow.blur_radius.to_px(sizing, 1.0)
          * BlurType::Shadow.extent_multiplier()
          * transform_scale;
        let offset_x = shadow.offset_x.to_px(sizing, 1.0).abs() * transform_scale;
        let offset_y = shadow.offset_y.to_px(sizing, 1.0).abs() * transform_scale;
        (blur_spread + offset_x.max(offset_y)).ceil() as i32
      }
      _ => 0,
    })
    .sum()
}

fn affine_max_scale(transform: Affine) -> f32 {
  let s1 = transform.a * transform.a + transform.b * transform.b;
  let s2 = transform.c * transform.c + transform.d * transform.d;
  let off = transform.a * transform.c + transform.b * transform.d;
  let trace = s1 + s2;
  let half_trace = trace * 0.5;
  let det = s1 * s2 - off * off;
  let discriminant = (half_trace * half_trace - det).max(0.0);
  let sigma_max = (half_trace + discriminant.sqrt()).sqrt();
  if sigma_max.is_finite() {
    sigma_max.max(1.0)
  } else {
    1.0
  }
}

fn begin_node_render<'g>(
  root: &mut RenderNode<'g>,
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  node_paint: &NodePaint,
  defer_finish: bool,
  isolation_bounds_hint: Option<SceneBounds>,
) -> Result<Option<DeferredNodeRender>> {
  let Some(current) = get_node_mut_by_path(root, &node_paint.path) else {
    return Err(Error::LayoutError(TaffyError::InvalidInputNode(
      node_paint.node_id,
    )));
  };
  let layout = *layout_results.layout(node_paint.node_id)?;

  if current.context.style.is_invisible() || !node_paint.transform.is_invertible() {
    return Ok(None);
  }

  if let Some(bounds) = node_paint.paint_bounds
    && !bounds.intersects_viewport(canvas.viewport())
  {
    return Ok(Some(DeferredNodeRender::SkipRendering));
  }

  current.context.sizing.container_size = node_paint.container_size;
  current.context.transform = node_paint.transform;

  if !current.context.style.backdrop_filter.is_empty() {
    let border = BorderProperties::from_context(&current.context, layout.size, layout.border);
    apply_backdrop_filter(
      canvas,
      border,
      layout.size,
      node_paint.transform,
      &current.context,
    )?;
  }

  let should_isolate = current.context.style.needs_offscreen_compositing();
  let isolated_canvas = if should_isolate {
    Some(canvas.begin_subcanvas(compute_isolation_bounds(
      current,
      layout.size,
      node_paint.transform,
      canvas.viewport(),
      isolation_bounds_hint,
    ))?)
  } else {
    None
  };

  let mask_action = prepare_node_mask(
    &current.context,
    &current.context.style,
    layout,
    node_paint.transform,
    canvas.viewport(),
    &mut canvas.buffer_pool,
  )?;
  if matches!(mask_action, NodeMaskAction::SkipRendering) {
    if let Some(isolated_canvas) = isolated_canvas {
      canvas.composite_subcanvas(isolated_canvas, BlendMode::Normal, 0.0);
    }
    return Ok(Some(DeferredNodeRender::SkipRendering));
  }

  let has_constraint = mask_action.is_some();

  match mask_action {
    NodeMaskAction::None => {
      current.draw_shell(canvas, layout)?;
    }
    NodeMaskAction::Shell(mask) => {
      canvas.push_mask(mask);
      current.draw_shell(canvas, layout)?;
    }
    NodeMaskAction::Content(mask) => {
      current.draw_shell(canvas, layout)?;
      canvas.push_mask(mask);
    }
    NodeMaskAction::SkipRendering => return Ok(Some(DeferredNodeRender::SkipRendering)),
  }

  current.draw_content(canvas, layout)?;

  if current.context.draw_debug_border {
    draw_debug_border(canvas, layout, node_paint.transform);
  }

  if current.should_create_inline_layout() {
    current.draw_inline(canvas, layout)?;
    finish_node_render(
      current,
      canvas,
      has_constraint,
      isolated_canvas,
      node_paint.paint_bounds,
    )?;
  } else if !defer_finish {
    finish_node_render(
      current,
      canvas,
      has_constraint,
      isolated_canvas,
      node_paint.paint_bounds,
    )?;
  } else {
    return Ok(Some(DeferredNodeRender::Deferred {
      path: node_paint.path.clone(),
      has_constraint,
      isolated_canvas,
      filter_bounds: node_paint.paint_bounds,
    }));
  }

  Ok(None)
}

fn paint_single_node<'g>(
  root: &mut RenderNode<'g>,
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  node_paint: &NodePaint,
) -> Result<()> {
  match begin_node_render(root, layout_results, canvas, node_paint, false, None)? {
    Some(DeferredNodeRender::SkipRendering) | None => {}
    Some(DeferredNodeRender::Deferred {
      path,
      has_constraint,
      isolated_canvas,
      filter_bounds,
    }) => {
      let Some(current) = get_node_mut_by_path(root, &path) else {
        return Err(Error::LayoutError(TaffyError::InvalidInputNode(
          node_paint.node_id,
        )));
      };
      finish_node_render(
        current,
        canvas,
        has_constraint,
        isolated_canvas,
        filter_bounds,
      )?;
    }
  }
  Ok(())
}

fn paint_bucket<'g>(
  root: &mut RenderNode<'g>,
  contexts: &[StackingContextNode],
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  items: &[PaintItem],
) -> Result<()> {
  for item in items {
    match &item.kind {
      PaintItemKind::Node(node_paint) => {
        paint_single_node(root, layout_results, canvas, node_paint)?;
      }
      PaintItemKind::Context(context_id) => {
        paint_context(root, contexts, layout_results, canvas, *context_id)?;
      }
    }
  }
  Ok(())
}

pub(crate) fn paint_context<'g>(
  root: &mut RenderNode<'g>,
  contexts: &[StackingContextNode],
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  context_id: usize,
) -> Result<()> {
  let Some(context) = contexts.get(context_id) else {
    return Err(Error::LayoutError(TaffyError::InvalidInputNode(
      NodeId::new(context_id as u64),
    )));
  };

  if let Some(bounds) = context.paint_bounds
    && !bounds.intersects_viewport(canvas.viewport())
  {
    return Ok(());
  }

  let mut deferred_root = None;
  if let Some(root_paint) = &context.root {
    match begin_node_render(
      root,
      layout_results,
      canvas,
      root_paint,
      true,
      context.paint_bounds,
    )? {
      Some(DeferredNodeRender::SkipRendering) => return Ok(()),
      Some(deferred_root_render @ DeferredNodeRender::Deferred { .. }) => {
        deferred_root = Some(deferred_root_render);
      }
      None => {}
    }
  }

  for bucket in context.buckets.in_paint_order() {
    paint_bucket(root, contexts, layout_results, canvas, bucket)?;
  }

  if let Some(DeferredNodeRender::Deferred {
    path,
    has_constraint,
    isolated_canvas,
    filter_bounds,
  }) = deferred_root
  {
    let Some(current) = get_node_mut_by_path(root, &path) else {
      let node_id = context
        .root
        .as_ref()
        .map_or(layout_results.root_node_id(), |node| node.node_id);
      return Err(Error::LayoutError(TaffyError::InvalidInputNode(node_id)));
    };
    finish_node_render(
      current,
      canvas,
      has_constraint,
      isolated_canvas,
      context.paint_bounds.or(filter_bounds),
    )?;
  }

  Ok(())
}

fn supports_bounds_hint(node: &RenderNode<'_>, require_child_clipping: bool) -> bool {
  let style = &node.context.style;
  let has_children = node
    .children
    .as_ref()
    .is_some_and(|children| !children.is_empty());
  let clips_children = style.resolve_overflows().should_clip_content();
  let has_box_shadow = style
    .box_shadow
    .as_ref()
    .is_some_and(|shadows| !shadows.is_empty());
  let has_outline = style.outline_style.is_rendered();
  let has_text_shadow = style
    .text_shadow
    .as_ref()
    .is_some_and(|shadows| !shadows.is_empty());
  let has_text_stroke = style
    .webkit_text_stroke_width
    .is_some_and(|width| width != Default::default());
  let has_spread_background = style.background_image.as_ref().is_some_and(|images| {
    images.iter().any(|image| match image {
      BackgroundImage::Linear(gradient) => gradient.repeating,
      BackgroundImage::Radial(gradient) => gradient.repeating,
      BackgroundImage::Conic(gradient) => gradient.repeating,
      _ => false,
    })
  });

  style.filter.is_empty()
    && style.backdrop_filter.is_empty()
    && style.clip_path.is_none()
    && style.mask_image.as_ref().is_none_or(|images| {
      images
        .iter()
        .all(|image| matches!(image, BackgroundImage::None))
    })
    && !has_box_shadow
    && !has_outline
    && !has_text_shadow
    && !has_text_stroke
    && !has_spread_background
    && (!require_child_clipping || !has_children || clips_children)
}

fn placement_from_bounds(
  bounds: SceneBounds,
  viewport: CanvasViewport,
  padding: i32,
) -> Option<Placement> {
  let left = (bounds.left as i32 - padding).max(viewport.origin.x as i32);
  let top = (bounds.top as i32 - padding).max(viewport.origin.y as i32);
  let right = (bounds.right as i32 + padding).min(viewport.right());
  let bottom = (bounds.bottom as i32 + padding).min(viewport.bottom());

  Placement::from_bounds(left, top, right, bottom)
}

fn full_viewport_placement(viewport: CanvasViewport) -> Placement {
  Placement {
    left: viewport.origin.x as i32,
    top: viewport.origin.y as i32,
    width: viewport.size.width,
    height: viewport.size.height,
  }
}

fn compute_isolation_bounds(
  node: &RenderNode<'_>,
  size: Size<f32>,
  transform: Affine,
  viewport: CanvasViewport,
  paint_bounds_hint: Option<SceneBounds>,
) -> Placement {
  let placement = if supports_bounds_hint(node, false) {
    paint_bounds_hint.and_then(|bounds| placement_from_bounds(bounds, viewport, 2))
  } else if supports_bounds_hint(node, true) {
    transformed_rect_extents(Point::ZERO, size, transform).and_then(
      |(min_x, min_y, max_x, max_y)| {
        let left = min_x.floor().max(viewport.origin.x as f32) as i32;
        let top = min_y.floor().max(viewport.origin.y as f32) as i32;
        let right = max_x.ceil().min(viewport.right() as f32) as i32;
        let bottom = max_y.ceil().min(viewport.bottom() as f32) as i32;

        Placement::from_bounds(left, top, right, bottom)
      },
    )
  } else {
    None
  };

  placement.unwrap_or_else(|| full_viewport_placement(viewport))
}
