//! Backend-agnostic paint scene: the stacking-context tree that decides paint order, grouping, and
//! bounds.

use rustc_hash::FxHashMap;
#[cfg(feature = "fragmentation")]
use rustc_hash::FxHashSet;

use crate::{
  context::{BoxId, RenderContext},
  error::{Error, Result},
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, NodeId, Point, Size, transformed_rect_extents},
  layout::{
    decoration::OutlineGeometry,
    inline::InkBox,
    layout_view::LayoutView,
    tree::{ContainingBlocks, LayoutResults, OrderedChild, RenderNode},
  },
  paint_chunk::PaintChunk,
  paint_property::{
    ContainerContents, LineClampFloatClip, NodeProperties, PropertyState, PropertyTrees,
  },
  painter::OwnContent,
  shadow::SizedShadow,
  sort_key::sort_by_key,
  style::{Affine, BlurType, ComputedStyle, Display, Float},
  viewport::Viewport,
};
#[cfg(feature = "fragmentation")]
use std::collections::BTreeSet;
use std::collections::HashMap;

/// A node's resolved paint inputs.
#[derive(Clone)]
pub struct NodePaint {
  /// Child-index path from the root to this node.
  pub path: Vec<usize>,
  /// Layout id of the node.
  pub node_id: NodeId,
  /// Which of the node's fragments it paints.
  pub fragment_index: usize,
  /// Accumulated transform applied when painting.
  pub transform: Affine,
  /// Blink's paint offset of the border box: where it sits in the space paint snaps to pixels in.
  pub paint_offset: Point<f32>,
  /// Containing-block size; `None` on an axis is indefinite.
  pub container_size: Size<Option<f32>>,
  /// Device-space bounds of the paint output, if any.
  pub paint_bounds: Option<SceneBounds>,
  /// The clips and effects it paints under.
  pub properties: NodeProperties,
}

/// Device-space integer bounds of a node or stacking context's paint output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneBounds {
  /// Left edge, inclusive; negative past the canvas's left edge.
  pub left: i32,
  /// Top edge, inclusive; negative past the canvas's top edge.
  pub top: i32,
  /// Right edge, exclusive.
  pub right: i32,
  /// Bottom edge, exclusive.
  pub bottom: i32,
}

impl SceneBounds {
  /// The pixels a rectangle of `size` at the origin covers under `transform`.
  pub fn of_rect(size: Size<f32>, transform: Affine) -> Option<Self> {
    let (min_x, min_y, max_x, max_y) = transformed_rect_extents(Point::ZERO, size, transform)?;
    // Empty bounds mean "paints nothing"; None means "unknown" and forces full-viewport isolation.
    Some(Self::enclosing(min_x, min_y, max_x, max_y))
  }

  /// Whether the bounds enclose zero area.
  pub fn is_empty(self) -> bool {
    self.left >= self.right || self.top >= self.bottom
  }

  /// The pixels a rectangle from `(left, top)` to `(right, bottom)` touches, as
  /// `gfx::ToEnclosingRect` rounds it.
  pub fn enclosing(left: f32, top: f32, right: f32, bottom: f32) -> Self {
    Self {
      left: left.floor() as i32,
      top: top.floor() as i32,
      right: right.ceil() as i32,
      bottom: bottom.ceil() as i32,
    }
  }

  /// The smallest bounds holding both, as `gfx::Rect::Union` builds them.
  pub fn union(self, other: Self) -> Self {
    Self {
      left: self.left.min(other.left),
      top: self.top.min(other.top),
      right: self.right.max(other.right),
      bottom: self.bottom.max(other.bottom),
    }
  }
}

/// What a [`PaintItem`] paints.
#[derive(Clone)]
pub enum PaintItemKind {
  /// A single node's paint inputs.
  Node(NodePaint),
  /// Index of a nested stacking context.
  Context(usize),
  /// A table's collapsed borders, which paint once its cells' backgrounds have, as Blink paints
  /// them in the descendant block backgrounds phase.
  CollapsedBorders(NodePaint),
}

/// A paint entry plus its z-index and source order, which together order it uniquely.
#[derive(Clone)]
pub struct PaintItem {
  /// The node or nested stacking context to paint.
  pub kind: PaintItemKind,
  z_index: i32,
  source_order: usize,
  /// Whether an in-flow item paints whole with the content, as a flex or grid item does.
  atomic: bool,
}

impl PaintItem {
  fn z_order(&self) -> (i32, usize) {
    (self.z_index, self.source_order)
  }

  /// What the item paints in a phase painting `part`: a nested context paints whole once.
  pub(crate) fn part_in(&self, part: BoxPart) -> Option<BoxPart> {
    match (&self.kind, part) {
      (PaintItemKind::Node(_), part) => Some(part),
      (PaintItemKind::CollapsedBorders(_), BoxPart::Content) => None,
      (PaintItemKind::CollapsedBorders(_), part) => Some(part),
      (PaintItemKind::Context(_), BoxPart::Whole) => Some(BoxPart::Whole),
      (PaintItemKind::Context(_), BoxPart::Decorations) => (!self.atomic).then_some(BoxPart::Whole),
      (PaintItemKind::Context(_), BoxPart::Content) => self.atomic.then_some(BoxPart::Whole),
    }
  }
}

#[derive(Clone, Copy)]
/// The buckets of [CSS 2.1 Appendix E](https://www.w3.org/TR/CSS21/zindex.html) paint order.
enum PaintBucket {
  /// Negative `z-index`.
  Negative,
  /// In-flow, non-positioned boxes.
  InFlow,
  /// Non-positioned floats.
  Float,
  /// Positioned boxes and stacking contexts at `z-index: auto` or `0`, in tree order.
  Positioned,
  /// Positive `z-index`.
  Positive,
}

#[derive(Default)]
struct StackingBuckets {
  negative: Vec<PaintItem>,
  in_flow: Vec<PaintItem>,
  floats: Vec<PaintItem>,
  positioned: Vec<PaintItem>,
  positive: Vec<PaintItem>,
}

impl StackingBuckets {
  fn push(&mut self, bucket: PaintBucket, item: PaintItem) {
    match bucket {
      PaintBucket::Negative => self.negative.push(item),
      PaintBucket::InFlow => self.in_flow.push(item),
      PaintBucket::Float => self.floats.push(item),
      PaintBucket::Positioned => self.positioned.push(item),
      PaintBucket::Positive => self.positive.push(item),
    }
  }

  /// Orders the z-indexed buckets; the others are pushed in tree order already.
  fn sort(&mut self) {
    sort_by_key(&mut self.negative, PaintItem::z_order);
    sort_by_key(&mut self.positive, PaintItem::z_order);
  }

  /// Frees the room the buckets grew past their items: most boxes paint one item or none, and a
  /// scene keeps them all until the render ends.
  fn shrink_to_fit(&mut self) {
    self.negative.shrink_to_fit();
    self.in_flow.shrink_to_fit();
    self.floats.shrink_to_fit();
    self.positioned.shrink_to_fit();
    self.positive.shrink_to_fit();
  }

  fn in_paint_order(&self) -> [&[PaintItem]; 5] {
    [
      &self.negative,
      &self.in_flow,
      &self.floats,
      &self.positioned,
      &self.positive,
    ]
  }
}

