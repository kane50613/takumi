use std::{
  borrow::Cow, collections::HashMap, hash::Hasher, iter::Copied, mem::take, rc::Rc, slice,
  vec::IntoIter,
};

use parley::fontique::{Attributes, FontStyle as FontiqueStyle};
use taffy::{
  BlockContext, Cache, CacheTree, Display as TaffyDisplay, Layout, LayoutBlockContainer,
  LayoutFlexboxContainer, LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree,
  NodeId as TaffyNodeId, RequestedAxis, RoundTree, RunMode, Size as TaffySize, SizingMode, Style,
  TraversePartialTree, TraverseTree, compute_block_layout, compute_cached_layout,
  compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
  compute_root_layout,
};
use xxhash_rust::xxh3::Xxh3;

use crate::{
  Error,
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, ComputedLayout, NodeId, Rect, Size},
  layout::{
    inline::{
      InlineContentKind, InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineMeasureOptions,
      collect_inline_items, create_inline_constraint, create_inline_layout,
    },
    list_marker::{ListCounter, is_list_element, list_marker, owns_list_counter},
    node::{Node, NodeStyleLayers, TextData},
  },
  matching::{MatchedDeclarationsView, NodeMatchedDeclarations, match_stylesheets_view},
  style::{
    Affine, BackgroundImage, BackgroundImages, Color, ComputedStyle, ContentItem, ContentValue,
    Display, Float, Length, LineHeight, ListStylePosition, Position, SizingContext,
    Style as NodeStyle, StyleDeclaration, StyleDeclarationBlock, StyleSheet, TextWrapMode,
    TwBlocks, WhiteSpaceCollapse, apply_stylesheet_animations,
  },
  viewport::Viewport,
};

/// A render-tree child paired with its layout node id.
#[derive(Debug, Clone, Copy)]
pub struct OrderedChild {
  /// Index of the child in its parent's render order.
  pub render_index: usize,
  /// Layout node id.
  pub node_id: NodeId,
  /// Containing block the child was hoisted to, if out-of-flow.
  pub hoisted_cb: Option<NodeId>,
}

/// Each visited node's device transform and content box, kept so a hoisted
/// out-of-flow child resolves against its containing block instead of its
/// box-tree parent.
#[derive(Default)]
pub struct ContainingBlocks {
  transforms: HashMap<NodeId, Affine>,
  content_boxes: HashMap<NodeId, Size<Option<f32>>>,
}

impl ContainingBlocks {
  /// Records the device transform a node was placed with.
  pub fn record_transform(&mut self, node_id: NodeId, transform: Affine) {
    self.transforms.insert(node_id, transform);
  }

  /// Records the content box a node lays its children out in.
  pub fn record_content_box(&mut self, node_id: NodeId, content_box: Size<Option<f32>>) {
    self.content_boxes.insert(node_id, content_box);
  }

  /// The transform and container size `child` resolves against: its containing
  /// block's when hoisted, otherwise the parent's.
  pub fn base_for(
    &self,
    child: &OrderedChild,
    parent_transform: Affine,
    parent_content_box: Size<Option<f32>>,
  ) -> (Affine, Size<Option<f32>>) {
    match child.hoisted_cb {
      Some(cb) => (
        self
          .transforms
          .get(&cb)
          .copied()
          .unwrap_or(parent_transform),
        self
          .content_boxes
          .get(&cb)
          .copied()
          .unwrap_or(parent_content_box),
      ),
      None => (parent_transform, parent_content_box),
    }
  }
}

/// Immutable per-node layout output after computing a tree.
pub struct LayoutResults {
  nodes: Vec<LayoutResultNode>,
}

struct LayoutResultNode {
  layout: Layout,
  unsnapped: Layout,
  first_baseline_y: Option<f32>,
  box_children: Box<[OrderedChild]>,
}

impl LayoutResults {
  /// Lays out the tree under `root` in `available_space`.
  pub(super) fn compute(root: &RenderNode, available_space: Size<AvailableSpace>) -> Self {
    let mut tree = LayoutTree::from_render_node(root);

    tree.compute_layout(available_space);
    tree.into_results()
  }

  /// Computed layout of a node.
  pub fn layout(&self, node_id: NodeId) -> crate::Result<ComputedLayout> {
    self
      .node(node_id)
      .map(|node| ComputedLayout::from_taffy(&node.layout, &node.unsnapped))
  }

  /// Paint-ordered children of a node.
  pub fn box_children(&self, node_id: NodeId) -> crate::Result<&[OrderedChild]> {
    self.node(node_id).map(|node| node.box_children.as_ref())
  }

  pub(crate) fn first_baseline_y(&self, node_id: NodeId) -> crate::Result<Option<f32>> {
    self.node(node_id).map(|node| node.first_baseline_y)
  }

  /// The root's border-box size, zero when the tree is empty.
  pub(super) fn root_size(&self) -> Size<f32> {
    self
      .layout(NodeId::ROOT)
      .map_or(Size::ZERO, |layout| layout.size)
  }

  fn node(&self, node_id: NodeId) -> crate::Result<&LayoutResultNode> {
    self
      .nodes
      .get(usize::from(node_id))
      .ok_or(Error::InvalidLayoutNode(node_id.into()))
  }
}

/// Mutable taffy tree wrapping render nodes during layout.
pub struct LayoutTree<'r> {
  nodes: Vec<LayoutNodeState>,
  render_nodes: Vec<&'r RenderNode>,
}

struct LayoutNodeState {
  style: Style,
  /// Whether the style is the same whatever query container it resolves
  /// against, so taffy's repeated passes can reuse it.
  container_independent: bool,
  cache: Cache,
  unrounded_layout: Layout,
  final_layout: Layout,
  first_baseline_y: Option<f32>,
  is_inline_children: bool,
  children: Box<[TaffyNodeId]>,
  box_children: Box<[OrderedChild]>,
}

/// Who created a box: the author's node tree, or a layout-generated construct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeOrigin {
  /// An authored node, at its document-order position in the source tree.
  Authored {
    /// Position in the source tree, counted in document order.
    source_order: usize,
  },
  /// A generated `::marker` box.
  Marker,
  /// A generated `::before`/`::after` box.
  Pseudo,
  /// An anonymous box layout invented, such as an inline-text wrapper.
  Anonymous,
}

/// A box's source-table role before lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TablePart {
  /// The lowered `display: table` box itself.
  Table,
  /// A `table-caption` box.
  Caption,
  /// A cell from a `table-header-group` row.
  HeaderCell,
  /// A cell from a body row.
  BodyCell,
  /// A cell from a `table-footer-group` row.
  FooterCell,
}

/// A styled node plus its children, ready for layout.
#[derive(Clone)]
pub struct RenderNode {
  /// Resolved style and rendering context.
  pub context: RenderContext,
  /// Source node, absent for anonymous wrappers.
  pub node: Option<Node>,
  /// Who created this box.
  pub origin: NodeOrigin,
  /// Child render nodes.
  pub children: Option<Box<[RenderNode]>>,
  pub(crate) layout_style_override: Option<Box<Style>>,
  /// Text for an anonymous inline-text wrapper.
  pub anonymous_text_content: Option<String>,
  /// Generated marker box, emitted before this box's own inline content.
  pub(crate) marker: Option<Box<RenderNode>>,
  pub(crate) force_inline_layout: bool,
  /// Grid lines a lowered table's header rows cover, as `[start, end)`, for
  /// paged output to repeat per css-tables-3 §repeated-headers.
  pub table_header_lines: Option<(i16, i16)>,
  /// The role this box had in a source table, kept through table lowering.
  pub table_part: Option<TablePart>,
}

/// Drops the render tree iteratively; recursive drop glue overflows the stack
/// on deep user trees, same reason `Node` has an iterative `Drop`.
impl Drop for RenderNode {
  fn drop(&mut self) {
    let mut stack: Vec<RenderNode> = Vec::new();
    let collect = |node: &mut RenderNode, stack: &mut Vec<RenderNode>| {
      if let Some(children) = node.children.take() {
        stack.extend(children.into_vec());
      }
      if let Some(marker) = node.marker.take() {
        stack.push(*marker);
      }
    };

    collect(self, &mut stack);
    while let Some(mut node) = stack.pop() {
      collect(&mut node, &mut stack);
    }
  }
}

