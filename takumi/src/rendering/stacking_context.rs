use std::mem::replace;

use image::RgbaImage;
use taffy::{NodeId, geometry::Size};

use crate::{
  Result,
  layout::{
    style::{
      Affine, ComputedStyle, Display, Filter, ImageScalingAlgorithm, SpacePair,
      apply_backdrop_filter, apply_filters,
    },
    tree::{LayoutResults, RenderNode},
  },
  rendering::{
    BorderProperties, Canvas, CanvasConstrain, CanvasConstrainResult, Sizing, draw_debug_border,
    overlay_image,
  },
};

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

fn get_node_mut_by_path<'a, 'g>(
  root: &'a mut RenderNode<'g>,
  path: &[usize],
) -> Option<&'a mut RenderNode<'g>> {
  let mut current = root;
  for &index in path {
    let children = current.children.as_deref_mut()?;
    current = children.get_mut(index)?;
  }
  Some(current)
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
  render_child_len: usize,
) -> Result<Vec<OrderedChild>> {
  let layout_children = layout_results.children(node_id)?;
  let child_count = render_child_len.min(layout_children.len());
  Ok(
    layout_children
      .iter()
      .copied()
      .take(child_count)
      .enumerate()
      .map(|(render_index, node_id)| OrderedChild {
        render_index,
        node_id,
      })
      .collect(),
  )
}

#[derive(Clone)]
struct NodePaint {
  path: Vec<usize>,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
}

enum DeferredNodeRender {
  Deferred {
    path: Vec<usize>,
    has_constrain: bool,
    original_canvas_image: Option<RgbaImage>,
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
    self
      .inflow
      .sort_by(|left, right| left.source_order.cmp(&right.source_order));
    self
      .auto_zero
      .sort_by(|left, right| left.source_order.cmp(&right.source_order));
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
}

impl StackingContextNode {
  fn with_root(root: Option<NodePaint>) -> Self {
    Self {
      root,
      buckets: StackingBuckets::default(),
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
      unreachable!()
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

    let layout_children = collect_layout_children(layout_results, visit.node_id, children.len())?;
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

  Ok(contexts)
}

fn finish_node_render<'g>(
  node: &mut RenderNode<'g>,
  canvas: &mut Canvas,
  has_constrain: bool,
  original_canvas_image: Option<RgbaImage>,
) -> Result<()> {
  let opacity_filter =
    (node.context.style.opacity.0 < 1.0).then_some(Filter::Opacity(node.context.style.opacity));

  if !node.context.style.filter.is_empty() || opacity_filter.is_some() {
    apply_filters(
      &mut canvas.image,
      &node.context.sizing,
      node.context.current_color,
      &mut canvas.buffer_pool,
      node
        .context
        .style
        .filter
        .iter()
        .chain(opacity_filter.iter()),
    )?;
  }

  if let Some(mut source_canvas_image) = original_canvas_image {
    overlay_image(
      &mut source_canvas_image,
      &canvas.image,
      BorderProperties::zero(),
      Affine::IDENTITY,
      ImageScalingAlgorithm::Auto,
      node.context.style.mix_blend_mode,
      &[],
      &mut canvas.mask_memory,
      &mut canvas.buffer_pool,
    );

    let isolated_image = replace(&mut canvas.image, source_canvas_image);
    canvas.buffer_pool.release_image(isolated_image);
  }

  if has_constrain {
    canvas.pop_constrain();
  }

  Ok(())
}

fn begin_node_render<'g>(
  root: &mut RenderNode<'g>,
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  node_paint: &NodePaint,
  defer_finish: bool,
) -> Result<Option<DeferredNodeRender>> {
  let Some(current) = get_node_mut_by_path(root, &node_paint.path) else {
    unreachable!()
  };
  let layout = *layout_results.layout(node_paint.node_id)?;

  if current.context.style.is_invisible() || !node_paint.transform.is_invertible() {
    return Ok(None);
  }

  current.context.sizing.container_size = node_paint.container_size;
  current.context.transform = node_paint.transform;

  let constrain = CanvasConstrain::from_node(
    &current.context,
    &current.context.style,
    layout,
    node_paint.transform,
    &mut canvas.mask_memory,
    &mut canvas.buffer_pool,
  )?;
  if matches!(constrain, CanvasConstrainResult::SkipRendering) {
    return Ok(Some(DeferredNodeRender::SkipRendering));
  }

  let has_constrain = constrain.is_some();

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
  let original_canvas_image = if should_isolate {
    Some(canvas.replace_new_image()?)
  } else {
    None
  };

  match constrain {
    CanvasConstrainResult::None => {
      current.draw_shell(canvas, layout)?;
    }
    CanvasConstrainResult::Some(constrain) => match constrain {
      CanvasConstrain::ClipPath { .. } | CanvasConstrain::MaskImage { .. } => {
        canvas.push_constrain(constrain);
        current.draw_shell(canvas, layout)?;
      }
      CanvasConstrain::Overflow { .. } => {
        current.draw_shell(canvas, layout)?;
        canvas.push_constrain(constrain);
      }
    },
    CanvasConstrainResult::SkipRendering => unreachable!(),
  }

  current.draw_content(canvas, layout)?;

  if current.context.draw_debug_border {
    draw_debug_border(canvas, layout, node_paint.transform);
  }

  if current.should_create_inline_layout() {
    current.draw_inline(canvas, layout)?;
    finish_node_render(current, canvas, has_constrain, original_canvas_image)?;
    return Ok(None);
  }

  if defer_finish {
    return Ok(Some(DeferredNodeRender::Deferred {
      path: node_paint.path.clone(),
      has_constrain,
      original_canvas_image,
    }));
  }

  finish_node_render(current, canvas, has_constrain, original_canvas_image)?;
  Ok(None)
}

fn paint_single_node<'g>(
  root: &mut RenderNode<'g>,
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  node_paint: &NodePaint,
) -> Result<()> {
  match begin_node_render(root, layout_results, canvas, node_paint, false)? {
    Some(DeferredNodeRender::SkipRendering) | None => {}
    Some(DeferredNodeRender::Deferred { .. }) => unreachable!(),
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
    unreachable!()
  };

  let mut deferred_root = None;
  if let Some(root_paint) = &context.root {
    match begin_node_render(root, layout_results, canvas, root_paint, true)? {
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
    has_constrain,
    original_canvas_image,
  }) = deferred_root
  {
    let Some(current) = get_node_mut_by_path(root, &path) else {
      unreachable!()
    };
    finish_node_render(current, canvas, has_constrain, original_canvas_image)?;
  }

  Ok(())
}