/// Which part of a box a [`PaintPhase`] paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoxPart {
  /// Everything the box paints.
  Whole,
  /// Its shadows, background and border.
  Decorations,
  /// Its text, replaced content and outline.
  Content,
}

/// One phase of painting a stacking context.
#[derive(Clone, Copy)]
pub(crate) enum PaintPhase<'a> {
  /// The context root's own content.
  RootContent,
  /// Items, each painting what [`PaintItem::part_in`] says for `part`.
  Items(&'a [PaintItem], BoxPart),
}

/// One stacking context: an optional root node and its descendants bucketed into CSS paint order.
pub struct StackingContextNode {
  /// The node that owns this context, one paint per fragment for the root of a paged scene.
  roots: Vec<NodePaint>,
  buckets: StackingBuckets,
  paint_bounds: Option<SceneBounds>,
}

impl StackingContextNode {
  /// The node that owns this context, if any (the synthetic root has none).
  pub fn root(&self) -> Option<&NodePaint> {
    self.roots.first()
  }

  /// Each fragment of the node that owns this context, in page order.
  pub fn roots(&self) -> &[NodePaint] {
    &self.roots
  }

  /// The context's device-space paint bounds, once computed.
  pub fn paint_bounds(&self) -> Option<SceneBounds> {
    self.paint_bounds
  }

  /// Paint items grouped by stacking layer in paint order.
  pub fn in_paint_order(&self) -> [&[PaintItem]; 5] {
    self.buckets.in_paint_order()
  }

  /// The phases after the root's decorations, in [CSS 2.1 Appendix E](https://www.w3.org/TR/CSS21/zindex.html) order.
  pub(crate) fn paint_phases(&self) -> [PaintPhase<'_>; 7] {
    let buckets = &self.buckets;

    [
      PaintPhase::Items(&buckets.negative, BoxPart::Whole),
      PaintPhase::Items(&buckets.in_flow, BoxPart::Decorations),
      PaintPhase::Items(&buckets.floats, BoxPart::Whole),
      PaintPhase::RootContent,
      PaintPhase::Items(&buckets.in_flow, BoxPart::Content),
      PaintPhase::Items(&buckets.positioned, BoxPart::Whole),
      PaintPhase::Items(&buckets.positive, BoxPart::Whole),
    ]
  }

  fn with_root(root: Option<NodePaint>) -> Self {
    Self {
      roots: root.into_iter().collect(),
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
    atomic: bool,
  ) {
    self.buckets.push(
      bucket,
      PaintItem {
        kind,
        z_index,
        source_order,
        atomic,
      },
    );
  }
}

/// Where a box's children start: the device transform and Blink's paint offset of the space
/// their layout locations are measured in.
#[derive(Clone, Copy)]
struct ChildBase {
  transform: Affine,
  paint_offset: Point<f32>,
}

struct StackingContextBuildVisit {
  path: Vec<usize>,
  node_id: NodeId,
  fragment_index: usize,
  base: ChildBase,
  container_size: Size<Option<f32>>,
  /// The clip and effect the box starts from: its parent's contents state, with the clip of its
  /// containing block's contents when it is hoisted there.
  state: PropertyState,
  /// The context in-flow boxes and floats paint in.
  context_id: usize,
  /// The nearest stacking context, where positioned boxes paint.
  stacking_id: usize,
  /// The clip the nearest line-clamp container gives the floats it clips.
  line_clamp_float_clip: Option<LineClampFloatClip>,
  parent_display: Option<Display>,
  is_root: bool,
}

impl PaintBucket {
  /// The bucket a child with `style` paints in, and its z-index there.
  fn of(
    style: &ComputedStyle,
    is_flex_or_grid_item: bool,
    creates_stacking_context: bool,
  ) -> (Self, i32) {
    let z = style.paint_order_z(is_flex_or_grid_item);

    if z < 0 {
      (Self::Negative, z)
    } else if z > 0 {
      (Self::Positive, z)
    } else if creates_stacking_context
      || style.participates_in_positioned_paint_bucket(is_flex_or_grid_item)
    {
      (Self::Positioned, 0)
    } else if style.base_data.float != Float::None && !is_flex_or_grid_item {
      (Self::Float, 0)
    } else {
      (Self::InFlow, 0)
    }
  }

  /// Whether the bucket belongs to the nearest stacking context, not the nearest atomic box.
  fn lifts(&self) -> bool {
    matches!(self, Self::Negative | Self::Positioned | Self::Positive)
  }
}

/// What a scene is built from.
#[derive(Clone, Copy)]
pub struct SceneRequest<'a> {
  /// The tree to paint.
  pub root: &'a RenderNode,
  /// Its layout.
  pub layout_results: &'a LayoutResults,
  /// The size percentages of the root resolve against.
  pub container_size: Size<Option<f32>>,
  /// Whether to compute each node's paint bounds, which the raster and SVG backends clip and cull by.
  pub paint_bounds: bool,
}

impl SceneRequest<'_> {
  /// Flattens the node tree into CSS-ordered stacking contexts for painting.
  pub fn build(self) -> Result<SceneLayers> {
    build_layers(
      self.root,
      self.layout_results.view(),
      self.container_size,
      self.paint_bounds,
      &[(0, Point::ZERO)],
      #[cfg(feature = "fragmentation")]
      None,
    )
  }
}

/// What a scene build saw of the out-of-flow boxes and their containing blocks.
#[cfg(feature = "fragmentation")]
#[derive(Default)]
struct BuildTrace {
  /// The fragments visited of boxes that positioned out-of-flow boxes as their containing block.
  containing_blocks: Vec<(NodeId, usize)>,
  /// The out-of-flow boxes among the children of the fragments visited, with their parents.
  out_of_flow: Vec<(NodeId, NodeId)>,
}