/// The layout an anonymous block box takes in place of a computed style.
fn block_style() -> Style {
  Style {
    display: TaffyDisplay::Block,
    ..Style::default()
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AtomicInlineMetrics {
  pub(crate) size: Size<f32>,
  pub(crate) baseline_offset: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InlineBaselineSource {
  InlineContentLastLine,
  InlineContentFirstLine,
  LayoutFirstBaseline,
}

/// An element's own important declarations by cascade tier.
struct ElementImportant {
  tw: Option<Rc<TwBlocks>>,
  inline: Option<StyleDeclarationBlock>,
}

fn registered_custom_property_parent_style<'a>(
  parent_style: &'a ComputedStyle,
  stylesheets: &[StyleSheet],
  viewport: Viewport,
) -> Cow<'a, ComputedStyle> {
  if stylesheets
    .iter()
    .all(|sheet| sheet.property_rules().is_empty())
  {
    return Cow::Borrowed(parent_style);
  }

  let mut adjusted_parent = parent_style.clone();

  for sheet in stylesheets {
    for property_rule in sheet.property_rules() {
      if !property_rule
        .media_queries
        .iter()
        .all(|media_query| media_query.matches(viewport))
      {
        continue;
      }

      adjusted_parent
        .custom_properties
        .register_in_scope(property_rule, &parent_style.custom_properties);
    }
  }

  Cow::Owned(adjusted_parent)
}

impl<'r> LayoutTree<'r> {
  /// Builds a layout tree from a render-node root.
  pub fn from_render_node(render_root: &'r RenderNode) -> Self {
    let mut tree = Self {
      nodes: Vec::with_capacity(1),
      render_nodes: Vec::with_capacity(1),
    };
    let root_id = tree.push_subtree(render_root);

    debug_assert_eq!(root_id, TaffyNodeId::from(0usize));

    tree
  }

  /// Appends the layout nodes of the subtree under `render_root`, returning its root id.
  fn push_subtree(&mut self, render_root: &'r RenderNode) -> TaffyNodeId {
    struct PendingNode<'r> {
      node_id: TaffyNodeId,
      position: Position,
      contains_fixed: bool,
      next_child_index: usize,
      children: Option<&'r [RenderNode]>,
      taffy_child_ids: Vec<TaffyNodeId>,
      box_children: Vec<OrderedChild>,
    }

    fn push_node_state<'r>(
      nodes: &mut Vec<LayoutNodeState>,
      render_nodes: &mut Vec<&'r RenderNode>,
      render_node: &'r RenderNode,
    ) -> PendingNode<'r> {
      let node_index = nodes.len();
      let node_id = TaffyNodeId::from(node_index);
      let is_inline_children = render_node.should_create_inline_layout();
      let children = if is_inline_children {
        None
      } else {
        render_node.children.as_deref()
      };
      let position = render_node.context.style.position;
      let contains_fixed = render_node.context.style.contains_fixed_descendants();

      render_nodes.push(render_node);

      let (style, container_independent) = {
        let sizing = &render_node.context.sizing;

        // Resolution reports whether it read the query container, which is
        // exact. Comparing two resolved sizes is not: `min(10px, 100cqw)`
        // agrees across two large containers and disagrees with a small one.
        sizing.container_read.set(false);
        let style = render_node.layout_style(sizing);
        let independent = !sizing.container_read.get();

        (style, independent)
      };

      nodes.push(LayoutNodeState {
        style,
        container_independent,
        cache: Cache::new(),
        unrounded_layout: Layout::new(),
        final_layout: Layout::new(),
        first_baseline_y: None,
        is_inline_children,
        children: Box::new([]),
        box_children: Box::new([]),
      });

      let capacity = children.map_or(0, <[RenderNode]>::len);
      PendingNode {
        node_id,
        position,
        contains_fixed,
        next_child_index: 0,
        children,
        taffy_child_ids: Vec::with_capacity(capacity),
        box_children: Vec::with_capacity(capacity),
      }
    }

    // Out-of-flow nodes are re-parented (hoisted) in the taffy tree so taffy's
    // direct-parent positioning resolves against the correct CSS containing
    // block: the nearest ancestor that establishes one. The box (render) tree is
    // preserved separately for painting.
    let Self {
      nodes,
      render_nodes,
    } = self;
    let mut cb_stack: Vec<TaffyNodeId> = Vec::new();
    let mut fixed_cb_stack: Vec<TaffyNodeId> = Vec::new();
    let mut hoisted: HashMap<TaffyNodeId, Vec<TaffyNodeId>> = HashMap::new();

    let root = push_node_state(nodes, render_nodes, render_root);
    let root_id = root.node_id;
    cb_stack.push(root_id);
    fixed_cb_stack.push(root_id);
    let mut stack = vec![root];

    while let Some(current) = stack.last_mut() {
      if let Some(children) = current.children
        && let Some(child) = children.get(current.next_child_index)
      {
        current.next_child_index += 1;
        let pending = push_node_state(nodes, render_nodes, child);
        if pending.position.is_positioned() || pending.contains_fixed {
          cb_stack.push(pending.node_id);
        }
        if pending.contains_fixed {
          fixed_cb_stack.push(pending.node_id);
        }
        stack.push(pending);
        continue;
      }

      let Some(finished) = stack.pop() else {
        break;
      };
      let fid = finished.node_id;

      let mut taffy_children = finished.taffy_child_ids;
      if let Some(extra) = hoisted.remove(&fid) {
        taffy_children.extend(extra);
      }
      let idx: usize = fid.into();
      if matches!(
        nodes[idx].style.display,
        TaffyDisplay::Flex | TaffyDisplay::Grid
      ) {
        sort_children_by_order(&mut taffy_children, |child_id| {
          let child_idx: usize = child_id.into();
          render_nodes
            .get(child_idx)
            .map_or(0, |child| child.context.style.order.0)
        });
      }
      nodes[idx].children = taffy_children.into_boxed_slice();
      nodes[idx].box_children = finished.box_children.into_boxed_slice();

      if finished.position.is_positioned() || finished.contains_fixed {
        cb_stack.pop();
      }
      if finished.contains_fixed {
        fixed_cb_stack.pop();
      }

      if let Some(parent) = stack.last_mut() {
        let render_index = parent.next_child_index - 1;
        let cb = match finished.position {
          Position::Absolute => Some(*cb_stack.last().unwrap_or(&root_id)),
          Position::Fixed => Some(*fixed_cb_stack.last().unwrap_or(&root_id)),
          _ => None,
        };
        // Only re-parent when the containing block differs from the structural
        // parent; otherwise keep the node in place to preserve DOM order (and the
        // in-flow static position for auto-inset out-of-flow boxes).
        let hoisted_cb = match cb {
          Some(cb) if cb != parent.node_id => {
            hoisted.entry(cb).or_default().push(fid);
            Some(cb)
          }
          _ => {
            parent.taffy_child_ids.push(fid);
            None
          }
        };
        parent.box_children.push(OrderedChild {
          render_index,
          node_id: NodeId::from_taffy(fid),
          hoisted_cb: hoisted_cb.map(NodeId::from_taffy),
        });
      }
    }

    root_id
  }

  /// Computes and rounds the layout for the whole tree.
  pub fn compute_layout(&mut self, available_space: Size<AvailableSpace>) {
    let root_node_id = NodeId::ROOT.into_taffy();
    compute_root_layout(
      self,
      root_node_id,
      available_space.map(AvailableSpace::into_taffy).into_taffy(),
    );
    self.snap_layout(root_node_id, 0.0, 0.0);
  }

  /// Snaps every box to whole pixels, both edges in absolute space so a box
  /// always meets the one beside it. taffy's `round_layout` documents the same
  /// rule but rounds a location against its parent, which parts two siblings by a
  /// pixel whenever their parent sits on a fraction.
  /// Blink snaps the same way, against the absolute offset's fraction
  /// (`SnapSizeToPixel`, platform/geometry/layout_unit.h).
  fn snap_layout(&mut self, node_id: TaffyNodeId, parent_x: f32, parent_y: f32) {
    let unrounded = self.get_unrounded_layout(node_id);
    let mut layout = unrounded;
    let x = parent_x + unrounded.location.x;
    let y = parent_y + unrounded.location.y;

    layout.location.x = x.round() - parent_x.round();
    layout.location.y = y.round() - parent_y.round();
    layout.size.width = (x + unrounded.size.width).round() - x.round();
    layout.size.height = (y + unrounded.size.height).round() - y.round();
    layout.padding.left = (x + unrounded.padding.left).round() - x.round();
    layout.padding.right = (x + unrounded.size.width).round()
      - (x + unrounded.size.width - unrounded.padding.right).round();
    layout.padding.top = (y + unrounded.padding.top).round() - y.round();
    layout.padding.bottom = (y + unrounded.size.height).round()
      - (y + unrounded.size.height - unrounded.padding.bottom).round();

    self.set_final_layout(node_id, &layout);

    for index in 0..self.child_count(node_id) {
      let child = self.get_child_id(node_id, index);

      self.snap_layout(child, x, y);
    }
  }

  /// Consumes the tree into immutable per-node layout results.
  pub fn into_results(self) -> LayoutResults {
    LayoutResults {
      nodes: self
        .nodes
        .into_iter()
        .map(|node| LayoutResultNode {
          layout: node.final_layout,
          unsnapped: node.unrounded_layout,
          first_baseline_y: node.first_baseline_y,
          box_children: node.box_children,
        })
        .collect(),
    }
  }

  fn get_layout_node_ref(&self, node_id: TaffyNodeId) -> Option<&LayoutNodeState> {
    self.nodes.get(usize::from(node_id))
  }

  fn get_layout_node_mut_ref(&mut self, node_id: TaffyNodeId) -> Option<&mut LayoutNodeState> {
    self.nodes.get_mut(usize::from(node_id))
  }

  fn update_node_style_for_available_space(
    &mut self,
    node_id: TaffyNodeId,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) {
    let idx = usize::from(node_id);
    let (Some(node), Some(render_node)) = (self.nodes.get_mut(idx), self.render_nodes.get(idx))
    else {
      return;
    };

    if node.container_independent {
      return;
    }

    let mut sizing = render_node.context.sizing.clone();

    sizing.container_size = Size {
      width: known_dimensions
        .width
        .or(available_space.width.into_option()),
      height: known_dimensions
        .height
        .or(available_space.height.into_option()),
    };
    node.style = render_node.layout_style(&sizing);
  }
}

