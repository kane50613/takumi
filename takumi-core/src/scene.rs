//! Backend-agnostic paint scene: the stacking-context tree that decides paint
//! order, grouping, and bounds. Raster and SVG backends consume this instead of
//! each walking the node tree independently.

use std::collections::HashMap;

use parley::{InlineBoxKind, PositionedLayoutItem};
use taffy::{AvailableSpace, Layout, NodeId, Point, TaffyError, geometry::Size};

use crate::{
  error::{Error, Result},
  font_style::SizedFontStyle,
  layout::{
    inline::{
      InlineLayoutMode, InlineLayoutRequest, collect_inline_items, create_inline_layout,
      get_parent_font_metrics, resolve_inline_line_metrics, resolve_inline_line_states,
      resolve_inline_max_height, resolve_visual_inline_box, scale_text_fit_x,
      text_fit_line_alignment_correction,
    },
    style::{Affine, ComputedStyle, Display, SizingContext},
    tree::{LayoutResults, OrderedChild, RenderNode},
  },
};

/// Transforms a rect's four corners and returns the axis-aligned `(min_x, min_y,
/// max_x, max_y)` extents, or `None` if any corner is non-finite.
pub fn transformed_rect_extents(
  origin: Point<f32>,
  size: Size<f32>,
  transform: Affine,
) -> Option<(f32, f32, f32, f32)> {
  let corners = [
    transform.transform_point(origin),
    transform.transform_point(Point {
      x: origin.x + size.width,
      y: origin.y,
    }),
    transform.transform_point(Point {
      x: origin.x,
      y: origin.y + size.height,
    }),
    transform.transform_point(Point {
      x: origin.x + size.width,
      y: origin.y + size.height,
    }),
  ];

  let mut min_x = f32::INFINITY;
  let mut min_y = f32::INFINITY;
  let mut max_x = f32::NEG_INFINITY;
  let mut max_y = f32::NEG_INFINITY;
  for point in corners {
    min_x = min_x.min(point.x);
    min_y = min_y.min(point.y);
    max_x = max_x.max(point.x);
    max_y = max_y.max(point.y);
  }

  if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
    return None;
  }

  Some((min_x, min_y, max_x, max_y))
}

pub fn apply_transform(
  transform: &mut Affine,
  style: &ComputedStyle,
  border_box: Size<f32>,
  sizing: &SizingContext,
) {
  *transform *= style.local_transform(border_box, sizing);
}

/// Resolves a node by its [`NodePaint::path`] (child indices from the root).
pub fn get_node_by_path<'a>(root: &'a RenderNode, path: &[usize]) -> Option<&'a RenderNode> {
  let mut current = root;
  for &index in path {
    let children = current.children.as_deref()?;
    current = children.get(index)?;
  }
  Some(current)
}

/// Mutable [`get_node_by_path`].
pub fn get_node_mut_by_path<'a>(
  root: &'a mut RenderNode,
  path: &[usize],
) -> Option<&'a mut RenderNode> {
  let mut current = root;
  for &index in path {
    let children = current.children.as_deref_mut()?;
    current = children.get_mut(index)?;
  }
  Some(current)
}

pub fn collect_layout_children(
  layout_results: &LayoutResults,
  node_id: NodeId,
) -> Result<Vec<OrderedChild>> {
  Ok(layout_results.box_children(node_id)?.to_vec())
}

/// A node's resolved paint inputs: where it sits in the tree, its accumulated
/// transform, the container it resolves against, and its device-space bounds.
#[derive(Clone)]
pub struct NodePaint {
  pub path: Vec<usize>,
  pub node_id: NodeId,
  pub transform: Affine,
  pub container_size: Size<Option<f32>>,
  pub paint_bounds: Option<SceneBounds>,
}

/// Device-space integer bounds of a node or stacking context's paint output.
#[derive(Clone, Copy)]
pub struct SceneBounds {
  pub left: usize,
  pub top: usize,
  pub right: usize,
  pub bottom: usize,
}

impl SceneBounds {
  pub fn is_empty(self) -> bool {
    self.left >= self.right || self.top >= self.bottom
  }
}

#[derive(Clone)]
pub enum PaintItemKind {
  Node(NodePaint),
  Context(usize),
}

#[derive(Clone)]
pub struct PaintItem {
  pub kind: PaintItemKind,
  z_index: i32,
  source_order: usize,
}

#[derive(Clone, Copy)]
enum PaintBucket {
  Negative,
  AutoZero,
  Positive,
}

#[derive(Default)]
pub struct StackingBuckets {
  negative: Vec<PaintItem>,
  auto_zero: Vec<PaintItem>,
  positive: Vec<PaintItem>,
}