/// The scene layers of the root fragments `roots` names, laid out as `layout_results` says.
fn build_layers(
  root: &RenderNode,
  layout_results: LayoutView<'_>,
  container_size: Size<Option<f32>>,
  paint_bounds: bool,
  roots: &[(usize, Point<f32>)],
  #[cfg(feature = "fragmentation")] mut trace: Option<&mut BuildTrace>,
) -> Result<SceneLayers> {
  let mut contexts = vec![StackingContextNode::with_root(None)];
  let mut source_order = 0usize;
  let mut containing_blocks = ContainingBlocks::default();
  let mut properties = PropertyTrees::default();
  let mut contents: FxHashMap<NodeId, ContainerContents> = FxHashMap::default();
  let mut fragment_children: FxHashMap<NodeId, FragmentChildren> = FxHashMap::default();
  let mut hoisted = HoistedFragments::default();
  // Each table with collapsed borders being visited: the context its boxes paint in, and its paint.
  let mut collapsed_borders: AfterVisits<(usize, NodePaint)> = AfterVisits::default();
  // The first root fragment pops first, so its page paints first.
  let mut visits: Vec<StackingContextBuildVisit> = roots
    .iter()
    .rev()
    .map(
      |&(fragment_index, paint_offset)| StackingContextBuildVisit {
        path: Vec::new(),
        node_id: NodeId::ROOT,
        fragment_index,
        base: ChildBase {
          transform: Affine::translation(paint_offset.x, paint_offset.y),
          paint_offset,
        },
        container_size,
        state: PropertyState::default(),
        context_id: 0,
        stacking_id: 0,
        line_clamp_float_clip: None,
        parent_display: None,
        is_root: true,
      },
    )
    .collect();

  loop {
    // A table's collapsed borders follow every box inside it, once the last has been visited.
    while let Some((context_id, borders)) = collapsed_borders.pop_done(visits.len()) {
      contexts[context_id].push_item(
        PaintBucket::InFlow,
        PaintItemKind::CollapsedBorders(borders),
        0,
        source_order,
        false,
      );
      source_order += 1;
    }

    // A containing block's fragment visits the hoisted fragments it holds that no parent did,
    // once the last box inside it has been visited.
    while let Some((cb, deferred)) = hoisted.finished(visits.len()) {
      for mut visit in deferred.into_iter().rev() {
        let Some(container) = contents.get(&cb) else {
          continue;
        };

        (visit.base, visit.container_size) =
          containing_blocks.base_for(Some(cb), visit.base, visit.container_size);
        properties.release_escaped_effects(visit.state.effect, container);
        visit.state.clip = container.clip;
        visits.push(visit);
      }
    }

    let Some(visit) = visits.pop() else {
      break;
    };
    let Some(current) = root.node_at_path(&visit.path) else {
      return Err(Error::InvalidLayoutNode(visit.node_id.into()));
    };
    let layout = layout_results.fragment_layout(visit.node_id, visit.fragment_index)?;
    if current.context.style.is_invisible() || layout_results.is_hidden_for_paint(visit.node_id) {
      continue;
    }

    let local_transform = current.context.style.local_transform(
      layout.size.width,
      layout.size.height,
      &current.context.sizing,
    );
    let mut current_transform = visit.base.transform;
    current_transform *= Affine::translation(layout.location.x, layout.location.y);
    current_transform *= local_transform;
    if !current_transform.is_invertible() {
      continue;
    }
    let child_base = ChildBase {
      transform: current_transform,
      paint_offset: current
        .context
        .style
        .paint_offset_after_translation(visit.base.paint_offset + layout.location, local_transform),
    };
    containing_blocks.record_placement(visit.node_id, child_base);

    let state = match visit.line_clamp_float_clip {
      Some(clip) if layout_results.is_line_clamp_clipped_float(visit.node_id) => {
        clip.apply(&mut properties, visit.state, &visit.path)
      }
      _ => visit.state,
    };
    let line_clamp_float_clip = LineClampFloatClip::of(&current.context, layout, current_transform)
      .or(visit.line_clamp_float_clip);
    let node_properties = NodeProperties::build(
      &mut properties,
      state,
      &visit.path,
      &current.context,
      layout,
      current_transform,
      child_base.paint_offset,
    );

    contents.insert(
      visit.node_id,
      ContainerContents {
        clip: node_properties.contents.clip,
        path: visit.path.clone(),
        fragment_index: visit.fragment_index,
      },
    );
    hoisted.enter((visit.node_id, visit.fragment_index), visits.len());
    #[cfg(feature = "fragmentation")]
    if let Some(trace) = trace.as_deref_mut()
      && layout_results
        .hoisted_children(visit.node_id)
        .next()
        .is_some()
    {
      trace
        .containing_blocks
        .push((visit.node_id, visit.fragment_index));
    }

    let node_paint = NodePaint {
      path: visit.path.clone(),
      node_id: visit.node_id,
      fragment_index: visit.fragment_index,
      transform: current_transform,
      paint_offset: child_base.paint_offset,
      container_size: visit.container_size,
      paint_bounds: paint_bounds
        .then(|| {
          compute_node_paint_bounds(current, visit.fragment_index, layout, current_transform)
        })
        .flatten(),
      properties: node_properties,
    };
    let table_borders = layout_results
      .table_fragment(visit.node_id, visit.fragment_index)
      .is_some_and(|fragment| fragment.borders.is_some())
      .then(|| node_paint.clone());

    let is_flex_or_grid_item = visit
      .parent_display
      .is_some_and(|display| display.should_blockify_children());

    let creates_stacking_context = visit.is_root
      || current
        .context
        .style
        .creates_stacking_context(is_flex_or_grid_item);

    let mut context_id = visit.context_id;
    let mut stacking_id = visit.stacking_id;

    if visit.is_root {
      contexts[0].roots.push(node_paint);
    } else {
      let (bucket, z_index) = PaintBucket::of(
        &current.context.style,
        is_flex_or_grid_item,
        creates_stacking_context,
      );
      let parent = if bucket.lifts() {
        visit.stacking_id
      } else {
        visit.context_id
      };
      // Atomic boxes paint as if stacking contexts, but lift positioned descendants to the real one.
      let atomic = !matches!(bucket, PaintBucket::InFlow)
        || is_flex_or_grid_item
        || current.participates_as_inline_box();

      if creates_stacking_context || atomic {
        let child_context = contexts.len();

        contexts.push(StackingContextNode::with_root(Some(node_paint)));
        contexts[parent].push_item(
          bucket,
          PaintItemKind::Context(child_context),
          z_index,
          source_order,
          atomic,
        );
        context_id = child_context;
        if creates_stacking_context {
          stacking_id = child_context;
        }
      } else {
        contexts[parent].push_item(
          bucket,
          PaintItemKind::Node(node_paint),
          z_index,
          source_order,
          false,
        );
      }
      source_order += 1;
    }

    if current.children.is_none() {
      continue;
    }

    // An inline formatting context paints its text itself; its layout children are the
    // out-of-flow boxes, atomic inlines, and floats inside that content, which paint as boxes
    // of their own.
    if let Some(borders) = table_borders {
      collapsed_borders.push(visits.len(), (context_id, borders));
    }

    let layout_children = layout_results.box_children(visit.node_id)?;
    // A box broken across pages visits only the children each of its fragments holds.
    let held = (layout_results
      .fragment_containers(visit.node_id)
      .nth(1)
      .is_some())
    .then(|| {
      #[cfg(feature = "fragmentation")]
      if let Some(linked) = layout_results.linked_children(visit.node_id, visit.fragment_index) {
        return linked;
      }
      fragment_children
        .entry(visit.node_id)
        .or_insert_with(|| FragmentChildren::of(layout_results, visit.node_id, layout_children))
        .in_fragment(visit.fragment_index)
    });
    let positions: Box<dyn Iterator<Item = usize>> = match &held {
      Some(positions) => Box::new(positions.iter().rev().copied()),
      None => Box::new((0..layout_children.len()).rev()),
    };
    let child_container_size = Size {
      width: Some(layout.content_box_width()),
      height: Some(layout.content_box_height()),
    };
    containing_blocks.record_content_box(visit.node_id, child_container_size);

    for position in positions {
      let child = &layout_children[position];
      let hoisted_cb = layout_results.hoisted_cb(visit.node_id, child);
      // A hoisted box belongs to the fragment of its containing block that positioned it.
      let container_fragment_index = hoisted_cb
        .and_then(|cb| contents.get(&cb))
        .map_or(visit.fragment_index, |container| container.fragment_index);
      let child_path = || {
        let mut path = visit.path.clone();

        child.extend_path(&mut path);
        path
      };

      #[cfg(feature = "fragmentation")]
      if let Some(trace) = trace.as_deref_mut()
        && root
          .node_at_path(&child_path())
          .is_some_and(|node| node.context.style.base_data.position.is_out_of_flow())
      {
        trace.out_of_flow.push((child.node_id, visit.node_id));
      }

      let child_visit = |fragment_index, base, container_size, clip| StackingContextBuildVisit {
        path: child_path(),
        node_id: child.node_id,
        fragment_index,
        base,
        container_size,
        state: PropertyState {
          clip,
          effect: node_properties.contents.effect,
        },
        context_id,
        stacking_id,
        line_clamp_float_clip,
        parent_display: Some(current.context.style.base_data.display),
        is_root: false,
      };

      for child_fragment_index in
        layout_results.fragments_in(child.node_id, container_fragment_index)
      {
        if hoisted_cb.is_some() && !hoisted.claim(child.node_id, child_fragment_index) {
          continue;
        }

        let (base, base_container) =
          containing_blocks.base_for(hoisted_cb, child_base, child_container_size);
        let clip = match hoisted_cb.and_then(|cb| contents.get(&cb)) {
          Some(container) => {
            properties.release_escaped_effects(node_properties.contents.effect, container);
            container.clip
          }
          None => node_properties.contents.clip,
        };

        visits.push(child_visit(
          child_fragment_index,
          base,
          base_container,
          clip,
        ));
      }
      #[cfg(feature = "fragmentation")]
      if let Some(cb) = hoisted_cb
        && contents.contains_key(&cb)
      {
        for (child_fragment_index, cb_fragment_index) in layout_results
          .fragment_containers(child.node_id)
          .filter(|&(_, index)| index > container_fragment_index)
        {
          hoisted.defer(
            (cb, cb_fragment_index),
            child_visit(child_fragment_index, child_base, child_container_size, None),
          );
        }
      }
    }
  }

  for context in &mut contexts {
    context.buckets.sort();
    context.buckets.shrink_to_fit();
  }
  contexts.shrink_to_fit();

  let mut layers = SceneLayers {
    contexts,
    properties,
  };

  if paint_bounds {
    layers.set_bounds(root);
  }
  Ok(layers)
}