impl RenderNode {
  // Taffy may inject a flex stretch-derived cross-size into leaf `known_dimensions`
  // during intrinsic single-axis sizing (`ComputeSize` with `InherentSize` or `ContentSize`). For replaced
  // elements, letting that value participate in aspect-ratio transfer can
  // incorrectly inflate the measured main-size. Strip that hint at the leaf boundary.
  fn should_strip_flex_intrinsic_stretch_known_dimension(
    &self,
    inputs: LayoutInput,
    known_dimensions: Size<Option<f32>>,
  ) -> bool {
    if inputs.run_mode != RunMode::ComputeSize
      || !matches!(
        inputs.sizing_mode,
        SizingMode::InherentSize | SizingMode::ContentSize
      )
    {
      return false;
    }

    if !matches!(
      inputs.axis,
      RequestedAxis::Horizontal | RequestedAxis::Vertical
    ) {
      return false;
    }

    let Some(node) = self.node.as_ref() else {
      return false;
    };

    if !node.is_replaced_element() {
      return false;
    }

    match inputs.axis {
      RequestedAxis::Horizontal => {
        known_dimensions.width.is_none() && known_dimensions.height.is_some()
      }
      RequestedAxis::Vertical => {
        known_dimensions.height.is_none() && known_dimensions.width.is_some()
      }
      RequestedAxis::Both => false,
    }
  }
}

/// Stable-sorts flex/grid children by `order`, keeping source order for ties.
fn sort_children_by_order(
  children: &mut [TaffyNodeId],
  mut child_order: impl FnMut(TaffyNodeId) -> i32,
) {
  if children.iter().all(|&child_id| child_order(child_id) == 0) {
    return;
  }

  children.sort_by_key(|&child_id| child_order(child_id));
}

impl TraversePartialTree for LayoutTree<'_> {
  type ChildIter<'a>
    = Copied<slice::Iter<'a, TaffyNodeId>>
  where
    Self: 'a;

  fn child_ids(&self, parent_node_id: TaffyNodeId) -> Self::ChildIter<'_> {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return [].iter().copied();
    };

    node.children.iter().copied()
  }

  fn child_count(&self, parent_node_id: TaffyNodeId) -> usize {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return 0;
    };

    node.children.len()
  }

  fn get_child_id(&self, parent_node_id: TaffyNodeId, child_index: usize) -> TaffyNodeId {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return TaffyNodeId::from(0usize);
    };

    node.children[child_index]
  }
}

impl TraverseTree for LayoutTree<'_> {}

impl LayoutPartialTree for LayoutTree<'_> {
  type CoreContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type CustomIdent = String;

  fn get_core_container_style(&self, node_id: TaffyNodeId) -> Self::CoreContainerStyle<'_> {
    if let Some(node) = self.get_layout_node_ref(node_id) {
      return &node.style;
    }
    &self.nodes[0].style
  }

  fn set_unrounded_layout(&mut self, node_id: TaffyNodeId, layout: &Layout) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.unrounded_layout = *layout;
  }

  fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
    let Some(root) = self.render_nodes.first() else {
      return 0.0;
    };

    root.context.sizing.resolve_calc(val, basis)
  }

  fn compute_child_layout(&mut self, node: TaffyNodeId, inputs: LayoutInput) -> LayoutOutput {
    self.compute_child_layout_inner(node, inputs, None)
  }
}

impl<'r> LayoutTree<'r> {
  fn compute_child_layout_inner(
    &mut self,
    node: TaffyNodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    self.update_node_style_for_available_space(
      node,
      Size::from_taffy(inputs.available_space).map(AvailableSpace::from_taffy),
      Size::from_taffy(inputs.known_dimensions),
    );

    if inputs.run_mode == RunMode::PerformHiddenLayout {
      return compute_hidden_layout(self, node);
    }

    let output = compute_cached_layout(self, node, inputs, |tree, node, inputs| {
      let Some(node_data) = tree.get_layout_node_ref(node) else {
        return compute_hidden_layout(tree, node);
      };

      let display_mode = node_data.style.display;
      let has_children = !node_data.children.is_empty();

      match (display_mode, has_children) {
        (TaffyDisplay::None, _) => compute_hidden_layout(tree, node),
        (TaffyDisplay::Block, true) => compute_block_layout(tree, node, inputs, block_ctx),
        // A flow-root box is a block formatting context root, so it never joins
        // its parent's context. <https://drafts.csswg.org/css-display-3/#valdef-display-flow-root>
        (TaffyDisplay::FlowRoot, true) => compute_block_layout(tree, node, inputs, None),
        (TaffyDisplay::Flex, true) => compute_flexbox_layout(tree, node, inputs),
        (TaffyDisplay::Grid, true) => compute_grid_layout(tree, node, inputs),
        (_, false) => {
          let idx: usize = node.into();
          let Some(render_node) = tree.render_nodes.get(idx) else {
            return compute_hidden_layout(tree, node);
          };

          let stripped_known_dimensions = |known_dimensions: TaffySize<Option<f32>>| {
            if render_node.should_strip_flex_intrinsic_stretch_known_dimension(
              inputs,
              Size::from_taffy(known_dimensions),
            ) {
              TaffySize::NONE
            } else {
              known_dimensions
            }
          };

          let mut output = compute_leaf_layout(
            inputs,
            &node_data.style,
            |val, basis| tree.resolve_calc_value(val, basis),
            |known_dimensions, available_space| {
              let known_dimensions = stripped_known_dimensions(known_dimensions);

              if let TaffySize {
                width: Some(width),
                height: Some(height),
              } = known_dimensions.maybe_apply_aspect_ratio(node_data.style.aspect_ratio)
              {
                return TaffySize { width, height };
              }

              render_node
                .measure(
                  Size::from_taffy(available_space).map(AvailableSpace::from_taffy),
                  Size::from_taffy(known_dimensions),
                  &node_data.style,
                  node_data.is_inline_children,
                )
                .into_taffy()
            },
          );

          let lays_out_text = node_data.is_inline_children
            || matches!(
              render_node.node.as_ref().and_then(Node::inline_content),
              Some(InlineContentKind::Text(_))
            );

          // `compute_leaf_layout` reports no baseline, which leaves flexbox
          // baseline alignment on its bottom margin edge fallback.
          if lays_out_text && inputs.run_mode == RunMode::PerformLayout {
            output.baselines.first = render_node.inline_content_border_box_baseline(
              Size::from_taffy(inputs.available_space).map(AvailableSpace::from_taffy),
              Size::from_taffy(output.size),
              false,
            );
          }

          output
        }
      }
    });

    if let Some(node_data) = self.get_layout_node_mut_ref(node) {
      node_data.first_baseline_y = output.baselines.first;
    }

    output
  }
}

impl CacheTree for LayoutTree<'_> {
  fn cache_get(&mut self, node_id: TaffyNodeId, input: &LayoutInput) -> Option<LayoutOutput> {
    let node = self.get_layout_node_mut_ref(node_id)?;
    node.cache.get(input)
  }

  fn cache_store(
    &mut self,
    node_id: TaffyNodeId,
    input: &LayoutInput,
    layout_output: LayoutOutput,
  ) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.cache.store(input, layout_output);
  }

  fn cache_clear(&mut self, node_id: TaffyNodeId) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.cache.clear();
  }
}

impl LayoutBlockContainer for LayoutTree<'_> {
  type BlockContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type BlockItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_block_container_style(&self, node_id: TaffyNodeId) -> Self::BlockContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_block_child_style(&self, child_node_id: TaffyNodeId) -> Self::BlockItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }

  fn compute_block_child_layout(
    &mut self,
    node: TaffyNodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    self.compute_child_layout_inner(node, inputs, block_ctx)
  }
}

impl LayoutFlexboxContainer for LayoutTree<'_> {
  type FlexboxContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type FlexboxItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_flexbox_container_style(&self, node_id: TaffyNodeId) -> Self::FlexboxContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_flexbox_child_style(&self, child_node_id: TaffyNodeId) -> Self::FlexboxItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }
}

impl LayoutGridContainer for LayoutTree<'_> {
  type GridContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type GridItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_grid_container_style(&self, node_id: TaffyNodeId) -> Self::GridContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_grid_child_style(&self, child_node_id: TaffyNodeId) -> Self::GridItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }
}

impl RoundTree for LayoutTree<'_> {
  fn get_unrounded_layout(&self, node_id: TaffyNodeId) -> Layout {
    let Some(node) = self.get_layout_node_ref(node_id) else {
      return Layout::new();
    };

    node.unrounded_layout
  }

  fn set_final_layout(&mut self, node_id: TaffyNodeId, layout: &Layout) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    let mut final_layout = *layout;
    if node.is_inline_children {
      final_layout.size.width = node.unrounded_layout.size.width;
    }
    // Snap the box, not the stroke: a rounded border width comes out as 2px on
    // one edge and 3px on another for a uniform 2.5px border, while the
    // fractional width paints evenly through coverage AA, as browsers do.
    final_layout.border = node.unrounded_layout.border;
    node.final_layout = final_layout;
  }
}

impl RenderNode {
  /// The taffy style this node lays out with: its own override when it has one, otherwise
  /// its computed style.
  fn layout_style(&self, sizing: &SizingContext) -> taffy::Style {
    match self.layout_style_override.as_deref() {
      Some(style) => style.clone(),
      None => self.context.style.to_taffy_style(sizing),
    }
  }

  /// A box with no layout override, marker, or table role.
  pub(super) fn new(
    context: RenderContext,
    origin: NodeOrigin,
    node: Option<Node>,
    children: Option<Box<[RenderNode]>>,
  ) -> Self {
    Self {
      context,
      node,
      origin,
      children,
      layout_style_override: None,
      anonymous_text_content: None,
      marker: None,
      force_inline_layout: false,
      table_header_lines: None,
      table_part: None,
    }
  }