impl StackingBuckets {
  fn push(&mut self, bucket: PaintBucket, item: PaintItem) {
    match bucket {
      PaintBucket::Negative => self.negative.push(item),
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
    self.auto_zero.sort_by_key(|item| item.source_order);
    self.positive.sort_by(|left, right| {
      left
        .z_index
        .cmp(&right.z_index)
        .then_with(|| left.source_order.cmp(&right.source_order))
    });
  }

  pub fn in_paint_order(&self) -> [&[PaintItem]; 3] {
    [&self.negative, &self.auto_zero, &self.positive]
  }
}

pub struct StackingContextNode {
  pub root: Option<NodePaint>,
  pub buckets: StackingBuckets,
  pub paint_bounds: Option<SceneBounds>,
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

/// The effective paint-order z for a child: its `z-index` when it participates in
/// the positioned paint bucket, otherwise 0 (non-positioned elements paint in tree
/// order alongside positioned `z-auto` elements). A stable sort by this key
/// reproduces CSS stacking order; both backends share it so they cannot drift.
pub fn paint_order_z(style: &ComputedStyle, is_flex_or_grid_item: bool) -> i32 {
  if style.participates_in_positioned_paint_bucket(is_flex_or_grid_item) {
    style.z_index.painting_order_value()
  } else {
    0
  }
}

fn classify_bucket(style: &ComputedStyle, is_flex_or_grid_item: bool) -> (PaintBucket, i32) {
  let z = paint_order_z(style, is_flex_or_grid_item);

  if z < 0 {
    (PaintBucket::Negative, z)
  } else if z > 0 {
    (PaintBucket::Positive, z)
  } else {
    (PaintBucket::AutoZero, 0)
  }
}

pub fn build_stacking_contexts(
  root: &RenderNode,
  layout_results: &LayoutResults,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
) -> Result<Vec<StackingContextNode>> {
  let mut contexts = vec![StackingContextNode::with_root(None)];
  let mut source_order = 0usize;
  // Hoisted out-of-flow nodes resolve geometry against their containing block,
  // not their box-tree parent. Memoize each node's transform and content box so
  // a hoisted child can use its CB's values as its base.
  let mut node_transforms: HashMap<NodeId, Affine> = HashMap::new();
  let mut node_content_box: HashMap<NodeId, Size<Option<f32>>> = HashMap::new();
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
    node_transforms.insert(visit.node_id, current_transform);

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

    if current.children.is_none() {
      continue;
    }

    if current.should_create_inline_layout() {
      continue;
    }

    let layout_children = collect_layout_children(layout_results, visit.node_id)?;
    let child_container_size = Size {
      width: Some(layout.content_box_width()),
      height: Some(layout.content_box_height()),
    };
    node_content_box.insert(visit.node_id, child_container_size);

    for child in layout_children.into_iter().rev() {
      let mut child_path = visit.path.clone();
      child_path.push(child.render_index);
      let (base_transform, base_container) = match child.hoisted_cb {
        Some(cb) => (
          *node_transforms.get(&cb).unwrap_or(&current_transform),
          *node_content_box.get(&cb).unwrap_or(&child_container_size),
        ),
        None => (current_transform, child_container_size),
      };
      visits.push(StackingContextBuildVisit {
        path: child_path,
        node_id: child.node_id,
        transform: base_transform,
        container_size: base_container,
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
  node: &RenderNode,
  layout: Layout,
  transform: Affine,
) -> Option<SceneBounds> {
  let mut bounds = bounds_for_rect(layout.size, transform);
  if !has_inline_paint_content(node) {
    return bounds;
  }

  let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);
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
    context: &node.context,
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

fn has_inline_paint_content(node: &RenderNode) -> bool {
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

  // Empty bounds mean "paints nothing"; None means "unknown" and forces full-viewport isolation.
  Some(SceneBounds {
    left,
    top,
    right,
    bottom,
  })
}

fn merge_bounds(left: Option<SceneBounds>, right: Option<SceneBounds>) -> Option<SceneBounds> {
  match (left, right) {
    // Empty bounds paint nothing and sit at clamped positions; don't let them expand the union.
    (Some(left), Some(right)) if left.is_empty() => Some(right),
    (Some(left), Some(right)) if right.is_empty() => Some(left),
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

#[cfg(test)]
mod tests {
  use taffy::geometry::Size;

  use super::{SceneBounds, bounds_for_rect, merge_bounds};
  use crate::layout::style::Affine;

  #[test]
  fn zero_sized_rect_produces_empty_bounds() {
    let bounds = bounds_for_rect(
      Size {
        width: 0.0,
        height: 100.0,
      },
      Affine::translation(50.0, 50.0),
    );

    assert!(
      bounds.is_some_and(|bounds| bounds.is_empty()),
      "zero-sized rect should produce empty bounds, got {:?}",
      bounds.map(|bounds| (bounds.left, bounds.top, bounds.right, bounds.bottom))
    );
  }

  #[test]
  fn merge_bounds_ignores_empty_bounds() {
    let empty = SceneBounds {
      left: 0,
      top: 5,
      right: 0,
      bottom: 10,
    };
    let real = SceneBounds {
      left: 1000,
      top: 0,
      right: 1200,
      bottom: 50,
    };

    for (left, right) in [(empty, real), (real, empty)] {
      let merged = merge_bounds(Some(left), Some(right));
      assert!(
        merged.is_some_and(
          |bounds| (bounds.left, bounds.top, bounds.right, bounds.bottom)
            == (real.left, real.top, real.right, real.bottom)
        ),
        "empty bounds must not expand the union"
      );
    }
  }
}