/// A scene's stacking contexts and the property trees their boxes paint under.
pub struct SceneLayers {
  /// The stacking contexts; the first is the synthetic root.
  pub contexts: Vec<StackingContextNode>,
  /// The clips and effects.
  pub properties: PropertyTrees,
}

impl SceneLayers {
  fn set_bounds(&mut self, root: &RenderNode) {
    // `None` means "unknown extent" and poisons the union; dropping it would
    // under-report the context and clip or cull visible paint.
    for context_id in (0..self.contexts.len()).rev() {
      let mut paint_bounds = None;
      let mut unknown = false;
      for root in &self.contexts[context_id].roots {
        match root.paint_bounds {
          Some(bounds) => paint_bounds = merge_bounds(paint_bounds, Some(bounds)),
          None => unknown = true,
        }
      }
      for bucket in self.contexts[context_id].buckets.in_paint_order() {
        for item in bucket {
          let item_bounds = match &item.kind {
            PaintItemKind::Node(node_paint) | PaintItemKind::CollapsedBorders(node_paint) => {
              node_paint.paint_bounds
            }
            PaintItemKind::Context(child_context_id) => {
              self.contexts[*child_context_id].paint_bounds
            }
          };
          match item_bounds {
            Some(bounds) => paint_bounds = merge_bounds(paint_bounds, Some(bounds)),
            None => unknown = true,
          }
        }
      }
      if let Some(root_paint) = self.contexts[context_id].roots.first()
        && let Some(root_node) = root.node_at_path(&root_paint.path)
      {
        paint_bounds = outset_bounds(
          paint_bounds,
          filter_reach(&root_node.context),
          root_paint.transform,
        );
      }
      self.contexts[context_id].paint_bounds = if unknown { None } else { paint_bounds };
    }

    set_effect_bounds(root, &self.contexts, &mut self.properties);
  }
}

/// A render tree laid out, with the stacking contexts that paint it.
pub struct Scene {
  /// The tree.
  pub root: RenderNode,
  /// Its layout.
  pub results: LayoutResults,
  /// Its stacking contexts; the first is the synthetic root.
  pub contexts: Vec<StackingContextNode>,
  /// The clips and effects its boxes paint under.
  pub properties: PropertyTrees,
  /// The size it paints at: the viewport on a definite axis, the root's border box otherwise.
  pub size: Size<f32>,
  /// The boxes it paints more than one fragment of, set up before each paints.
  pub fragments: Option<FragmentPlacements>,
}

impl Scene {
  /// Lays `root` out in `viewport` and builds its scene at the origin.
  pub fn lay_out(root: RenderNode, viewport: Viewport, paint_bounds: bool) -> Result<Self> {
    root.clear_layout_caches();
    let results = LayoutResults::compute_for_paint(&root, viewport.into());
    let container_size = Size::from(viewport.size);
    let size = container_size.zip_map(results.layout(NodeId::ROOT)?.size, Option::unwrap_or);
    let layers = SceneRequest {
      root: &root,
      layout_results: &results,
      container_size,
      paint_bounds,
    }
    .build()?;

    Ok(Self::new(root, results, layers, size))
  }

  /// The scene of `root` laid out as `results` and stacked as `layers`, its boxes' contexts set to
  /// their paint offsets.
  pub(crate) fn new(
    root: RenderNode,
    results: LayoutResults,
    layers: SceneLayers,
    size: Size<f32>,
  ) -> Self {
    set_paint_offsets(&root, &layers.placements(results.view()));

    let SceneLayers {
      contexts,
      properties,
    } = layers;

    Self {
      root,
      results,
      contexts,
      properties,
      size,
      fragments: None,
    }
  }
}

impl Scene {
  /// The scene as the painters read it.
  pub fn view(&self) -> SceneView<'_> {
    SceneView {
      root: &self.root,
      results: self.results.view(),
      contexts: &self.contexts,
      properties: &self.properties,
      fragments: self.fragments.as_ref(),
    }
  }
}

/// A scene as the painters read it: the scene of a whole tree, or a window of a paged layout.
#[derive(Clone, Copy)]
pub struct SceneView<'a> {
  /// The tree.
  pub root: &'a RenderNode,
  /// The layout.
  pub results: LayoutView<'a>,
  /// The stacking contexts; the first is the synthetic root.
  pub contexts: &'a [StackingContextNode],
  /// The clips and effects the boxes paint under.
  pub properties: &'a PropertyTrees,
  fragments: Option<&'a FragmentPlacements>,
}

impl SceneView<'_> {
  /// The layout of the fragment `paint` paints, with its box set to paint it.
  pub fn enter(&self, paint: &NodePaint) -> Result<ComputedLayout> {
    let layout = self
      .results
      .fragment_layout(paint.node_id, paint.fragment_index)?;

    if let Some(fragments) = self.fragments {
      fragments.enter(self.root, paint, layout.location);
    }
    Ok(layout)
  }

  /// Visits every node paint of the scene, in paint order.
  pub fn for_each_paint(self, mut visit: impl FnMut(&NodePaint)) {
    fn walk(view: SceneView<'_>, id: usize, visit: &mut impl FnMut(&NodePaint)) {
      let Some(context) = view.contexts.get(id) else {
        return;
      };

      for paint in context.roots() {
        visit(paint);
      }
      for bucket in context.in_paint_order() {
        for item in bucket {
          match &item.kind {
            PaintItemKind::Node(paint) => visit(paint),
            PaintItemKind::Context(child) => walk(view, *child, visit),
            PaintItemKind::CollapsedBorders(_) => {}
          }
        }
      }
    }

    walk(self, 0, &mut visit);
  }
}