  /// An anonymous box laid out with `layout_style` instead of its computed style.
  fn anonymous(
    context: RenderContext,
    node: Option<Node>,
    children: Option<Box<[RenderNode]>>,
    layout_style: Style,
  ) -> Self {
    let mut anonymous = Self::new(context, NodeOrigin::Anonymous, node, children);

    anonymous.layout_style_override = Some(Box::new(layout_style));
    anonymous
  }

  pub(super) fn anonymous_text_item(parent_context: &RenderContext, text: String) -> Self {
    Self::text_item(RenderContext::for_anonymous(parent_context), text)
  }

  fn text_item(context: RenderContext, text: String) -> Self {
    let mut item = Self::anonymous(context, None, None, block_style());

    item.anonymous_text_content = Some(text);
    item.force_inline_layout = true;
    item
  }

  pub(super) fn anonymous_block_container(
    parent_context: &RenderContext,
    children: Vec<RenderNode>,
  ) -> Self {
    Self::anonymous(
      RenderContext::for_anonymous(parent_context),
      None,
      Some(children.into_boxed_slice()),
      block_style(),
    )
  }

  pub(super) fn anonymous_image_item(
    parent_context: &RenderContext,
    image: BackgroundImage,
  ) -> Self {
    // Cap image content to the parent pseudo's box so explicit `width` / `height`
    // on the pseudo wins over intrinsic / default sizing.
    let max_size = TaffySize {
      width: taffy::LengthPercentageAuto::percent(1.0),
      height: taffy::LengthPercentageAuto::percent(1.0),
    };

    match image {
      BackgroundImage::Url(url) => Self::anonymous(
        RenderContext::for_anonymous(parent_context),
        Some(Node::image(url)),
        None,
        Style {
          max_size,
          ..Style::default()
        },
      ),
      gradient => {
        let mut context = RenderContext::for_anonymous(parent_context);

        context.style.background_image = Some(BackgroundImages::from([gradient]));
        Self::anonymous(
          context,
          Some(Node::container([])),
          None,
          // css-images-3 §5.1 default object size when the parent is auto.
          Style {
            size: TaffySize {
              width: taffy::Dimension::length(300.0),
              height: taffy::Dimension::length(150.0),
            },
            max_size,
            ..Style::default()
          },
        )
      }
    }
  }

  /// An element's own text, moved into a child so generated content can precede it.
  fn generated_sibling_text(parent_context: &RenderContext, text: String) -> Self {
    let (mut style, sizing, current_color) =
      parent_context.resolve_pseudo_style(&MatchedDeclarationsView::default());
    let parent_style = &parent_context.style;

    style
      .text_decoration_line
      .clone_from(&parent_style.text_decoration_line);
    style.text_decoration_style = parent_style.text_decoration_style;
    style
      .text_decoration_color
      .clone_from(&parent_style.text_decoration_color);
    style.text_decoration_thickness = parent_style.text_decoration_thickness;

    let context = RenderContext::from_parent(parent_context, style, sizing, current_color);

    Self::text_item(context, text)
  }

  fn pseudo_content_child(
    originating_node: &Node,
    pseudo_context: &RenderContext,
    item: ContentItem,
  ) -> Option<Self> {
    let text = match item {
      ContentItem::Text(text) => text.as_ref().to_owned(),
      ContentItem::Attr(attr) => originating_node
        .attribute(&attr.name)
        .map(str::to_owned)
        .unwrap_or_else(|| attr.fallback.as_ref().to_owned()),
      ContentItem::Image(image) => {
        return Some(Self::anonymous_image_item(pseudo_context, *image));
      }
    };

    (!text.is_empty()).then(|| Self::anonymous_text_item(pseudo_context, text))
  }

  fn from_pseudo_match(
    parent_context: &RenderContext,
    originating_node: &Node,
    pseudo_matched: &MatchedDeclarationsView<'_>,
  ) -> Option<Self> {
    let (mut style, sizing, current_color) = parent_context.resolve_pseudo_style(pseudo_matched);

    if matches!(style.display, Display::None) {
      return None;
    }

    // flex/grid add no semantics over a flat content list; downgrade per spec §8.
    if matches!(
      style.display,
      Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid
    ) {
      style.display = Display::Block;
    }

    let items = match take(&mut style.content) {
      ContentValue::Items(items) => items,
      _ => return None,
    };

    let pseudo_context = RenderContext::from_parent(parent_context, style, sizing, current_color);

    let children: Box<[Self]> = items
      .into_vec()
      .into_iter()
      .filter_map(|item| Self::pseudo_content_child(originating_node, &pseudo_context, item))
      .collect();

    if children.is_empty() {
      return None;
    }

    Some(Self::new(
      pseudo_context,
      NodeOrigin::Pseudo,
      Some(Node::container([])),
      Some(children),
    ))
  }

  /// Blink positions an outside marker against the item's first line box, so the
  /// marker goes on the box that establishes that line, however deep it sits. An
  /// inside marker is the item's own content and stays on the item.
  fn attach_marker(&mut self, marker: RenderNode) {
    if self.should_create_inline_layout() {
      self.marker = Some(Box::new(marker));
      return;
    }

    if marker.context.style.list_style_position == ListStylePosition::Outside
      && let Some(block) = self.marker_host_child()
    {
      block.attach_marker(marker);
      return;
    }

    let has_block_content = self.children.as_deref().is_some_and(|children| {
      children
        .iter()
        .any(|child| !child.participates_in_inline_formatting_context())
    });

    if !has_block_content {
      // Text of its own, or nothing at all: the marker shares that line.
      self.force_inline_layout = true;
      self.marker = Some(Box::new(marker));
      return;
    }

    // Block-level content the marker may not join, so it gets a line of its own.
    let mut line = RenderNode::anonymous_block_container(&self.context, Vec::new());
    line.force_inline_layout = true;
    line.marker = Some(Box::new(marker));

    let mut children = Vec::from(self.children.take().unwrap_or_default());
    children.insert(0, line);
    self.children = Some(children.into_boxed_slice());
  }

  /// The block box the marker travels into when this box has no line of its own.
  fn marker_host_child(&mut self) -> Option<&mut RenderNode> {
    let child = self
      .children
      .as_deref_mut()?
      .iter_mut()
      .find(|child| !child.is_out_of_flow())?;

    child.hosts_marker_line().then_some(child)
  }

  fn hosts_marker_line(&self) -> bool {
    self.context.style.display == Display::Block
      && self.context.style.float == Float::None
      && self.leads_to_a_line()
  }

  /// Whether this box, or the block chain below it, ends in a line the marker can share.
  fn leads_to_a_line(&self) -> bool {
    self.should_create_inline_layout()
      || self.children.is_none()
      || self
        .children
        .as_deref()
        .and_then(|children| children.iter().find(|child| !child.is_out_of_flow()))
        .is_some_and(RenderNode::hosts_marker_line)
  }

  fn is_anonymous_text_item(&self) -> bool {
    self.anonymous_text_content.is_some() && self.node.is_none()
  }

  fn is_whitespace_only_text_node(&self) -> bool {
    self
      .node
      .as_ref()
      .is_some_and(Node::is_whitespace_only_text)
  }

  // Only fully-collapsible whitespace may be dropped. preserve / preserve-spaces
  // keep their spaces, and preserve-breaks may hold a forced break, so a
  // whitespace-only node in any of those still renders.
  fn is_collapsible_whitespace_only_text_node(&self) -> bool {
    self.context.style.white_space_collapse == WhiteSpaceCollapse::Collapse
      && self.is_whitespace_only_text_node()
  }

  /// True if any direct child is an anonymous text item.
  pub fn has_anonymous_text_item_child(&self) -> bool {
    self
      .children
      .as_ref()
      .is_some_and(|children| children.iter().any(RenderNode::is_anonymous_text_item))
  }

  /// The authored node's document-order position, absent for generated boxes.
  pub fn source_order(&self) -> Option<usize> {
    match self.origin {
      NodeOrigin::Authored { source_order } => Some(source_order),
      _ => None,
    }
  }

  /// Resolves the descendant at `path` (child indices from this node).
  pub fn node_at_path(&self, path: &[usize]) -> Option<&RenderNode> {
    let mut current = self;
    for &index in path {
      current = current.children.as_deref()?.get(index)?;
    }
    Some(current)
  }

  /// Mutable [`node_at_path`](Self::node_at_path).
  pub fn node_at_path_mut(&mut self, path: &[usize]) -> Option<&mut RenderNode> {
    let mut current = self;
    for &index in path {
      current = current.children.as_deref_mut()?.get_mut(index)?;
    }
    Some(current)
  }

  pub(crate) fn is_inline_level(&self) -> bool {
    self.context.style.display.is_inline_level()
  }

  pub(crate) fn is_inline_atomic_container(&self) -> bool {
    matches!(
      self.context.style.display,
      Display::InlineBlock | Display::InlineFlex | Display::InlineGrid
    )
  }

  /// True if this node is laid out as an inline-level box (atomic inline or float).
  pub(crate) fn participates_as_inline_box(&self) -> bool {
    self.is_inline_atomic_container() || self.context.style.float != Float::None
  }

  fn participates_in_inflow_inline_formatting_context(&self) -> bool {
    self.is_inline_level()
      || self.is_inline_atomic_container()
      || self.anonymous_text_content.is_some()
  }

  fn participates_in_inline_formatting_context(&self) -> bool {
    self.participates_in_inflow_inline_formatting_context()
      || self.is_out_of_flow()
      || self.context.style.float != Float::None
  }

  fn is_out_of_flow(&self) -> bool {
    self.context.style.position.is_out_of_flow()
  }