/// What to release once the visits left fall back to the count recorded with it.
struct AfterVisits<T>(Vec<(usize, T)>);

impl<T> AfterVisits<T> {
  fn push(&mut self, visits_left: usize, item: T) {
    self.0.push((visits_left, item));
  }

  fn pop_done(&mut self, visits_len: usize) -> Option<T> {
    if self.0.last()?.0 < visits_len {
      return None;
    }
    self.0.pop().map(|(_, item)| item)
  }
}

impl<T> Default for AfterVisits<T> {
  fn default() -> Self {
    Self(Vec::new())
  }
}

/// The fragments of hoisted boxes a scene visits once each, as Blink's `FragmentedOofInCb` places
/// them.
#[cfg(feature = "fragmentation")]
#[derive(Default)]
struct HoistedFragments {
  /// By containing block fragment, the visits its hoisted fragments take there.
  deferred: FxHashMap<(NodeId, usize), Vec<StackingContextBuildVisit>>,
  claimed: FxHashSet<(NodeId, usize)>,
  /// The containing block fragments with deferred visits being visited.
  entered: AfterVisits<(NodeId, usize)>,
}

#[cfg(feature = "fragmentation")]
impl HoistedFragments {
  /// Whether the fragment `fragment_index` of `node` is still to visit, which it then no longer is.
  fn claim(&mut self, node: NodeId, fragment_index: usize) -> bool {
    self.claimed.insert((node, fragment_index))
  }

  fn defer(&mut self, container: (NodeId, usize), visit: StackingContextBuildVisit) {
    self.deferred.entry(container).or_default().push(visit);
  }

  fn enter(&mut self, container: (NodeId, usize), visits_left: usize) {
    if self.deferred.contains_key(&container) {
      self.entered.push(visits_left, container);
    }
  }

  /// The containing block whose boxes are done with `visits_len` visits left, and the visits
  /// deferred to its fragment that no parent claimed.
  fn finished(&mut self, visits_len: usize) -> Option<(NodeId, Vec<StackingContextBuildVisit>)> {
    let container = self.entered.pop_done(visits_len)?;
    let mut deferred = self.deferred.remove(&container).unwrap_or_default();

    deferred.retain(|visit| self.claimed.insert((visit.node_id, visit.fragment_index)));
    Some((container.0, deferred))
  }
}

#[cfg(not(feature = "fragmentation"))]
#[derive(Default)]
struct HoistedFragments {}

#[cfg(not(feature = "fragmentation"))]
impl HoistedFragments {
  fn claim(&mut self, _node: NodeId, _fragment_index: usize) -> bool {
    true
  }

  fn enter(&mut self, _container: (NodeId, usize), _visits_left: usize) {}

  fn finished(&mut self, _visits_len: usize) -> Option<(NodeId, Vec<StackingContextBuildVisit>)> {
    None
  }
}

/// A fragmented box's children by the fragment of it that holds them, as positions among its box
/// children, and its hoisted children, which belong to the fragment of their containing block.
pub(crate) struct FragmentChildren {
  held: Vec<Vec<usize>>,
  hoisted: Vec<usize>,
}

impl FragmentChildren {
  pub(crate) fn of(results: LayoutView<'_>, parent: NodeId, children: &[OrderedChild]) -> Self {
    let mut held: Vec<Vec<usize>> = Vec::new();
    let mut hoisted = Vec::new();

    for (position, child) in children.iter().enumerate() {
      if results.hoisted_cb(parent, child).is_some() {
        hoisted.push(position);
        continue;
      }
      for (_, container) in results.fragment_containers(child.node_id) {
        if held.len() <= container {
          held.resize_with(container + 1, Vec::new);
        }
        if held[container].last() != Some(&position) {
          held[container].push(position);
        }
      }
    }
    Self { held, hoisted }
  }

  /// The positions the box's fragment `fragment_index` visits, in child order.
  pub(crate) fn in_fragment(&self, fragment_index: usize) -> Vec<usize> {
    let mut positions: Vec<usize> = self
      .held
      .get(fragment_index)
      .into_iter()
      .flatten()
      .chain(&self.hoisted)
      .copied()
      .collect();

    positions.sort_unstable();
    positions
  }
}

/// For a scene that paints several fragments of a box: by each box's child path, the indices of
/// its children that no paint of their own places.
pub struct FragmentPlacements(FxHashMap<BoxId, Vec<usize>>);

impl FragmentPlacements {
  /// The placements of the content of the boxes at `placed`, whose paths paint on their own: only
  /// a box with inline content, or children that are not boxes of their own, has any.
  #[cfg(feature = "fragmentation")]
  fn new(
    root: &RenderNode,
    layout: LayoutView<'_>,
    placed: &HashMap<Vec<usize>, ScenePlacement>,
  ) -> Self {
    let mut unplaced = FxHashMap::default();
    let mut stack: Vec<(&RenderNode, Vec<usize>)> = placed
      .iter()
      .filter_map(|(path, placement)| {
        let node = root.node_at_path(path)?;
        let has_boxes_only = !node.should_create_inline_layout()
          && layout
            .box_children(placement.node_id)
            .is_ok_and(|children| children.len() == node.children.as_deref().map_or(0, <[_]>::len));

        (!has_boxes_only).then(|| (node, path.clone()))
      })
      .collect();

    while let Some((node, path)) = stack.pop() {
      let mut children = Vec::new();

      for (index, child) in node.children.iter().flatten().enumerate() {
        let mut child_path = path.clone();

        child_path.push(index);
        if !placed.contains_key(&child_path) {
          children.push(index);
          stack.push((child, child_path));
        }
      }
      if !children.is_empty() {
        unplaced.insert(node.context.box_id(), children);
      }
    }
    Self(unplaced)
  }

  /// Sets the box `paint` paints, laid out at `location`, and the boxes of its content no other
  /// paint places, to paint the fragment `paint` names, as [`set_paint_offsets`] sets them for
  /// a scene with one fragment of each box.
  pub fn enter(&self, root: &RenderNode, paint: &NodePaint, location: Point<f32>) {
    let Some(node) = root.node_at_path(&paint.path) else {
      return;
    };

    node
      .context
      .enter_fragment(paint.paint_offset - location, paint.fragment_index);

    let mut stack: Vec<_> = self
      .unplaced_children(node)
      .map(|child| (child, paint.paint_offset))
      .collect();

    while let Some((node, base)) = stack.pop() {
      node.context.enter_fragment(base, paint.fragment_index);
      stack.extend(self.unplaced_children(node).map(|child| (child, base)));
    }
  }

  /// The children of `node` no paint of their own places.
  fn unplaced_children<'n>(&'n self, node: &'n RenderNode) -> impl Iterator<Item = &'n RenderNode> {
    self
      .0
      .get(&node.context.box_id())
      .into_iter()
      .flatten()
      .filter_map(|&index| node.children.as_deref()?.get(index))
  }
}

impl SceneLayers {
  /// Where these layers place each box they paint, by child path.
  fn placements(&self, results: LayoutView<'_>) -> HashMap<Vec<usize>, ScenePlacement> {
    let mut placements = HashMap::new();

    for context in &self.contexts {
      let items = context.buckets.in_paint_order().into_iter().flatten();
      let paints = items.filter_map(|item| match &item.kind {
        PaintItemKind::Node(paint) | PaintItemKind::CollapsedBorders(paint) => Some(paint),
        PaintItemKind::Context(_) => None,
      });

      for paint in context.roots.iter().chain(paints) {
        if let Ok(layout) = results.fragment_layout(paint.node_id, paint.fragment_index) {
          placements.insert(
            paint.path.clone(),
            ScenePlacement {
              #[cfg(feature = "fragmentation")]
              node_id: paint.node_id,
              paint_offset: paint.paint_offset,
              location: layout.location,
              fragment_index: paint.fragment_index,
            },
          );
        }
      }
    }
    placements
  }
}

/// The scene of every page stacked, which the windows of [`SceneRef`] are checked against.
#[cfg(feature = "fragmentation")]
pub struct PagedScene {
  scene: Scene,
  page_count: usize,
}

#[cfg(feature = "fragmentation")]
impl PagedScene {
  /// Lays `root` out on at most `max_pages` pages `page_size` and builds the scene of them all.
  pub fn lay_out(
    root: RenderNode,
    page_size: Size<f32>,
    max_pages: usize,
    paint_bounds: bool,
  ) -> Result<Self> {
    root.clear_layout_caches();
    let (results, page_count) = LayoutResults::compute_pages(&root, page_size, max_pages)?;
    // A page the root starts after holds none of it.
    let roots = page_roots(
      results.view(),
      page_size,
      &(0..page_count).collect::<Vec<_>>(),
    );
    let layers = build_layers(
      &root,
      results.view(),
      page_size.map(Some),
      paint_bounds,
      &roots,
      None,
    )?;
    let fragments =
      FragmentPlacements::new(&root, results.view(), &layers.placements(results.view()));
    let mut scene = Scene::new(root, results, layers, page_size);

    scene.fragments = Some(fragments);
    Ok(Self { scene, page_count })
  }

  /// The number of pages.
  pub fn page_count(&self) -> usize {
    self.page_count
  }

  /// The scene of every page stacked.
  pub fn into_scene(self) -> Scene {
    self.scene
  }
}

/// The scene of some of a paged layout's root fragments, built from the layout so far. It borrows
/// the tree and the layout instead of owning them, so painting one page does not keep the rest.
#[cfg(feature = "fragmentation")]
pub struct SceneRef<'a> {
  /// The tree.
  pub root: &'a RenderNode,
  /// The layout.
  pub results: LayoutView<'a>,
  /// Its stacking contexts; the first is the synthetic root.
  pub contexts: Vec<StackingContextNode>,
  /// The clips and effects its boxes paint under.
  pub properties: PropertyTrees,
  /// The boxes it paints more than one fragment of, set up before each paints.
  fragments: FragmentPlacements,
}

#[cfg(feature = "fragmentation")]
impl<'a> SceneRef<'a> {
  /// Where page `page` starts in the scene of pages `page_size`: the pages stack in the block
  /// direction.
  pub fn page_origin(page_size: Size<f32>, page: usize) -> Point<f32> {
    Point {
      x: 0.0,
      y: page_size.height * page as f32,
    }
  }

  /// The scene of the root's fragments on `pages`, which stack as in [`PagedScene`], provided
  /// `pages` holds every page [`PageWindows`] names for it.
  pub fn window(
    root: &'a RenderNode,
    layout: LayoutView<'a>,
    page_size: Size<f32>,
    pages: &[usize],
    paint_bounds: bool,
  ) -> Result<Self> {
    let layers = build_layers(
      root,
      layout,
      page_size.map(Some),
      paint_bounds,
      &page_roots(layout, page_size, pages),
      None,
    )?;
    let placements = layers.placements(layout);
    let fragments = FragmentPlacements::new(root, layout, &placements);

    for (path, placement) in &placements {
      if let Some(node) = root.node_at_path(path) {
        node.context.enter_fragment(
          placement.paint_offset - placement.location,
          placement.fragment_index,
        );
      }
    }

    let SceneLayers {
      contexts,
      properties,
    } = layers;

    Ok(Self {
      root,
      results: layout,
      contexts,
      properties,
      fragments,
    })
  }

  /// The scene as the painters read it.
  pub fn view(&self) -> SceneView<'_> {
    SceneView {
      root: self.root,
      results: self.results,
      contexts: &self.contexts,
      properties: &self.properties,
      fragments: Some(&self.fragments),
    }
  }
}

#[cfg(feature = "fragmentation")]
fn page_roots(
  layout: LayoutView<'_>,
  page_size: Size<f32>,
  pages: &[usize],
) -> Vec<(usize, Point<f32>)> {
  pages
    .iter()
    .filter_map(|&page| {
      layout
        .fragment_in(NodeId::ROOT, page)
        .map(|fragment_index| (fragment_index, SceneRef::page_origin(page_size, page)))
    })
    .collect()
}

#[cfg(feature = "fragmentation")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PageReach {
  Empty,
  /// It paints between two heights.
  Span {
    top: i32,
    bottom: i32,
  },
  /// A chunk's bounds are unknown, so it may paint on any page.
  Unbounded,
}

#[cfg(feature = "fragmentation")]
impl PageReach {
  fn union(self, other: Self) -> Self {
    match (self, other) {
      (Self::Empty, reach) | (reach, Self::Empty) => reach,
      (Self::Unbounded, _) | (_, Self::Unbounded) => Self::Unbounded,
      (
        Self::Span { top, bottom },
        Self::Span {
          top: other_top,
          bottom: other_bottom,
        },
      ) => Self::Span {
        top: top.min(other_top),
        bottom: bottom.max(other_bottom),
      },
    }
  }

  fn intersects(self, top: f32, bottom: f32) -> bool {
    match self {
      Self::Empty => false,
      Self::Span {
        top: reach_top,
        bottom: reach_bottom,
      } => (reach_top as f32) < bottom && (reach_bottom as f32) > top,
      Self::Unbounded => true,
    }
  }
}

#[cfg(feature = "fragmentation")]
impl SceneLayers {
  /// How far down the scene paints. A chunk under a filter paints as far as the filter spreads it,
  /// which the bounds of the effects around it hold.
  fn reach(&self) -> PageReach {
    let mut reach = PageReach::Empty;

    for chunk in PaintChunk::in_paint_order(&self.contexts) {
      let mut effect = chunk.state().effect;
      let mut bounds = vec![chunk.node.paint_bounds];

      while let Some(id) = effect {
        let node = self.properties.effect(id);

        bounds.push(node.bounds);
        effect = node.parent;
      }
      for bounds in bounds {
        match bounds {
          None => return PageReach::Unbounded,
          Some(bounds) if bounds.is_empty() => {}
          Some(bounds) => {
            reach = reach.union(PageReach::Span {
              top: bounds.top,
              bottom: bounds.bottom,
            });
          }
        }
      }
    }
    reach
  }
}