  /// True if this node's children form an inline formatting context.
  pub fn should_create_inline_layout(&self) -> bool {
    self.force_inline_layout
      || (matches!(
        self.context.style.display,
        Display::Block
          | Display::FlowRoot
          | Display::InlineBlock
          | Display::ListItem
          | Display::TableCell
      ) && self.children.as_ref().is_some_and(|children| {
        children
          .iter()
          .any(RenderNode::participates_in_inflow_inline_formatting_context)
          && children
            .iter()
            .all(RenderNode::participates_in_inline_formatting_context)
      }))
  }

  /// Builds a render tree from a node under the given parent context.
  pub fn from_node(parent_context: &RenderContext, node: Node) -> Self {
    let matched_styles = match_stylesheets_view(
      &node,
      parent_context.stylesheet(),
      parent_context.sizing.viewport,
    );
    let mut tree = Self::from_node_iterative(parent_context, node, &matched_styles);

    tree.lower_tables();

    if tree.is_inline_level() {
      tree.context.style.display.blockify();
    }

    tree
  }

  fn from_node_iterative(
    parent_context: &RenderContext,
    root: Node,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
  ) -> Self {
    let mut source_cursor = 0;
    let mut root_counter = ListCounter::new(&root);
    let mut stack = vec![PendingRenderNode::build(
      parent_context,
      root,
      matched_declarations,
      &mut source_cursor,
      &mut root_counter,
      false,
    )];

    loop {
      let Some(current) = stack.last_mut() else {
        return RenderNode::empty(parent_context);
      };

      if let Some(child) = current.pending_children.next() {
        let child_pending = PendingRenderNode::build(
          &current.context,
          child,
          matched_declarations,
          &mut source_cursor,
          &mut current.list_counter,
          current.inside_list,
        );
        stack.push(child_pending);
        continue;
      }

      let Some(finished) = stack.pop() else {
        return RenderNode::empty(parent_context);
      };

      if !finished.owns_list_counter
        && let Some(parent) = stack.last_mut()
      {
        parent.list_counter = finished.list_counter;
      }

      let render_node = finished.finish();

      if let Some(parent) = stack.last_mut() {
        parent.rendered_children.push(render_node);
      } else {
        return render_node;
      }
    }
  }

  fn empty(parent_context: &RenderContext) -> Self {
    Self::new(
      parent_context.clone(),
      NodeOrigin::Anonymous,
      Some(Node::container([])),
      None,
    )
  }

  /// Blockifies, trims, and wraps `children` the way their parent's display
  /// requires, blockifying the parent itself when it holds a block.
  fn normalize_children(
    context: &mut RenderContext,
    children: Box<[RenderNode]>,
  ) -> Box<[RenderNode]> {
    if context.style.display.should_blockify_children() {
      // CSS Flexbox L1 §4 / Grid L1 §6: collapsible whitespace-only text
      // between items is not rendered; every remaining child blockifies.
      let mut children = Vec::from(children);
      children.retain(|child| !child.is_collapsible_whitespace_only_text_node());
      for child in &mut children {
        child.context.style.display.blockify();
      }

      return children.into_boxed_slice();
    }

    // Blink's Text::TextLayoutObjectIsNeeded: collapsible
    // whitespace-only text renders only after an in-flow inline-level
    // sibling, and leading whitespace only inside an inline parent
    // (#711, #992).
    let children =
      drop_collapsible_boundary_whitespace(Vec::from(children), context.style.display.is_inline())
        .into_boxed_slice();

    // https://github.com/kane50613/takumi/issues/738: out-of-flow boxes
    // must not be swept into an anonymous block box.
    let has_inline = children
      .iter()
      .any(|child| child.participates_in_inline_formatting_context() && !child.is_out_of_flow());
    let has_block = children
      .iter()
      .any(|child| !child.participates_in_inline_formatting_context());
    let has_out_of_flow = children.iter().any(RenderNode::is_out_of_flow);
    let parent_is_inline = context.style.display.is_inline();

    if parent_is_inline && has_block {
      context.style.display = context.style.display.as_blockified();
    }

    // A block parent mixing inline content with out-of-flow children wraps
    // the inline part so the absolute boxes stay as block-level children.
    // An inline parent keeps its inline formatting context untouched — an
    // anonymous block there would be dropped by the surrounding line box.
    if !(has_inline && (has_block || (!parent_is_inline && has_out_of_flow))) {
      return children;
    }

    let mut final_children = Vec::new();
    let mut inline_group = Vec::new();

    for item in children {
      if item.participates_in_inline_formatting_context() && !item.is_out_of_flow() {
        inline_group.push(item);
        continue;
      }

      flush_inline_group(&mut inline_group, &mut final_children, context);
      final_children.push(item);
    }

    flush_inline_group(&mut inline_group, &mut final_children, context);
    final_children.into_boxed_slice()
  }

  /// Padding in pixels, with a percentage resolving to zero.
  pub(super) fn padding_px(&self) -> Rect<f32> {
    let style = &self.context.style;

    Rect {
      top: style.padding_top,
      right: style.padding_right,
      bottom: style.padding_bottom,
      left: style.padding_left,
    }
    .map(|length| length.to_px(&self.context.sizing, 0.0))
  }

  /// Margins in pixels, with a percentage resolving to zero.
  pub(super) fn margin_px(&self) -> Rect<f32> {
    let style = &self.context.style;

    Rect {
      top: style.margin_top,
      right: style.margin_right,
      bottom: style.margin_bottom,
      left: style.margin_left,
    }
    .map(|length| length.to_px(&self.context.sizing, 0.0))
  }

  fn inline_content_baseline_offset(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    use_last_line: bool,
  ) -> Option<f32> {
    let baseline = self.inline_content_border_box_baseline(available_space, size, use_last_line)?;

    Some(self.margin_px().top + baseline)
  }

  /// Baseline of the first or last line box, measured from the border box top.
  fn inline_content_border_box_baseline(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    use_last_line: bool,
  ) -> Option<f32> {
    if matches!(
      self.node.as_ref().and_then(Node::inline_content),
      Some(InlineContentKind::Box)
    ) {
      return None;
    }

    // An atomic box with no in-flow inline content has no line boxes, so it has
    // no content baseline; the caller must fall back to the bottom margin edge.
    // https://www.w3.org/TR/CSS22/visudet.html#leading
    let items = collect_inline_items(self);
    if items.is_empty() {
      return None;
    }

    // `size` is the border box, but the content wrapped against the content box.
    let known_dimensions = Size {
      width: Some(size.width.max(0.0)),
      height: None,
    };
    // A clamped measurement dropped lines this pass keeps, so it lays out again.
    let measured = self
      .plain_text(&items)
      .map(|text| text.measurement(&self.context, available_space, known_dimensions))
      .filter(|measured| !measured.clamped);
    let baseline = match measured {
      Some(measured) => {
        if use_last_line {
          measured.last_baseline?
        } else {
          measured.first_baseline?
        }
      }
      None => {
        let font_style = SizedFontStyle::from_style(&self.context.style, &self.context);
        let (max_width, _) =
          create_inline_constraint(&self.context, available_space, known_dimensions);
        let built = create_inline_layout(InlineLayoutRequest {
          items,
          available_space: Size {
            width: AvailableSpace::Definite(max_width),
            height: available_space.height,
          },
          max_width,
          max_height: None,
          style: &font_style,
          context: &self.context,
          mode: InlineLayoutMode::Measure,
          shape_cacheable: true,
        });
        let resolved = built.line_metrics();
        let line = if use_last_line {
          resolved.last()?
        } else {
          resolved.first()?
        };
        line.resolved_baseline
      }
    };
    let sizing = &self.context.sizing;
    let border_top = Length::from(self.context.style.border_top_width).to_px(sizing, 0.0);
    let padding_top = self.context.style.padding_top.to_px(sizing, 0.0);

    Some(border_top + padding_top + baseline)
  }

  /// This node's text when it lays out exactly as [`TextData::measurement`] does.
  ///
  /// A text node's own text is always among its items, so a lone text item is that text.
  fn plain_text(&self, items: &[InlineItem<'_>]) -> Option<&TextData> {
    let text = self.node.as_ref()?.text_data()?;
    let [
      InlineItem::Text {
        link: None,
        decorations: None,
        ..
      },
    ] = items
    else {
      return None;
    };

    Some(text)
  }

  fn layout_first_baseline_offset(&self, results: &LayoutResults) -> Option<f32> {
    let baseline = results.first_baseline_y(NodeId::ROOT).ok().flatten()?;

    Some(self.margin_px().top + baseline)
  }

  fn valid_baseline_offset(candidate: Option<f32>, box_height: f32) -> Option<f32> {
    candidate
      .filter(|baseline| baseline.is_finite() && *baseline >= 0.0 && *baseline <= box_height + 0.5)
  }

  /// Where an atomic inline box takes its baseline from, in order; an empty list, or no source
  /// that resolves, falls back to the bottom margin edge.
  fn inline_baseline_sources(&self) -> &'static [InlineBaselineSource] {
    match self.context.style.display {
      Display::InlineBlock if self.context.style.clips_overflow() => &[],
      Display::InlineBlock => &[
        InlineBaselineSource::InlineContentLastLine,
        InlineBaselineSource::LayoutFirstBaseline,
      ],
      Display::InlineFlex | Display::InlineGrid => &[
        InlineBaselineSource::InlineContentLastLine,
        InlineBaselineSource::InlineContentFirstLine,
        InlineBaselineSource::LayoutFirstBaseline,
      ],
      _ => &[],
    }
  }