/// Which root fragments paint on each page of a paged layout, kept up to date by [`Self::refresh`].
#[cfg(feature = "fragmentation")]
pub struct PageWindows {
  page_size: Size<f32>,
  /// How far each page's root fragment paints.
  reaches: Vec<PageReach>,
  /// The pages each page's scene needs to paint the boxes they defer to its containing blocks.
  needs: Vec<Vec<usize>>,
  /// By out-of-flow box, the parents whose fragments pages visited, with those pages.
  parents: FxHashMap<NodeId, Vec<(NodeId, usize)>>,
  /// How many of the first pages were added once their fragments could no longer change.
  stable: usize,
}

#[cfg(feature = "fragmentation")]
impl PageWindows {
  /// Windows of pages `page_size`, none added yet.
  pub fn new(page_size: Size<f32>) -> Self {
    Self {
      page_size,
      reaches: Vec::new(),
      needs: Vec::new(),
      parents: FxHashMap::default(),
      stable: 0,
    }
  }

  /// Brings the pages up to `laid` from `layout`, measuring again those added before their fragments
  /// were final.
  pub fn refresh(
    &mut self,
    root: &RenderNode,
    layout: LayoutView<'_>,
    laid: usize,
    final_pages: usize,
  ) -> Result<()> {
    let stable = self.stable.min(self.reaches.len());

    self.reaches.truncate(stable);
    self.needs.truncate(stable);
    self.parents.retain(|_, parents| {
      parents.retain(|&(_, page)| page < stable);
      !parents.is_empty()
    });
    while self.reaches.len() < laid {
      self.add_page(root, layout)?;
    }
    self.stable = final_pages.min(laid);
    Ok(())
  }

  /// How many pages are added.
  pub fn page_count(&self) -> usize {
    self.reaches.len()
  }

  /// Measures how far the next page's root fragment paints.
  fn add_page(&mut self, root: &RenderNode, layout: LayoutView<'_>) -> Result<()> {
    let page = self.reaches.len();
    let mut trace = BuildTrace::default();
    let layers = build_layers(
      root,
      layout,
      self.page_size.map(Some),
      true,
      &page_roots(layout, self.page_size, &[page]),
      Some(&mut trace),
    )?;
    let mut reach = layers.reach();
    // A box an earlier page's parent of it hoisted to a fragment of its containing block this page
    // visited paints with this page, wherever the parent is.
    let mut needs: Vec<usize> = trace
      .containing_blocks
      .iter()
      .flat_map(|&(container, fragment_index)| {
        layout
          .hoisted_children(container)
          .filter(move |&child| layout.fragments_in(child, fragment_index).next().is_some())
          .filter_map(|child| self.parents.get(&child))
          .flatten()
          .filter(move |&&(parent, _)| parent != container)
          .map(|&(_, source)| source)
      })
      .collect();

    for (child, parent) in trace.out_of_flow {
      self.parents.entry(child).or_default().push((parent, page));
    }
    needs.sort_unstable();
    needs.dedup();
    self.needs.push(needs);

    if !self.needs[page].is_empty() {
      let pages = self.pages_of(page);

      reach = reach.union(
        build_layers(
          root,
          layout,
          self.page_size.map(Some),
          true,
          &page_roots(layout, self.page_size, &pages),
          None,
        )?
        .reach(),
      );
    }
    self.reaches.push(reach);
    Ok(())
  }

  /// The pages, in order, whose root fragments a scene needs to paint page `page` as the scene of
  /// every page would.
  pub fn pages_reaching(&self, page: usize) -> Vec<usize> {
    let top = self.page_size.height * page as f32;
    let bottom = top + self.page_size.height;
    let reaching = self
      .reaches
      .iter()
      .enumerate()
      .filter(|(_, reach)| reach.intersects(top, bottom))
      .map(|(page, _)| page);

    self.with_needs(reaching)
  }

  /// How many of the first `drawn` pages no scene of a later page paints from, counting from the
  /// `released` pages already known to be free. A scene paints from the pages reaching down to its
  /// page and from the parents of the out-of-flow boxes it paints, until `is_complete` says a
  /// box is laid out for good.
  pub fn releasable_pages(
    &mut self,
    released: usize,
    drawn: usize,
    is_complete: impl Fn(NodeId) -> bool,
  ) -> usize {
    self.parents.retain(|&child, _| !is_complete(child));

    let below = self.page_size.height * drawn as f32;
    // [`Self::refresh`] measures the pages from `stable` on again.
    let drawn = drawn.min(self.stable).min(self.reaches.len());
    let reaches = self.reaches.get(released..drawn).unwrap_or_default();
    let reaching = reaches
      .iter()
      .enumerate()
      .filter(|(_, reach)| reach.intersects(below, f32::INFINITY))
      .map(|(page, _)| released + page);
    let parents = self.parents.values().flatten().map(|&(_, page)| page);

    self
      .with_needs(reaching.chain(parents))
      .first()
      .map_or(drawn, |&first| first.min(drawn))
  }

  /// The pages whose root fragments a scene needs to paint the root fragment of `page`.
  pub fn pages_of(&self, page: usize) -> Vec<usize> {
    self.with_needs([page])
  }

  /// `pages` and every page their scenes need, in order.
  fn with_needs(&self, pages: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let mut included = BTreeSet::new();
    let mut stack: Vec<usize> = pages.into_iter().collect();

    while let Some(page) = stack.pop() {
      if included.insert(page) {
        stack.extend(&self.needs[page]);
      }
    }
    included.into_iter().collect()
  }
}

/// Where a scene placed a box.
#[derive(Clone, Copy)]
struct ScenePlacement {
  #[cfg(feature = "fragmentation")]
  node_id: NodeId,
  /// Blink's paint offset of the border box.
  paint_offset: Point<f32>,
  /// The layout location the paint offset adds to.
  location: Point<f32>,
  fragment_index: usize,
}

/// Sets the context of each box under `root` to the paint offset its layout location adds to and
/// the fragment it paints: from `offsets`, keyed by child path, for a box the scene placed, else
/// those of the box whose inline content it paints in.
fn set_paint_offsets(root: &RenderNode, offsets: &HashMap<Vec<usize>, ScenePlacement>) {
  let mut path = Vec::new();
  let mut stack = vec![(root, OffsetSlot::Root, Point::ZERO, 0)];

  while let Some((node, slot, inherited, inherited_fragment_index)) = stack.pop() {
    let placed = match slot {
      OffsetSlot::Root => offsets.get(path.as_slice()),
      OffsetSlot::Child { depth, index } => {
        path.truncate(depth);
        path.push(index);
        offsets.get(path.as_slice())
      }
    };
    let (base, own, fragment_index) = match placed {
      Some(placement) => (
        placement.paint_offset - placement.location,
        placement.paint_offset,
        placement.fragment_index,
      ),
      None => (inherited, inherited, inherited_fragment_index),
    };
    let depth = path.len();

    node.context.enter_fragment(base, fragment_index);
    for (index, child) in node.children.iter().flatten().enumerate() {
      stack.push((
        child,
        OffsetSlot::Child { depth, index },
        own,
        fragment_index,
      ));
    }
  }
}

/// Where a box sits in the child paths that key the scene's placed boxes.
#[derive(Clone, Copy)]
enum OffsetSlot {
  Root,
  /// Child `index` of the box whose path is `depth` long.
  Child {
    depth: usize,
    index: usize,
  },
}

/// Bounds each effect of `properties` by what paints under it, nested effects grown by their
/// owners' filters first.
fn set_effect_bounds(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  properties: &mut PropertyTrees,
) {
  let chunks = PaintChunk::in_paint_order(contexts);
  let owners = PaintChunk::effect_owners(&chunks, properties);
  let mut bounds = vec![(None, false); properties.effect_count()];

  for chunk in &chunks {
    if let Some(id) = chunk.state().effect {
      let (union, unknown) = &mut bounds[id.index()];

      match chunk.node.paint_bounds {
        Some(node_bounds) => *union = merge_bounds(*union, Some(node_bounds)),
        None => *unknown = true,
      }
    }
  }

  let ids: Vec<_> = properties.effect_ids().collect();

  for &id in ids.iter().rev() {
    let (union, unknown) = bounds[id.index()];
    let grown =
      match owners[id.index()].and_then(|owner| Some((owner, root.node_at_path(&owner.path)?))) {
        Some((owner, node)) => outset_bounds(union, filter_reach(&node.context), owner.transform),
        None => union,
      };

    if let Some(parent) = properties.effect(id).parent {
      let (parent_union, parent_unknown) = &mut bounds[parent.index()];

      *parent_union = merge_bounds(*parent_union, grown);
      *parent_unknown |= unknown;
    }

    properties.set_effect_bounds(id, (!unknown).then_some(grown).flatten());
  }
}

/// How far a shadow's ink reaches past the shape that casts it.
fn shadow_reach(shadow: &SizedShadow) -> f32 {
  shadow.offset_x.abs().max(shadow.offset_y.abs())
    + shadow.spread_radius.max(0.0)
    + BlurType::Shadow.extent(shadow.blur_radius)
}

/// How far the node's filters spread its layer, in local px.
pub(crate) fn filter_reach(context: &RenderContext) -> f32 {
  let sizing = &context.sizing;

  context
    .style
    .misc1_data
    .filter
    .iter()
    .map(|filter| filter.reach(sizing))
    .sum()
}

/// How far box shadows and the outline reach past the border box, in local px.
pub(crate) fn box_ink_reach(context: &RenderContext, size: Size<f32>) -> f32 {
  let shadows = context.style.misc1_data.box_shadow.iter().flatten();
  let shadow_reach = shadows
    .filter(|shadow| !shadow.inset)
    .map(|shadow| {
      shadow_reach(&SizedShadow::from_box_shadow(
        *shadow,
        &context.sizing,
        context.current_color,
        size,
      ))
    })
    .fold(0.0_f32, f32::max);
  let outline_reach =
    OutlineGeometry::painted(context, size).map_or(0.0, |outline| outline.grow.max(0.0));

  shadow_reach.max(outline_reach)
}

/// How far text shadows and the text stroke reach past glyph ink, in local px.
pub(crate) fn text_ink_reach(font_style: &SizedFontStyle) -> f32 {
  let shadow_reach = font_style
    .text_shadow
    .iter()
    .map(shadow_reach)
    .fold(0.0_f32, f32::max);

  shadow_reach.max(font_style.stroke_width)
}

/// Grows `bounds` by `reach` local px on every side, taking the transform's per-axis envelope.
fn outset_bounds(
  bounds: Option<SceneBounds>,
  reach: f32,
  transform: Affine,
) -> Option<SceneBounds> {
  let mut bounds = bounds?;
  if reach <= 0.0 {
    return Some(bounds);
  }

  let pad_x = (reach * (transform.a.abs() + transform.c.abs())).ceil() as i32;
  let pad_y = (reach * (transform.b.abs() + transform.d.abs())).ceil() as i32;
  bounds.left = bounds.left.saturating_sub(pad_x);
  bounds.top = bounds.top.saturating_sub(pad_y);
  bounds.right = bounds.right.saturating_add(pad_x);
  bounds.bottom = bounds.bottom.saturating_add(pad_y);
  Some(bounds)
}

fn compute_node_paint_bounds(
  node: &RenderNode,
  fragment_index: usize,
  layout: ComputedLayout,
  transform: Affine,
) -> Option<SceneBounds> {
  let mut bounds = outset_bounds(
    SceneBounds::of_rect(layout.size, transform),
    box_ink_reach(&node.context, layout.size),
    transform,
  );
  if !has_inline_paint_content(node) {
    return bounds;
  }

  let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);
  if font_style.sizing.font_size == 0.0 {
    return bounds;
  }

  let content_offset = layout.content_box_offset();
  let inline_transform = Affine::translation(content_offset.x, content_offset.y) * transform;

  let Some(items) = node.fragment_items_of(fragment_index, layout) else {
    return bounds;
  };
  let placed = |ink: Option<InkBox>, transform| {
    let (origin, size) = ink?.rect();

    bounds_for_placed_rect(origin, size, transform)
  };

  bounds = merge_bounds(bounds, placed(items.ink.content, inline_transform));
  bounds = merge_bounds(bounds, placed(items.ink.decorations, transform));

  outset_bounds(
    bounds,
    items.span_ink_reach.max(text_ink_reach(&font_style)),
    inline_transform,
  )
}

pub(crate) fn has_inline_paint_content(node: &RenderNode) -> bool {
  matches!(OwnContent::of(node), OwnContent::Inline(_))
}

/// [`SceneBounds::of_rect`] for a rect at `origin` in `transform`'s space.
fn bounds_for_placed_rect(
  origin: Point<f32>,
  size: Size<f32>,
  transform: Affine,
) -> Option<SceneBounds> {
  SceneBounds::of_rect(size, Affine::translation(origin.x, origin.y) * transform)
}

fn merge_bounds(left: Option<SceneBounds>, right: Option<SceneBounds>) -> Option<SceneBounds> {
  match (left, right) {
    // Empty bounds paint nothing, so they never expand the union.
    (Some(left), Some(right)) if left.is_empty() => Some(right),
    (Some(left), Some(right)) if right.is_empty() => Some(left),
    (Some(left), Some(right)) => Some(left.union(right)),
    (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
    (None, None) => None,
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::{Scene, SceneBounds, merge_bounds};
  use crate::{
    context::RenderContext,
    geometry::{Point, Size},
    layout::{node::Node, tree::RenderNode},
    resources::font::Fonts,
    style::{Affine, SizingContext, StyleSheet},
    viewport::Viewport,
  };

  #[test]
  fn a_marker_paints_at_its_host_offset() {
    let viewport = Viewport::new((200, 200));
    let stylesheet = StyleSheet::parse(
      ".list { padding: 10.5px; list-style-type: decimal } \
       .item { display: list-item; list-style-position: inside }",
    )
    .expect("stylesheet parses");
    let context = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(SizingContext::builder().viewport(viewport).build())
      .stylesheet(Arc::new(stylesheet))
      .build();
    let list = Node::container([Node::container([Node::text("item")]).with_class_name("item")])
      .with_class_name("list");

    let scene = Scene::lay_out(RenderNode::from_node(&context, list), viewport, false)
      .expect("scene lays out");
    let item = &scene.root.children.as_deref().expect("children")[0];
    let marker = item.marker().expect("marker");

    assert_eq!(marker.context.paint_offset(), Point { x: 10.5, y: 10.5 });
  }

  #[test]
  fn zero_sized_rect_produces_empty_bounds() {
    let bounds = SceneBounds::of_rect(
      Size {
        width: 0.0,
        height: 100.0,
      },
      Affine::translation(50.0, 50.0),
    );

    assert!(
      bounds.is_some_and(SceneBounds::is_empty),
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