  fn resolve_inline_baseline_source(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    source: InlineBaselineSource,
    results: &LayoutResults,
  ) -> Option<f32> {
    match source {
      InlineBaselineSource::InlineContentLastLine => {
        self.inline_content_baseline_offset(available_space, size, true)
      }
      InlineBaselineSource::InlineContentFirstLine => {
        self.inline_content_baseline_offset(available_space, size, false)
      }
      InlineBaselineSource::LayoutFirstBaseline => self.layout_first_baseline_offset(results),
    }
  }

  fn resolve_inline_baseline_offset(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    results: &LayoutResults,
  ) -> Option<f32> {
    let margin = self.margin_px();
    let margin_box_height = size.height + margin.top + margin.bottom;

    self.inline_baseline_sources().iter().find_map(|&source| {
      let candidate = self.resolve_inline_baseline_source(available_space, size, source, results);

      Self::valid_baseline_offset(candidate, margin_box_height)
    })
  }

  pub(crate) fn measure_inline_box(
    &self,
    available_space: Size<AvailableSpace>,
  ) -> AtomicInlineMetrics {
    if self.participates_as_inline_box() {
      return self.measure_atomic_subtree(available_space);
    }

    // Only an atomic box has a baseline of its own; a replaced one sits on its bottom margin edge.
    AtomicInlineMetrics {
      size: self.node.as_ref().map_or(Size::ZERO, |node| {
        node.measure(
          &self.context,
          available_space,
          Size::NONE,
          &self.layout_style(&self.context.sizing),
        )
      }),
      baseline_offset: None,
    }
  }

  /// An atomic inline box's shrink-to-fit size and baseline.
  fn measure_atomic_subtree(&self, available_space: Size<AvailableSpace>) -> AtomicInlineMetrics {
    let at_width = |width| Size {
      width,
      height: available_space.height,
    };

    // CSS shrink-to-fit for inline-level atomic boxes:
    // width = min(max-content, max(min-content, available)).
    // Reference: https://www.w3.org/TR/CSS22/visudet.html#float-width
    let min_content =
      LayoutResults::compute(self, at_width(AvailableSpace::MinContent)).root_size();
    let max_content = {
      let mut tree = LayoutTree::from_render_node(self);
      // Hack: Use Flexbox to avoid Block's "expand to fill" behavior when calculating max-content.
      // We want the content's preferred width, not the container's available width.
      if let Some(node) = tree.get_layout_node_mut_ref(NodeId::ROOT.into_taffy())
        && node.style.display == TaffyDisplay::Block
      {
        node.style.display = TaffyDisplay::Flex;
        node.style.flex_direction = taffy::FlexDirection::Row;
        node.style.justify_content = Some(taffy::JustifyContent::START);
      }

      tree.compute_layout(at_width(AvailableSpace::MaxContent));
      tree.into_results().root_size()
    };
    let used_width = match available_space.width {
      AvailableSpace::Definite(available) => {
        max_content.width.min(min_content.width.max(available))
      }
      AvailableSpace::MinContent => min_content.width,
      AvailableSpace::MaxContent => max_content.width,
    };
    let results = LayoutResults::compute(self, at_width(AvailableSpace::Definite(used_width)));

    results.layout(NodeId::ROOT).map_or(
      AtomicInlineMetrics {
        size: Size::ZERO,
        baseline_offset: None,
      },
      |layout| AtomicInlineMetrics {
        size: layout.size,
        baseline_offset: self.resolve_inline_baseline_offset(
          available_space,
          layout.size,
          &results,
        ),
      },
    )
  }

  pub(crate) fn measure(
    &self,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &Style,
    is_inline_children: bool,
  ) -> Size<f32> {
    if is_inline_children {
      let font_style = SizedFontStyle::from_style(&self.context.style, &self.context);
      let request = InlineLayoutRequest::in_available_space(
        collect_inline_items(self),
        available_space,
        known_dimensions,
        &font_style,
        &self.context,
        InlineLayoutMode::Measure,
      );
      let options = InlineMeasureOptions::new(
        request.max_width,
        font_style.parent.resolved_text_wrap_mode() == TextWrapMode::Wrap,
        available_space,
        known_dimensions,
      );

      return create_inline_layout(request).measure(options).size;
    }

    assert_ne!(
      self.context.style.display,
      Display::Inline,
      "Inline nodes should be wrapped in anonymous block boxes"
    );

    let Some(node) = &self.node else {
      return Size::ZERO;
    };

    node.measure(&self.context, available_space, known_dimensions, style)
  }
}

fn flush_inline_group(
  inline_group: &mut Vec<RenderNode>,
  final_children: &mut Vec<RenderNode>,
  parent_render_context: &RenderContext,
) {
  if inline_group.is_empty() {
    return;
  }

  final_children.push(RenderNode::anonymous_block_container(
    parent_render_context,
    take(inline_group),
  ));
}

// Mirrors Blink's Text::TextLayoutObjectIsNeeded, minus the ends-with-space
// refinement (inline collapsing already merges adjacent spaces).
fn drop_collapsible_boundary_whitespace(
  input: Vec<RenderNode>,
  parent_is_inline: bool,
) -> Vec<RenderNode> {
  let mut out = Vec::with_capacity(input.len());
  let mut after_in_flow_inline = parent_is_inline;

  for child in input {
    if child.is_collapsible_whitespace_only_text_node() && !after_in_flow_inline {
      continue;
    }

    if !child.is_out_of_flow() && child.context.style.float == Float::None {
      after_in_flow_inline = child.participates_in_inflow_inline_formatting_context();
    }

    out.push(child);
  }

  out
}

/// A node whose context is resolved while its children are still being built.
struct PendingRenderNode {
  context: RenderContext,
  node: Node,
  source_order: usize,
  children_is_some: bool,
  pending_children: IntoIter<Node>,
  rendered_children: Vec<RenderNode>,
  pseudo_after: Option<RenderNode>,
  list_counter: ListCounter,
  marker_ordinal: Option<i32>,
  inside_list: bool,
  owns_list_counter: bool,
}

impl PendingRenderNode {
  fn build(
    parent_context: &RenderContext,
    mut node: Node,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
    source_cursor: &mut usize,
    counter: &mut ListCounter,
    inside_list: bool,
  ) -> Self {
    let source_order = *source_cursor;
    *source_cursor += 1;
    let (style, sizing, current_color) =
      parent_context.resolve_child_style(&mut node, source_order, matched_declarations);
    let children = node.take_children();
    let children_is_some = children.is_some();
    let children = children.map_or_else(Vec::new, <[Node]>::into_vec);
    let context = RenderContext::from_parent(parent_context, style, sizing, current_color);

    let element_matched = matched_declarations.get(source_order);
    let marker_ordinal = (context.style.display == Display::ListItem).then(|| counter.take(&node));
    let pseudo_before = element_matched
      .and_then(|m| m.before())
      .and_then(|m| RenderNode::from_pseudo_match(&context, &node, m));
    let pseudo_after = element_matched
      .and_then(|m| m.after())
      .and_then(|m| RenderNode::from_pseudo_match(&context, &node, m));
    let pseudo_before_present = pseudo_before.is_some();

    let has_generated_children =
      marker_ordinal.is_some() || pseudo_before.is_some() || pseudo_after.is_some();
    let mut rendered_children = Vec::with_capacity(children.len() + 3);
    rendered_children.extend(pseudo_before);

    // The inline collector emits an element's own text before any child, so
    // an element that folded its text has to hand it back as a child for the
    // generated content to come first.
    if pseudo_before_present && let Some(text) = node.take_text() {
      rendered_children.push(RenderNode::generated_sibling_text(&context, text));
    }

    let owns_list_counter = owns_list_counter(&node, inside_list);

    Self {
      source_order,
      children_is_some: children_is_some || has_generated_children,
      list_counter: if owns_list_counter {
        ListCounter::new(&node)
      } else {
        *counter
      },
      inside_list: inside_list || is_list_element(&node),
      owns_list_counter,
      context,
      node,
      rendered_children,
      pending_children: children.into_iter(),
      pseudo_after,
      marker_ordinal,
    }
  }

  /// Closes the node once every child is rendered.
  fn finish(mut self) -> RenderNode {
    if let Some(after) = self.pseudo_after.take() {
      self.rendered_children.push(after);
    }

    let mut context = self.context;
    let children = if self.children_is_some {
      Some(RenderNode::normalize_children(
        &mut context,
        self.rendered_children.into_boxed_slice(),
      ))
    } else {
      Self::anonymous_text_child(&context, &self.node).map(|child| Box::from([child]))
    };
    let mut render_node = RenderNode::new(
      context,
      NodeOrigin::Authored {
        source_order: self.source_order,
      },
      Some(self.node),
      children,
    );

    if let Some(ordinal) = self.marker_ordinal
      && let Some(marker) = list_marker(&render_node.context, ordinal)
    {
      render_node.attach_marker(marker);
    }

    render_node
  }

  /// The text a childless flex or grid item carries, wrapped as its own child.
  fn anonymous_text_child(context: &RenderContext, node: &Node) -> Option<RenderNode> {
    if !context.style.display.should_blockify_children() {
      return None;
    }

    let text = node.inline_content().and_then(|content| match content {
      InlineContentKind::Text(text) => Some(text.into_owned()),
      InlineContentKind::Box => None,
    })?;

    Some(RenderNode::anonymous_text_item(context, text))
  }
}

impl RenderContext {
  /// The used `line-height: normal` for `style` at `font_size`, or zero for any other
  /// `line-height`.
  pub(crate) fn resolve_normal_line_height(&self, style: &ComputedStyle, font_size: f32) -> f32 {
    if !matches!(style.line_height, LineHeight::Normal) {
      return 0.0;
    }
    let attributes = Attributes {
      width: style.font_stretch.into_parlance(),
      style: style.font_style.into_parlance(),
      weight: style.font_weight.into_parlance(),
    };
    let font_family = self.expand_font_family(&style.font_family);

    let mut hasher = Xxh3::new();
    font_family.hash_tokens(&mut hasher);
    hasher.write_u32(attributes.weight.value().to_bits());
    hasher.write_u32(attributes.width.ratio().to_bits());
    match attributes.style {
      FontiqueStyle::Normal => hasher.write_u8(0),
      FontiqueStyle::Italic => hasher.write_u8(1),
      FontiqueStyle::Oblique(angle) => {
        hasher.write_u8(2);
        hasher.write_u32(angle.unwrap_or(f32::NAN).to_bits());
      }
    }
    hasher.write_u32(font_size.to_bits());

    self.normal_line_height(hasher.finish(), || {
      self
        .first_font_line_spacing(font_family.query_families(), attributes, font_size)
        .unwrap_or(font_size)
    })
  }

  /// A child's cascaded style and its element-owned important declarations.
  fn cascade(
    &self,
    node_layers: NodeStyleLayers,
    matched_declarations: &MatchedDeclarationsView<'_>,
  ) -> (NodeStyle, ElementImportant) {
    let mut style = NodeStyle::default();

    // `tw` is the last declared layer, below unlayered author rules, so its
    // important half goes last: the cascade reverses layer order for important
    // declarations.
    let tw = node_layers.author_tw.map(|author_tw| {
      author_tw.declaration_blocks(
        self.sizing.viewport,
        &self.stylesheet().breakpoints,
        self.tw_cache(),
      )
    });

    if let Some(preset) = node_layers.preset {
      style.append_block(preset.declarations);
    }

    if let Some(dir) = node_layers.dir {
      style.push(StyleDeclaration::direction(dir), false);
    }

    for &declarations in matched_declarations.layered_normal() {
      style.merge_matched_block(declarations);
    }

    // `tw` is the last declared layer, as Tailwind orders utilities: above every
    // named `@layer`, below unlayered author rules.
    if let Some(tw) = &tw {
      style.append_block_cloned(&tw.normal);
    }

    for &declarations in matched_declarations.unlayered_normal() {
      style.merge_matched_block(declarations);
    }

    // An element's own declarations outrank selector-based ones at the same
    // importance.
    let (inline_normal, inline_important) = node_layers
      .inline
      .map(|inline| StyleDeclarationBlock::from(inline).split_importance())
      .unzip();

    if let Some(inline_normal) = inline_normal {
      style.append_block(inline_normal);
    }

    // Important declarations reverse layer order, so `tw`, the last declared
    // layer, sits above unlayered rules and below every named `@layer`.
    for &declarations in matched_declarations.unlayered_important() {
      style.merge_matched_block(declarations);
    }

    if let Some(tw) = &tw {
      style.append_block_cloned(&tw.important);
    }

    for &declarations in matched_declarations.layered_important() {
      style.merge_matched_block(declarations);
    }

    if let Some(inline_important) = &inline_important {
      style.append_block(inline_important.clone());
    }

    (
      style,
      ElementImportant {
        tw,
        inline: inline_important,
      },
    )
  }

  /// This style as a child inherits it, with the stylesheet's registered custom properties.
  fn inherited_style(&self) -> Cow<'_, ComputedStyle> {
    registered_custom_property_parent_style(
      &self.style,
      slice::from_ref(self.stylesheet().as_ref()),
      self.sizing.viewport,
    )
  }

  /// Resolves a generated box's style and sizing from its matched declarations.
  pub(super) fn resolve_pseudo_style(
    &self,
    pseudo_matched: &MatchedDeclarationsView<'_>,
  ) -> (ComputedStyle, SizingContext, Color) {
    let (style_layers, _) = self.cascade(NodeStyleLayers::default(), pseudo_matched);
    let mut style = style_layers.inherit(&self.inherited_style());
    let sizing = self.child_sizing(&style, &self.sizing, false);
    let current_color = style.color.resolve(self.current_color);

    style.make_computed(&sizing);
    (style, sizing, current_color)
  }

  /// Resolves a child's style and sizing from its matched declarations.
  fn resolve_child_style(
    &self,
    node: &mut Node,
    source_order: usize,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
  ) -> (ComputedStyle, SizingContext, Color) {
    let default_matched = MatchedDeclarationsView::default();
    let matched = matched_declarations
      .get(source_order)
      .map(NodeMatchedDeclarations::element)
      .unwrap_or(&default_matched);
    let layers = node.take_style_layers();
    let lang = layers.lang;

    let (style_layers, element_important) = self.cascade(layers, matched);
    let inherited_parent = self.inherited_style();

    let mut style = style_layers.inherit_with_lang(&inherited_parent, lang);

    // A tree built in code is content, not a document: `rem` resolves against
    // the viewport, so a `font-size` on the outermost node styles text without
    // rescaling every `rem` length below it. A parsed document is the
    // exception, since its outermost node is the `<html>` element that CSS
    // does make the `rem` basis.
    let is_document_root = source_order == 0
      && node
        .tag_name()
        .is_some_and(|tag| tag.eq_ignore_ascii_case("html"));

    let mut child_sizing_for_final: Option<SizingContext> = None;
    if !style.animation_name.is_empty() {
      let (animated, child_sizing) = self.animated_style(style);

      style = animated;
      child_sizing_for_final = Some(child_sizing);
    }

    // Important declarations outrank an animation, and `inherit` among them
    // needs the parent the first pass resolved against.
    let important = matched
      .unlayered_important()
      .iter()
      .map(|declarations| declarations.iter())
      .chain(
        element_important
          .tw
          .iter()
          .map(|blocks| blocks.important.iter()),
      )
      .chain(
        matched
          .layered_important()
          .iter()
          .map(|declarations| declarations.iter()),
      )
      .chain(
        element_important
          .inline
          .iter()
          .map(|declarations| declarations.iter()),
      );

    for declarations in important {
      for declaration in declarations {
        declaration
          .clone()
          .apply_with_parent(&mut style, &inherited_parent);
      }
    }

    let sizing = self.child_sizing(
      &style,
      child_sizing_for_final.as_ref().unwrap_or(&self.sizing),
      is_document_root,
    );
    let current_color = style.color.resolve(self.current_color);
    style.make_computed(&sizing);
    (style, sizing, current_color)
  }

  /// The sizing a child with `style` resolves against. Its font size resolves in
  /// `font_size_basis`, and a document root sets the root font metrics itself.
  fn child_sizing(
    &self,
    style: &ComputedStyle,
    font_size_basis: &SizingContext,
    is_document_root: bool,
  ) -> SizingContext {
    let font_size = style
      .font_size
      .to_px(font_size_basis, font_size_basis.font_size);
    let normal_basis = self.resolve_normal_line_height(style, font_size);
    let line_height = style.line_height.to_px(&self.sizing, normal_basis);

    self.sizing.with_font_metrics(
      font_size,
      self
        .sizing
        .root_font_size
        .or_else(|| is_document_root.then_some(font_size)),
      line_height,
      self
        .sizing
        .root_line_height
        .or_else(|| is_document_root.then_some(line_height)),
    )
  }

  /// Samples the stylesheet animations on `style` at this render's time,
  /// against the sizing the pre-animation style produces.
  fn animated_style(&self, style: ComputedStyle) -> (ComputedStyle, SizingContext) {
    let child_sizing = self.child_sizing(&style, &self.sizing, false);
    let child_current_color = style.color.resolve(self.current_color);
    let child_context = RenderContext::from_parent(
      self,
      style.clone(),
      child_sizing.clone(),
      child_current_color,
    );
    let animated = apply_stylesheet_animations(
      style,
      child_context.stylesheet(),
      child_context.time_ms(),
      &child_context.sizing,
      child_context.current_color,
    );

    (animated, child_sizing)
  }
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use taffy::NodeId as TaffyNodeId;

  use super::{
    NodeOrigin, RenderNode, registered_custom_property_parent_style, sort_children_by_order,
  };
  use crate::{
    context::RenderContext,
    resources::font::Fonts,
    style::{
      ComputedStyle, Length, PropertyRule, SizingContext, Style, StyleDeclaration,
      StyleDeclarationBlock, StyleSheet,
    },
    viewport::Viewport,
  };

  fn parse_stylesheet(css: &str) -> StyleSheet {
    let result = StyleSheet::parse(css);
    assert!(result.is_ok(), "expected stylesheet to parse: {result:?}");
    result.unwrap_or_default()
  }

  #[test]
  fn render_node_drop_is_iterative() {
    let context = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .build();
    let leaf = |children: Option<Box<[RenderNode]>>| {
      RenderNode::new(context.clone(), NodeOrigin::Anonymous, None, children)
    };

    let mut root = leaf(None);
    for _ in 0..500_000 {
      root = leaf(Some(Box::new([root])));
    }

    drop(root);
  }

  #[test]
  fn anonymous_box_has_no_used_border_width() {
    let sizing = SizingContext::builder()
      .viewport(Viewport::default())
      .build();
    let parent = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(sizing)
      .build();

    let anonymous = RenderContext::for_anonymous(&parent);
    let style = &anonymous.style;
    let sizing = &anonymous.sizing;

    // `border-style` is `none`, so the initial `medium` width has a used value
    // of zero: an anonymous box reports no border it cannot render.
    assert_eq!(style.border_top_width.to_used_px(sizing), 0.0);
    assert_eq!(style.border_right_width.to_used_px(sizing), 0.0);
    assert_eq!(style.border_bottom_width.to_used_px(sizing), 0.0);
    assert_eq!(style.border_left_width.to_used_px(sizing), 0.0);
  }

  #[test]
  fn sort_children_by_order_keeps_source_order_for_equal_values() {
    let mut children = vec![
      TaffyNodeId::from(3usize),
      TaffyNodeId::from(1usize),
      TaffyNodeId::from(2usize),
    ];
    sort_children_by_order(&mut children, |child_id| match usize::from(child_id) {
      1 => -1,
      _ => 0,
    });
    assert_eq!(
      children,
      vec![
        TaffyNodeId::from(1usize),
        TaffyNodeId::from(3usize),
        TaffyNodeId::from(2usize)
      ]
    );
  }

  #[test]
  fn registered_custom_property_can_disable_inheritance() {
    let mut parent = ComputedStyle::default();
    parent
      .custom_properties
      .set("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet::from(vec![PropertyRule {
      name: "--box-size".to_owned(),
      syntax: "*".to_owned(),
      inherits: false,
      initial_value: Some("10px".to_owned()),
      media_queries: Vec::new(),
    }])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("10px")
    );
  }

  /// An `@property` registration is what decides whether a name inherits, even
  /// for the `--tw-*` names the utility engine also writes.
  #[test]
  fn a_registered_tw_property_reaches_the_child() {
    let parent = ComputedStyle::default();
    let stylesheets = [StyleSheet::from(vec![PropertyRule {
      name: "--tw-gradient-from-position".to_owned(),
      syntax: "<length-percentage>".to_owned(),
      inherits: false,
      initial_value: Some("0%".to_owned()),
      media_queries: Vec::new(),
    }])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    let child = ComputedStyle::from_parent(&adjusted_parent);

    assert_eq!(
      child.custom_properties.get("--tw-gradient-from-position"),
      Some("0%")
    );
  }

  #[test]
  fn registered_custom_property_preserves_parent_value_when_inheriting() {
    let mut parent = ComputedStyle::default();
    parent
      .custom_properties
      .set("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet::from(vec![PropertyRule {
      name: "--box-size".to_owned(),
      syntax: "*".to_owned(),
      inherits: true,
      initial_value: Some("10px".to_owned()),
      media_queries: Vec::new(),
    }])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("50px")
    );
  }

  #[test]
  fn registered_custom_property_uses_initial_value_when_missing_and_inheriting() {
    let parent = ComputedStyle::default();

    let stylesheets = [StyleSheet::from(vec![PropertyRule {
      name: "--box-size".to_owned(),
      syntax: "*".to_owned(),
      inherits: true,
      initial_value: Some("10px".to_owned()),
      media_queries: Vec::new(),
    }])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("10px")
    );
  }

  #[test]
  fn registered_custom_property_uses_last_inherited_initial_value_when_parent_is_missing() {
    let parent = ComputedStyle::default();

    let stylesheets = [StyleSheet::from(vec![
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      },
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: Some("20px".to_owned()),
        media_queries: Vec::new(),
      },
    ])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("20px")
    );
  }

  #[test]
  fn registered_custom_property_later_inheriting_rule_restores_parent_value() {
    let mut parent = ComputedStyle::default();
    parent
      .custom_properties
      .set("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet::from(vec![
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: false,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      },
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: Some("20px".to_owned()),
        media_queries: Vec::new(),
      },
    ])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("50px")
    );
  }

  #[test]
  fn registered_custom_property_later_inheriting_rule_clears_prior_synthesized_value_without_initial_value()
   {
    let parent = ComputedStyle::default();

    let stylesheets = [StyleSheet::from(vec![
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: false,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      },
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: None,
        media_queries: Vec::new(),
      },
    ])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(adjusted_parent.custom_properties.get("--box-size"), None);
  }

  #[test]
  fn registered_custom_property_accepts_assignment_without_syntax_validation() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--box-size".to_owned(),
      "red".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--box-size"), Some("red"));
  }

  #[test]
  fn registered_custom_property_accepts_valid_length_assignment() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--box-size".to_owned(),
      "24px".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--box-size"), Some("24px"));
  }

  #[test]
  fn registered_custom_property_keeps_var_assignment_without_validation() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default()
      .with(StyleDeclaration::CustomProperty(
        "--source".to_owned(),
        "18px".to_owned(),
      ))
      .with(StyleDeclaration::CustomProperty(
        "--box-size".to_owned(),
        "var(--source)".to_owned(),
      ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(
      resolved.custom_properties.get("--box-size"),
      Some("var(--source)")
    );
  }

  #[test]
  fn registered_custom_property_still_accepts_keyword_assignment() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --display-state {
          syntax: "none | auto";
          inherits: false;
          initial-value: none;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--display-state".to_owned(),
      "auto".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(
      resolved.custom_properties.get("--display-state"),
      Some("auto")
    );
  }

  #[test]
  fn registered_custom_property_still_accepts_alternative_assignment() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --accent {
          syntax: "<length> | <color>";
          inherits: false;
          initial-value: red;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--accent".to_owned(),
      "12px".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--accent"), Some("12px"));
  }

  #[test]
  fn registered_custom_property_still_accepts_supported_assignments() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --fade-duration {
          syntax: "<time>";
          inherits: false;
          initial-value: 150ms;
        }
        @property --move {
          syntax: "<transform-function>";
          inherits: false;
          initial-value: translate(10px, 20px);
        }
        @property --bg {
          syntax: "<image>";
          inherits: false;
          initial-value: linear-gradient(red, blue);
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default()
      .with(StyleDeclaration::CustomProperty(
        "--fade-duration".to_owned(),
        "2s".to_owned(),
      ))
      .with(StyleDeclaration::CustomProperty(
        "--move".to_owned(),
        "rotate(45deg)".to_owned(),
      ))
      .with(StyleDeclaration::CustomProperty(
        "--bg".to_owned(),
        "url(hero.png)".to_owned(),
      ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(
      resolved.custom_properties.get("--fade-duration"),
      Some("2s")
    );
    assert_eq!(
      resolved.custom_properties.get("--move"),
      Some("rotate(45deg)")
    );
    assert_eq!(
      resolved.custom_properties.get("--bg"),
      Some("url(hero.png)")
    );
  }

  #[test]
  fn registered_custom_property_initial_value_applies_through_var_resolution() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let declarations = StyleDeclarationBlock::from_str("width: var(--box-size)");
    assert!(
      declarations.is_ok(),
      "width declaration using registered custom property should parse: {declarations:?}"
    );
    let Ok(declarations) = declarations else {
      return;
    };

    let mut style = Style::default();
    style.append_block(declarations);

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.width, Length::Px(10.0).into());
  }

  #[test]
  fn registered_custom_property_accepts_invalid_transform_assignment_without_validation() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --move {
          syntax: "<transform-function>";
          inherits: false;
          initial-value: translate(10px, 20px);
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--move".to_owned(),
      "red".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--move"), Some("red"));
  }

  #[test]
  fn lang_pseudo_class_matches_the_nearest_ancestor_or_self_lang_attribute() {
    use std::sync::Arc;

    use crate::{
      context::RenderContext,
      layout::{node::Node, tree::RenderNode},
      resources::font::Fonts,
      style::{Lang, SizingContext},
    };

    let stylesheet = StyleSheet::parse(
      r#"
        :lang(zh-Hant) { width: 10px; }
        :lang(ja) { width: 20px; }
      "#,
    )
    .expect("stylesheet parses");

    let fonts = Fonts::default();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .stylesheet(Arc::new(stylesheet))
      .build();

    let tree = RenderNode::from_node(
      &context,
      Node::container([
        // No `lang` of its own — `:lang(zh-Hant)` must walk up to the root to match.
        Node::container([Node::text("inherits")]),
        Node::container([Node::text("overrides")]).with_lang(Lang::parse("ja").unwrap()),
      ])
      .with_lang(Lang::parse("zh-Hant").unwrap()),
    );

    assert_eq!(tree.context.style.width, Length::Px(10.0).into());

    let children = tree.children.as_deref().expect("block children");
    assert_eq!(children[0].context.style.width, Length::Px(10.0).into());
    assert_eq!(children[1].context.style.width, Length::Px(20.0).into());
  }

  #[test]
  fn rem_follows_the_document_root_only_when_the_tree_is_a_document() {
    use std::sync::Arc;

    use crate::{
      context::RenderContext,
      layout::{node::Node, tree::RenderNode},
      resources::font::Fonts,
      style::{SizingContext, StyleSheet},
      viewport::Viewport,
    };

    fn child_width(root: Node) -> f32 {
      let stylesheet = StyleSheet::parse("#root { font-size: 32px } #child { width: 1rem }")
        .expect("stylesheet parses");
      let fonts = Fonts::default();
      let context = RenderContext::builder()
        .fonts(fonts.snapshot())
        .sizing(
          SizingContext::builder()
            .viewport(Viewport::default())
            .build(),
        )
        .stylesheet(Arc::new(stylesheet))
        .build();

      let tree = RenderNode::from_node(&context, root);
      let children = tree.children.as_deref().expect("children");

      let child = &children[0];

      child
        .context
        .style
        .width
        .as_length()
        .expect("length width")
        .to_px(&child.context.sizing, 0.0)
    }

    let content = Node::container([Node::container([]).with_id("child")]).with_id("root");
    assert_eq!(child_width(content), 16.0);

    let document = Node::container([Node::container([]).with_id("child")])
      .with_id("root")
      .with_tag_name("html");
    assert_eq!(child_width(document), 32.0);
  }
}
