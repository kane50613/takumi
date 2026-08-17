use std::{borrow::Cow, collections::HashMap, mem::take, sync::Arc, vec::IntoIter};

use parley::fontique::Attributes;
use taffy::{
  BlockContext, Cache, CacheTree, Display as TaffyDisplay, Layout, LayoutBlockContainer,
  LayoutFlexboxContainer, LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree,
  NodeId as TaffyNodeId, RequestedAxis, RoundTree, RunMode, Size as TaffySize, SizingMode, Style,
  TraversePartialTree, TraverseTree, compute_block_layout, compute_cached_layout,
  compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
  compute_root_layout,
};

use crate::{
  Error,
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, ComputedLayout, NodeId, Size},
  layout::{
    inline::{
      InlineContentKind, InlineLayoutMode, InlineLayoutRequest, InlineMeasureOptions,
      collect_inline_items, create_inline_constraint, create_inline_layout, measure_inline_layout,
    },
    list_marker::{ListCounter, is_list_element, list_marker, owns_list_counter},
    node::{Node, NodeStyleLayers},
    table::lower_tables,
  },
  matching::{MatchedDeclarationsView, NodeMatchedDeclarations, match_stylesheets_view},
  style::{
    BackgroundImage, BackgroundImages, BlendMode, BoxSizing, Color, ComputedStyle, ContentItem,
    ContentValue, Display, Filters, Float, Isolation, Length, LineHeight, ListStylePosition,
    PercentageNumber, Position, SizingContext, Style as NodeStyle, StyleDeclaration, StyleSheet,
    TextWrapMode, WhiteSpaceCollapse, apply_stylesheet_animations,
  },
  viewport::Viewport,
};

/// A render-tree child paired with its layout node id. `hoisted_cb` is set
/// when the child is out-of-flow and was re-parented to a containing block in
/// the layout tree; its geometry then resolves against that block.
#[derive(Debug, Clone, Copy)]
pub struct OrderedChild {
  /// Index of the child in its parent's render order.
  pub render_index: usize,
  /// Layout node id.
  pub node_id: NodeId,
  /// Containing block the child was hoisted to, if out-of-flow.
  pub hoisted_cb: Option<NodeId>,
}

/// Immutable per-node layout output after computing a tree.
pub struct LayoutResults {
  nodes: Vec<LayoutResultNode>,
}

struct LayoutResultNode {
  layout: Layout,
  first_baseline_y: Option<f32>,
  box_children: Box<[OrderedChild]>,
}

impl LayoutResults {
  /// Computed layout of a node.
  pub fn layout(&self, node_id: NodeId) -> crate::Result<ComputedLayout> {
    let idx: usize = node_id.into();
    self
      .nodes
      .get(idx)
      .map(|node| ComputedLayout::from_taffy(&node.layout))
      .ok_or(Error::InvalidLayoutNode(node_id.into()))
  }

  /// Paint-ordered children of a node.
  pub fn box_children(&self, node_id: NodeId) -> crate::Result<&[OrderedChild]> {
    let idx: usize = node_id.into();
    self
      .nodes
      .get(idx)
      .map(|node| node.box_children.as_ref())
      .ok_or(Error::InvalidLayoutNode(node_id.into()))
  }

  pub(crate) fn first_baseline_y(&self, node_id: NodeId) -> crate::Result<Option<f32>> {
    let idx: usize = node_id.into();
    self
      .nodes
      .get(idx)
      .map(|node| node.first_baseline_y)
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
  /// Stylesheet matching keys its results by the same number.
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
  pub(crate) layout_style_override: Option<Style>,
  /// Text for an anonymous inline-text wrapper.
  pub anonymous_text_content: Option<String>,
  /// Generated marker box, emitted before this box's own inline content.
  pub(crate) marker: Option<Box<RenderNode>>,
  pub(crate) force_inline_layout: bool,
  /// Grid lines a lowered table's header rows cover, as `[start, end)`, for
  /// paged output to repeat per css-tables-3 §repeated-headers.
  pub table_header_lines: Option<(i16, i16)>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InlineBaselineFallback {
  BottomMarginEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InlineBaselineBoxKind {
  AtomicContainer,
  Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InlineBaselineStrategy {
  sources: &'static [InlineBaselineSource],
  fallback: InlineBaselineFallback,
}

fn resolve_normal_line_height(
  context: &RenderContext,
  style: &ComputedStyle,
  font_size: f32,
) -> f32 {
  if !matches!(style.line_height, LineHeight::Normal) {
    return 0.0;
  }
  let attributes = Attributes {
    width: style.font_stretch.into_parlance(),
    style: style.font_style.into_parlance(),
    weight: style.font_weight.into_parlance(),
  };
  let font_family = context.expand_font_family(&style.font_family);

  context
    .first_font_line_spacing(font_family.query_families(), attributes, font_size)
    .unwrap_or(font_size)
}

fn build_style_layers(
  node_layers: NodeStyleLayers,
  matched_declarations: &MatchedDeclarationsView<'_>,
  viewport: Viewport,
) -> NodeStyle {
  let mut style = NodeStyle::default();

  if let Some(preset) = node_layers.preset {
    style.merge_from(preset);
  }

  if let Some(dir) = node_layers.dir {
    style.push(StyleDeclaration::direction(dir), false);
  }

  for &declarations in matched_declarations.normal() {
    for declaration in declarations.iter() {
      declaration.merge_into_ref(&mut style);
    }
  }

  if let Some(author_tw) = node_layers.author_tw {
    style.append_block(author_tw.into_declaration_block(viewport));
  }

  if let Some(inline) = node_layers.inline {
    style.merge_from(inline);
  }

  for &declarations in matched_declarations.important() {
    for declaration in declarations.iter() {
      declaration.merge_into_ref(&mut style);
    }
  }

  style
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
  let registered = Arc::make_mut(&mut adjusted_parent.registered_custom_properties);
  let custom = Arc::make_mut(&mut adjusted_parent.custom_properties);

  for sheet in stylesheets {
    for property_rule in sheet.property_rules() {
      if !property_rule
        .media_queries
        .iter()
        .all(|media_query| media_query.matches(viewport))
      {
        continue;
      }

      registered.insert(property_rule.name.clone(), property_rule.clone());

      if property_rule.inherits {
        if let Some(parent_value) = parent_style.custom_properties.get(&property_rule.name) {
          custom.insert(property_rule.name.clone(), parent_value.clone());
        } else if let Some(initial_value) = &property_rule.initial_value {
          custom.insert(property_rule.name.clone(), initial_value.clone());
        } else {
          custom.remove(&property_rule.name);
        }
      } else {
        custom.remove(&property_rule.name);
        if let Some(initial_value) = &property_rule.initial_value {
          custom.insert(property_rule.name.clone(), initial_value.clone());
        }
      }
    }
  }

  Cow::Owned(adjusted_parent)
}

pub(super) fn pseudo_computed_style(
  parent_context: &RenderContext,
  pseudo_matched: &MatchedDeclarationsView<'_>,
) -> (ComputedStyle, SizingContext, Color) {
  let style_layers = build_style_layers(
    NodeStyleLayers::default(),
    pseudo_matched,
    parent_context.sizing.viewport,
  );
  let inherited_parent = registered_custom_property_parent_style(
    &parent_context.style,
    std::slice::from_ref(parent_context.stylesheet.as_ref()),
    parent_context.sizing.viewport,
  );
  let mut style = style_layers.inherit(&inherited_parent);

  let font_size = style
    .font_size
    .to_px(&parent_context.sizing, parent_context.sizing.font_size);
  let normal_basis = resolve_normal_line_height(parent_context, &style, font_size);
  let line_height = style
    .line_height
    .to_px(&parent_context.sizing, normal_basis);
  let sizing = parent_context.sizing.with_font_metrics(
    font_size,
    parent_context.sizing.root_font_size,
    line_height,
    parent_context.sizing.root_line_height,
  );
  let current_color = style.color.resolve(parent_context.current_color);
  style.make_computed(&sizing);
  (style, sizing, current_color)
}

fn push_layout_node<'r>(
  nodes: &mut Vec<LayoutNodeState>,
  render_nodes: &mut Vec<&'r RenderNode>,
  render_root: &'r RenderNode,
) -> TaffyNodeId {
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

    nodes.push(LayoutNodeState {
      style: render_node
        .layout_style_override
        .clone()
        .unwrap_or_else(|| {
          render_node
            .context
            .style
            .to_taffy_style(&render_node.context.sizing)
        }),
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

impl<'r> LayoutTree<'r> {
  /// Builds a layout tree from a render-node root.
  pub fn from_render_node(render_root: &'r RenderNode) -> Self {
    let mut nodes = Vec::with_capacity(1);
    let mut render_nodes = Vec::with_capacity(1);
    let root_id = push_layout_node(&mut nodes, &mut render_nodes, render_root);

    debug_assert_eq!(root_id, TaffyNodeId::from(0usize));

    Self {
      nodes,
      render_nodes,
    }
  }

  /// Computes and rounds the layout for the whole tree.
  pub fn compute_layout(&mut self, available_space: Size<AvailableSpace>) {
    let root_node_id = NodeId::ROOT.into_taffy();
    compute_root_layout(
      self,
      root_node_id,
      available_space.map(AvailableSpace::into_taffy).into_taffy(),
    );
    snap_layout(self, root_node_id, 0.0, 0.0);
  }

  /// Consumes the tree into immutable per-node layout results.
  pub fn into_results(self) -> LayoutResults {
    LayoutResults {
      nodes: self
        .nodes
        .into_iter()
        .map(|node| LayoutResultNode {
          layout: node.final_layout,
          first_baseline_y: node.first_baseline_y,
          box_children: node.box_children,
        })
        .collect(),
    }
  }

  fn get_index(&self, node_id: TaffyNodeId) -> Option<usize> {
    let idx = node_id.into();
    (idx < self.nodes.len()).then_some(idx)
  }

  fn get_layout_node_ref(&self, node_id: TaffyNodeId) -> Option<&LayoutNodeState> {
    self.get_index(node_id).and_then(|idx| self.nodes.get(idx))
  }

  fn get_layout_node_mut_ref(&mut self, node_id: TaffyNodeId) -> Option<&mut LayoutNodeState> {
    self
      .get_index(node_id)
      .and_then(|idx| self.nodes.get_mut(idx))
  }

  fn update_node_style_for_available_space(
    &mut self,
    node_id: TaffyNodeId,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) {
    let Some(idx) = self.get_index(node_id) else {
      return;
    };

    let Some(render_node) = self.render_nodes.get(idx) else {
      return;
    };

    let style = if let Some(style_override) = &render_node.layout_style_override {
      style_override.clone()
    } else {
      let mut sizing = render_node.context.sizing.clone();
      sizing.container_size = Size {
        width: known_dimensions.width.or(match available_space.width {
          AvailableSpace::Definite(value) => Some(value),
          _ => None,
        }),
        height: known_dimensions.height.or(match available_space.height {
          AvailableSpace::Definite(value) => Some(value),
          _ => None,
        }),
      };

      render_node.context.style.to_taffy_style(&sizing)
    };

    if let Some(node) = self.nodes.get_mut(idx) {
      node.style = style;
    }
  }
}

// Taffy may inject a flex stretch-derived cross-size into leaf `known_dimensions`
// during intrinsic single-axis sizing (`ComputeSize` with `InherentSize` or `ContentSize`). For replaced
// elements, letting that value participate in aspect-ratio transfer can
// incorrectly inflate the measured main-size. Strip that hint at the leaf boundary.
fn should_strip_flex_intrinsic_stretch_known_dimension(
  render_node: &RenderNode,
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

  let Some(node) = render_node.node.as_ref() else {
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

fn sort_children_by_order(
  children: &[TaffyNodeId],
  mut child_order: impl FnMut(TaffyNodeId) -> i32,
) -> Vec<TaffyNodeId> {
  let mut ordered = children
    .iter()
    .copied()
    .enumerate()
    .map(|(source_index, child_id)| (source_index, child_id, child_order(child_id)))
    .collect::<Vec<_>>();
  ordered.sort_by(|left, right| left.2.cmp(&right.2).then_with(|| left.0.cmp(&right.0)));
  ordered
    .into_iter()
    .map(|(_, child_id, _)| child_id)
    .collect()
}

impl TraversePartialTree for LayoutTree<'_> {
  type ChildIter<'a>
    = IntoIter<TaffyNodeId>
  where
    Self: 'a;

  fn child_ids(&self, parent_node_id: TaffyNodeId) -> Self::ChildIter<'_> {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return Vec::new().into_iter();
    };

    let children = if matches!(node.style.display, TaffyDisplay::Flex | TaffyDisplay::Grid) {
      sort_children_by_order(&node.children, |child_id| {
        let child_idx: usize = child_id.into();
        self
          .render_nodes
          .get(child_idx)
          .map_or(0, |child| child.context.style.order.0)
      })
    } else {
      node.children.to_vec()
    };

    children.into_iter()
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

    if matches!(node.style.display, TaffyDisplay::Flex | TaffyDisplay::Grid) {
      let mut ordered_children = self.child_ids(parent_node_id);
      return ordered_children
        .nth(child_index)
        .unwrap_or_else(|| TaffyNodeId::from(0usize));
    }

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
        (TaffyDisplay::Flex, true) => compute_flexbox_layout(tree, node, inputs),
        (TaffyDisplay::Grid, true) => compute_grid_layout(tree, node, inputs),
        (_, false) => {
          let idx: usize = node.into();
          let Some(render_node) = tree.render_nodes.get(idx) else {
            return compute_hidden_layout(tree, node);
          };

          let stripped_known_dimensions = |known_dimensions: TaffySize<Option<f32>>| {
            if should_strip_flex_intrinsic_stretch_known_dimension(
              render_node,
              inputs,
              Size::from_taffy(known_dimensions),
            ) {
              TaffySize::NONE
            } else {
              known_dimensions
            }
          };

          compute_leaf_layout(
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
          )
        }
      }
    });

    if let Some(node_data) = self.get_layout_node_mut_ref(node) {
      node_data.first_baseline_y = output.first_baselines.y;
    }

    output
  }
}

impl CacheTree for LayoutTree<'_> {
  fn cache_get(&self, node_id: TaffyNodeId, input: &LayoutInput) -> Option<LayoutOutput> {
    let node = self.get_layout_node_ref(node_id)?;
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

/// Snaps every box to whole pixels, both edges in absolute space so a box
/// always meets the one beside it. taffy's `round_layout` documents the same
/// rule but rounds a location against its parent, which parts two siblings by a
/// pixel whenever their parent sits on a fraction.
/// Blink snaps the same way, against the absolute offset's fraction
/// (`SnapSizeToPixel`, platform/geometry/layout_unit.h).
fn snap_layout(tree: &mut LayoutTree<'_>, node_id: TaffyNodeId, parent_x: f32, parent_y: f32) {
  let unrounded = tree.get_unrounded_layout(node_id);
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

  tree.set_final_layout(node_id, &layout);

  for index in 0..tree.child_count(node_id) {
    let child = tree.get_child_id(node_id, index);

    snap_layout(tree, child, x, y);
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
  fn anonymous_box_context(parent_context: &RenderContext) -> RenderContext {
    let mut context = parent_context.clone();
    context.style.display = Display::Block;
    context.style.opacity = PercentageNumber(1.0);
    context.style.filter = Filters::default();
    context.style.backdrop_filter = Filters::default();
    context.style.mix_blend_mode = BlendMode::Normal;
    context.style.isolation = Isolation::Auto;
    context.style.clip_path = None;
    context.style.mask_image = None;
    context.style.mask_size = Default::default();
    context.style.mask_position = Default::default();
    context.style.mask_repeat = Default::default();
    context.style.transform = None;
    context.style.rotate = None;
    context.style.scale = Default::default();
    context.style.translate = Default::default();
    context.style.break_before = Default::default();
    context.style.break_after = Default::default();
    context.style.break_inside = Default::default();
    context
  }

  pub(super) fn anonymous_text_item(parent_context: &RenderContext, text: String) -> Self {
    Self::text_item(Self::anonymous_box_context(parent_context), text)
  }

  fn text_item(context: RenderContext, text: String) -> Self {
    Self {
      context,
      node: None,
      origin: NodeOrigin::Anonymous,
      children: None,
      layout_style_override: Some(Style {
        display: TaffyDisplay::Block,
        ..Style::default()
      }),
      anonymous_text_content: Some(text),
      marker: None,
      force_inline_layout: true,
      table_header_lines: None,
    }
  }

  fn anonymous_block_container(parent_context: &RenderContext, children: Vec<RenderNode>) -> Self {
    Self {
      context: Self::anonymous_box_context(parent_context),
      node: None,
      origin: NodeOrigin::Anonymous,
      children: Some(children.into_boxed_slice()),
      layout_style_override: Some(Style {
        display: TaffyDisplay::Block,
        ..Style::default()
      }),
      anonymous_text_content: None,
      marker: None,
      force_inline_layout: false,
      table_header_lines: None,
    }
  }

  pub(super) fn anonymous_image_item(
    parent_context: &RenderContext,
    image: BackgroundImage,
  ) -> Self {
    // Cap image content to the parent pseudo's box so explicit `width` / `height`
    // on the pseudo wins over intrinsic / default sizing.
    let max_size = TaffySize {
      width: taffy::Dimension::percent(1.0),
      height: taffy::Dimension::percent(1.0),
    };

    match image {
      BackgroundImage::Url(url) => Self {
        context: Self::anonymous_box_context(parent_context),
        node: Some(Node::image(url)),
        origin: NodeOrigin::Anonymous,
        children: None,
        layout_style_override: Some(Style {
          max_size,
          ..Style::default()
        }),
        anonymous_text_content: None,
        marker: None,
        force_inline_layout: false,
        table_header_lines: None,
      },
      gradient => {
        let mut context = Self::anonymous_box_context(parent_context);
        context.style.background_image = Some(BackgroundImages::from([gradient]));
        Self {
          context,
          node: Some(Node::container([])),
          origin: NodeOrigin::Anonymous,
          children: None,
          // css-images-3 §5.1 default object size when the parent is auto.
          layout_style_override: Some(Style {
            size: TaffySize {
              width: taffy::Dimension::length(300.0),
              height: taffy::Dimension::length(150.0),
            },
            max_size,
            ..Style::default()
          }),
          anonymous_text_content: None,
          marker: None,
          force_inline_layout: false,
          table_header_lines: None,
        }
      }
    }
  }

  /// An element's own text, moved into a child so generated content can precede
  /// it. Text decorations do not inherit, so they are propagated the way CSS
  /// propagates them to an anonymous descendant.
  fn generated_sibling_text(parent_context: &RenderContext, text: String) -> Self {
    let (mut style, sizing, current_color) =
      pseudo_computed_style(parent_context, &MatchedDeclarationsView::default());
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
    let (mut style, sizing, current_color) = pseudo_computed_style(parent_context, pseudo_matched);

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

    let items = match std::mem::take(&mut style.content) {
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

    Some(Self {
      context: pseudo_context,
      node: Some(Node::container([])),
      origin: NodeOrigin::Pseudo,
      children: Some(children),
      layout_style_override: None,
      anonymous_text_content: None,
      marker: None,
      force_inline_layout: false,
      table_header_lines: None,
    })
  }

  /// The block box the marker travels into when this box has no line of its
  /// own. A list item is left alone: its own marker holds that line already.
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

  /// Whether this box, or the block chain below it, ends in a line the marker
  /// can share.
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

  /// Resolves the descendant at `path` (child indices from this node). An empty
  /// path returns `self`.
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
  pub fn participates_as_inline_box(&self) -> bool {
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
        Display::Block | Display::InlineBlock | Display::ListItem
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
      &parent_context.stylesheet,
      parent_context.sizing.viewport,
    );
    let mut tree = Self::from_node_iterative(parent_context, node, &matched_styles);

    lower_tables(&mut tree);

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

    fn next_source_order(source_cursor: &mut usize) -> usize {
      let source_order = *source_cursor;
      *source_cursor += 1;
      source_order
    }

    fn take_children_vec(node: &mut Node) -> (bool, Vec<Node>) {
      let children = node.take_children();
      let children_is_some = children.is_some();
      let children = children.map_or_else(Vec::new, <[Node]>::into_vec);
      (children_is_some, children)
    }

    fn resolve_computed_style(
      parent_context: &RenderContext,
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

      let style_layers = build_style_layers(layers, matched, parent_context.sizing.viewport);
      let inherited_parent = registered_custom_property_parent_style(
        &parent_context.style,
        std::slice::from_ref(parent_context.stylesheet.as_ref()),
        parent_context.sizing.viewport,
      );

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
      let parent_root_font_size = parent_context.sizing.root_font_size;
      let parent_root_line_height = parent_context.sizing.root_line_height;

      let mut child_sizing_for_final: Option<SizingContext> = None;
      if !style.animation_name.is_empty() {
        let font_size = style
          .font_size
          .to_px(&parent_context.sizing, parent_context.sizing.font_size);
        let normal_basis = resolve_normal_line_height(parent_context, &style, font_size);
        let line_height = style
          .line_height
          .to_px(&parent_context.sizing, normal_basis);
        let child_sizing = parent_context.sizing.with_font_metrics(
          font_size,
          parent_root_font_size,
          line_height,
          parent_root_line_height,
        );
        let child_current_color = style.color.resolve(parent_context.current_color);
        let child_context = RenderContext::from_parent(
          parent_context,
          style.clone(),
          child_sizing.clone(),
          child_current_color,
        );
        style = apply_stylesheet_animations(
          style,
          &child_context.stylesheet,
          child_context.time_ms,
          &child_context.sizing,
          child_context.current_color,
        );
        child_sizing_for_final = Some(child_sizing);
      }

      for &declarations in matched.important() {
        for declaration in declarations.iter() {
          declaration.apply_to_computed(&mut style);
        }
      }

      let sizing_basis = child_sizing_for_final.unwrap_or_else(|| parent_context.sizing.clone());
      let font_size = style.font_size.to_px(&sizing_basis, sizing_basis.font_size);
      let normal_basis = resolve_normal_line_height(parent_context, &style, font_size);
      let line_height = style
        .line_height
        .to_px(&parent_context.sizing, normal_basis);
      let sizing = parent_context.sizing.with_font_metrics(
        font_size,
        parent_root_font_size.or_else(|| is_document_root.then_some(font_size)),
        line_height,
        parent_root_line_height.or_else(|| is_document_root.then_some(line_height)),
      );
      let current_color = style.color.resolve(parent_context.current_color);
      style.make_computed(&sizing);
      (style, sizing, current_color)
    }

    fn build_pending_node(
      parent_context: &RenderContext,
      mut node: Node,
      matched_declarations: &[NodeMatchedDeclarations<'_>],
      source_cursor: &mut usize,
      counter: &mut ListCounter,
      inside_list: bool,
    ) -> PendingRenderNode {
      let source_order = next_source_order(source_cursor);
      let (style, sizing, current_color) = resolve_computed_style(
        parent_context,
        &mut node,
        source_order,
        matched_declarations,
      );
      let (children_is_some, children) = take_children_vec(&mut node);
      let context = RenderContext::from_parent(parent_context, style, sizing, current_color);

      let element_matched = matched_declarations.get(source_order);
      let marker_ordinal =
        (context.style.display == Display::ListItem).then(|| counter.take(&node));
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

      PendingRenderNode {
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

    let mut source_cursor = 0;
    let mut root_counter = ListCounter::new(&root);
    let mut stack = vec![build_pending_node(
      parent_context,
      root,
      matched_declarations,
      &mut source_cursor,
      &mut root_counter,
      false,
    )];

    loop {
      let Some(current) = stack.last_mut() else {
        return RenderNode {
          context: parent_context.clone(),
          node: Some(Node::container([])),
          origin: NodeOrigin::Anonymous,
          children: None,
          layout_style_override: None,
          anonymous_text_content: None,
          marker: None,
          force_inline_layout: false,
          table_header_lines: None,
        };
      };

      if let Some(child) = current.pending_children.next() {
        let child_pending = build_pending_node(
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

      let Some(mut finished) = stack.pop() else {
        return RenderNode {
          context: parent_context.clone(),
          node: Some(Node::container([])),
          origin: NodeOrigin::Anonymous,
          children: None,
          layout_style_override: None,
          anonymous_text_content: None,
          marker: None,
          force_inline_layout: false,
          table_header_lines: None,
        };
      };

      if !finished.owns_list_counter
        && let Some(parent) = stack.last_mut()
      {
        parent.list_counter = finished.list_counter;
      }

      if let Some(after) = finished.pseudo_after.take() {
        finished.rendered_children.push(after);
      }

      let children = if finished.children_is_some {
        Some(finished.rendered_children.into_boxed_slice())
      } else {
        None
      };

      let marker_ordinal = finished.marker_ordinal;
      let mut render_node = if let Some(mut children) = children {
        if finished.context.style.display.should_blockify_children() {
          // CSS Flexbox L1 §4 / Grid L1 §6: collapsible whitespace-only text
          // between items is not rendered; every remaining child blockifies.
          let mut children = Vec::from(children);
          children.retain(|child| !child.is_collapsible_whitespace_only_text_node());
          for child in &mut children {
            child.context.style.display.blockify();
          }

          RenderNode {
            context: finished.context,
            node: Some(finished.node),
            origin: NodeOrigin::Authored {
              source_order: finished.source_order,
            },
            children: Some(children.into_boxed_slice()),
            layout_style_override: None,
            anonymous_text_content: None,
            marker: None,
            force_inline_layout: false,
            table_header_lines: None,
          }
        } else {
          // Blink's Text::TextLayoutObjectIsNeeded: collapsible
          // whitespace-only text renders only after an in-flow inline-level
          // sibling, and leading whitespace only inside an inline parent
          // (#711, #992).
          children = drop_collapsible_boundary_whitespace(
            Vec::from(children),
            finished.context.style.display.is_inline(),
          )
          .into_boxed_slice();

          // https://github.com/kane50613/takumi/issues/738: out-of-flow boxes
          // must not be swept into an anonymous block box.
          let has_inline = children.iter().any(|child| {
            child.participates_in_inline_formatting_context() && !child.is_out_of_flow()
          });
          let has_block = children
            .iter()
            .any(|child| !child.participates_in_inline_formatting_context());
          let has_out_of_flow = children.iter().any(RenderNode::is_out_of_flow);
          let parent_is_inline = finished.context.style.display.is_inline();
          let requires_inline_parent_blockification = parent_is_inline && has_block;
          // A block parent mixing inline content with out-of-flow children wraps
          // the inline part so the absolute boxes stay as block-level children.
          // An inline parent keeps its inline formatting context untouched — an
          // anonymous block there would be dropped by the surrounding line box.
          let needs_anonymous_boxes =
            has_inline && (has_block || (!parent_is_inline && has_out_of_flow));

          if requires_inline_parent_blockification {
            finished.context.style.display = finished.context.style.display.as_blockified();
          }

          if !needs_anonymous_boxes {
            RenderNode {
              context: finished.context,
              node: Some(finished.node),
              origin: NodeOrigin::Authored {
                source_order: finished.source_order,
              },
              children: Some(children),
              layout_style_override: None,
              anonymous_text_content: None,
              marker: None,
              force_inline_layout: false,
              table_header_lines: None,
            }
          } else {
            let mut final_children = Vec::new();
            let mut inline_group = Vec::new();

            for item in children {
              if item.participates_in_inline_formatting_context() && !item.is_out_of_flow() {
                inline_group.push(item);
                continue;
              }

              flush_inline_group(&mut inline_group, &mut final_children, &finished.context);

              final_children.push(item);
            }

            flush_inline_group(&mut inline_group, &mut final_children, &finished.context);

            RenderNode {
              context: finished.context,
              node: Some(finished.node),
              origin: NodeOrigin::Authored {
                source_order: finished.source_order,
              },
              children: Some(final_children.into_boxed_slice()),
              layout_style_override: None,
              anonymous_text_content: None,
              marker: None,
              force_inline_layout: false,
              table_header_lines: None,
            }
          }
        }
      } else {
        let maybe_anonymous_text = if finished.context.style.display.should_blockify_children() {
          finished
            .node
            .inline_content()
            .and_then(|content| match content {
              InlineContentKind::Text(text) => Some(text.into_owned()),
              InlineContentKind::Box => None,
            })
        } else {
          None
        };

        if let Some(text) = maybe_anonymous_text {
          let anonymous_text_item = RenderNode::anonymous_text_item(&finished.context, text);
          RenderNode {
            context: finished.context,
            node: Some(finished.node),
            origin: NodeOrigin::Authored {
              source_order: finished.source_order,
            },
            children: Some([anonymous_text_item].into()),
            layout_style_override: None,
            anonymous_text_content: None,
            marker: None,
            force_inline_layout: false,
            table_header_lines: None,
          }
        } else {
          RenderNode {
            context: finished.context,
            node: Some(finished.node),
            origin: NodeOrigin::Authored {
              source_order: finished.source_order,
            },
            children: None,
            layout_style_override: None,
            anonymous_text_content: None,
            marker: None,
            force_inline_layout: false,
            table_header_lines: None,
          }
        }
      };

      if let Some(ordinal) = marker_ordinal
        && let Some(marker) = list_marker(&render_node.context, ordinal)
      {
        attach_marker(&mut render_node, marker);
      }

      if let Some(parent) = stack.last_mut() {
        parent.rendered_children.push(render_node);
      } else {
        return render_node;
      }
    }
  }

  fn inline_box_margin_box_height(
    &self,
    content_size: Size<f32>,
    include_padding_border: bool,
  ) -> f32 {
    let sizing = &self.context.sizing;
    let mut height = content_size.height
      + self.context.style.margin_top.to_px(sizing, 0.0)
      + self.context.style.margin_bottom.to_px(sizing, 0.0);

    if include_padding_border {
      height += Length::from(self.context.style.border_top_width).to_px(sizing, 0.0)
        + Length::from(self.context.style.border_bottom_width).to_px(sizing, 0.0)
        + self.context.style.padding_top.to_px(sizing, 0.0)
        + self.context.style.padding_bottom.to_px(sizing, 0.0);
    }

    height
  }

  fn inline_replaced_content_size(
    &self,
    measured_size: Size<f32>,
    layout_style: &Style,
  ) -> Size<f32> {
    if self.context.style.box_sizing != BoxSizing::BorderBox {
      return measured_size;
    }

    let sizing = &self.context.sizing;
    let horizontal_insets = self.context.style.padding_left.to_px(sizing, 0.0)
      + self.context.style.padding_right.to_px(sizing, 0.0)
      + if !self.context.style.border_left_style.is_rendered() {
        0.0
      } else {
        Length::from(self.context.style.border_left_width).to_px(sizing, 0.0)
      }
      + if !self.context.style.border_right_style.is_rendered() {
        0.0
      } else {
        Length::from(self.context.style.border_right_width).to_px(sizing, 0.0)
      };
    let vertical_insets = self.context.style.padding_top.to_px(sizing, 0.0)
      + self.context.style.padding_bottom.to_px(sizing, 0.0)
      + if !self.context.style.border_top_style.is_rendered() {
        0.0
      } else {
        Length::from(self.context.style.border_top_width).to_px(sizing, 0.0)
      }
      + if !self.context.style.border_bottom_style.is_rendered() {
        0.0
      } else {
        Length::from(self.context.style.border_bottom_width).to_px(sizing, 0.0)
      };

    let width_auto = layout_style.size.width.is_auto();
    let height_auto = layout_style.size.height.is_auto();
    let measured_ratio = if measured_size.width > 0.0 && measured_size.height > 0.0 {
      Some(measured_size.width / measured_size.height)
    } else {
      None
    };

    match (width_auto, height_auto) {
      (false, false) => Size {
        width: (measured_size.width - horizontal_insets).max(0.0),
        height: (measured_size.height - vertical_insets).max(0.0),
      },
      (false, true) => {
        let width = (measured_size.width - horizontal_insets).max(0.0);
        let height = measured_ratio
          .filter(|ratio| *ratio > 0.0)
          .map_or(measured_size.height, |ratio| width / ratio);
        Size { width, height }
      }
      (true, false) => {
        let height = (measured_size.height - vertical_insets).max(0.0);
        let width = measured_ratio
          .filter(|ratio| *ratio > 0.0)
          .map_or(measured_size.width, |ratio| height * ratio);
        Size { width, height }
      }
      (true, true) => measured_size,
    }
  }

  fn inline_baseline_box_kind(&self) -> Option<InlineBaselineBoxKind> {
    if self.participates_as_inline_box() {
      return Some(InlineBaselineBoxKind::AtomicContainer);
    }

    self
      .node
      .as_ref()
      .filter(|node| node.is_replaced_element() && self.context.style.display == Display::Inline)
      .map(|_| InlineBaselineBoxKind::Replaced)
  }

  fn inline_content_baseline_offset(
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

    let font_style = SizedFontStyle::from_style(&self.context.style, &self.context);
    let max_width = size.width.max(0.0);
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
    let line = if use_last_line {
      built.layout.lines().last()?
    } else {
      built.layout.lines().next()?
    };
    let metrics = line.metrics();
    let sizing = &self.context.sizing;
    let margin_top = self.context.style.margin_top.to_px(sizing, 0.0);
    let border_top = Length::from(self.context.style.border_top_width).to_px(sizing, 0.0);
    let padding_top = self.context.style.padding_top.to_px(sizing, 0.0);
    Some(margin_top + border_top + padding_top + metrics.baseline)
  }

  fn layout_first_baseline_offset(
    &self,
    layout_results: &LayoutResults,
    root_node_id: NodeId,
  ) -> Option<f32> {
    let baseline = layout_results
      .first_baseline_y(root_node_id)
      .ok()
      .flatten()?;
    let sizing = &self.context.sizing;
    let margin_top = self.context.style.margin_top.to_px(sizing, 0.0);

    Some(margin_top + baseline)
  }

  fn valid_baseline_offset(candidate: Option<f32>, box_height: f32) -> Option<f32> {
    candidate
      .filter(|baseline| baseline.is_finite() && *baseline >= 0.0 && *baseline <= box_height + 0.5)
  }

  fn inline_baseline_strategy(&self) -> Option<InlineBaselineStrategy> {
    match self.inline_baseline_box_kind()? {
      InlineBaselineBoxKind::AtomicContainer => {
        let display = self.context.style.display;
        let overflow_hidden_inline_block =
          display == Display::InlineBlock && self.context.style.clips_overflow();

        Some(match display {
          Display::InlineBlock if overflow_hidden_inline_block => InlineBaselineStrategy {
            sources: &[],
            fallback: InlineBaselineFallback::BottomMarginEdge,
          },
          Display::InlineBlock => InlineBaselineStrategy {
            sources: &[
              InlineBaselineSource::InlineContentLastLine,
              InlineBaselineSource::LayoutFirstBaseline,
            ],
            fallback: InlineBaselineFallback::BottomMarginEdge,
          },
          Display::InlineFlex | Display::InlineGrid => InlineBaselineStrategy {
            sources: &[
              InlineBaselineSource::InlineContentLastLine,
              InlineBaselineSource::InlineContentFirstLine,
              InlineBaselineSource::LayoutFirstBaseline,
            ],
            fallback: InlineBaselineFallback::BottomMarginEdge,
          },
          _ => InlineBaselineStrategy {
            sources: &[],
            fallback: InlineBaselineFallback::BottomMarginEdge,
          },
        })
      }
      InlineBaselineBoxKind::Replaced => Some(InlineBaselineStrategy {
        sources: &[],
        fallback: InlineBaselineFallback::BottomMarginEdge,
      }),
    }
  }

  fn resolve_inline_baseline_source(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    source: InlineBaselineSource,
    layout_results: Option<(&LayoutResults, NodeId)>,
  ) -> Option<f32> {
    match source {
      InlineBaselineSource::InlineContentLastLine => {
        self.inline_content_baseline_offset(available_space, size, true)
      }
      InlineBaselineSource::InlineContentFirstLine => {
        self.inline_content_baseline_offset(available_space, size, false)
      }
      InlineBaselineSource::LayoutFirstBaseline => {
        layout_results.and_then(|(results, root_node_id)| {
          self.layout_first_baseline_offset(results, root_node_id)
        })
      }
    }
  }

  fn resolve_inline_baseline_offset(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    layout_results: Option<(&LayoutResults, NodeId)>,
  ) -> Option<f32> {
    let strategy = self.inline_baseline_strategy()?;
    let include_padding_border = !self.participates_as_inline_box();
    let margin_box_height = self.inline_box_margin_box_height(size, include_padding_border);

    for source in strategy.sources {
      let candidate =
        self.resolve_inline_baseline_source(available_space, size, *source, layout_results);

      if let Some(baseline) = Self::valid_baseline_offset(candidate, margin_box_height) {
        return Some(baseline);
      }
    }

    match strategy.fallback {
      InlineBaselineFallback::BottomMarginEdge => None,
    }
  }

  pub(crate) fn measure_inline_box(
    &self,
    available_space: Size<AvailableSpace>,
  ) -> AtomicInlineMetrics {
    if self.participates_as_inline_box() {
      return self.measure_atomic_subtree(available_space);
    }

    let Some(node) = &self.node else {
      return AtomicInlineMetrics {
        size: Size::ZERO,
        baseline_offset: None,
      };
    };

    let layout_style = self
      .layout_style_override
      .as_ref()
      .cloned()
      .unwrap_or_else(|| self.context.style.to_taffy_style(&self.context.sizing));
    let measured_size = node.measure(&self.context, available_space, Size::NONE, &layout_style);
    let size = self.inline_replaced_content_size(measured_size, &layout_style);

    AtomicInlineMetrics {
      size,
      baseline_offset: self.resolve_inline_baseline_offset(available_space, size, None),
    }
  }

  pub(crate) fn measure_atomic_subtree(
    &self,
    available_space: Size<AvailableSpace>,
  ) -> AtomicInlineMetrics {
    let measure_with = |width: AvailableSpace| {
      let mut tree = LayoutTree::from_render_node(self);
      tree.compute_layout(Size {
        width,
        height: available_space.height,
      });
      let results = tree.into_results();

      results
        .layout(NodeId::ROOT)
        .map_or(Size::ZERO, |layout| layout.size)
    };

    if self.participates_as_inline_box() {
      // CSS shrink-to-fit for inline-level atomic boxes:
      // width = min(max-content, max(min-content, available)).
      // Reference: https://www.w3.org/TR/CSS22/visudet.html#float-width
      let min_content = measure_with(AvailableSpace::MinContent);
      let max_content = {
        let mut tree = LayoutTree::from_render_node(self);
        // Hack: Use Flexbox to avoid Block's "expand to fill" behavior when calculating max-content.
        // We want the content's preferred width, not the container's available width.
        if let Some(node) = tree.get_layout_node_mut_ref(TaffyNodeId::from(0usize))
          && node.style.display == TaffyDisplay::Block
        {
          node.style.display = TaffyDisplay::Flex;
          node.style.flex_direction = taffy::FlexDirection::Row;
          node.style.justify_content = Some(taffy::JustifyContent::START);
        }

        tree.compute_layout(Size {
          width: AvailableSpace::MaxContent,
          height: available_space.height,
        });

        let results = tree.into_results();
        results
          .layout(NodeId::ROOT)
          .map_or(Size::ZERO, |layout| layout.size)
      };

      let used_width = match available_space.width {
        AvailableSpace::Definite(available) => {
          max_content.width.min(min_content.width.max(available))
        }
        AvailableSpace::MinContent => min_content.width,
        AvailableSpace::MaxContent => max_content.width,
      };
      let mut tree = LayoutTree::from_render_node(self);
      tree.compute_layout(Size {
        width: AvailableSpace::Definite(used_width),
        height: available_space.height,
      });
      let results = tree.into_results();
      let root_node_id = NodeId::ROOT;

      return results.layout(root_node_id).map_or(
        AtomicInlineMetrics {
          size: Size::ZERO,
          baseline_offset: None,
        },
        |layout| {
          let size = layout.size;
          let baseline_offset = self.resolve_inline_baseline_offset(
            available_space,
            size,
            Some((&results, root_node_id)),
          );
          AtomicInlineMetrics {
            size,
            baseline_offset,
          }
        },
      );
    }

    let size = measure_with(available_space.width);
    AtomicInlineMetrics {
      size,
      baseline_offset: None,
    }
  }

  pub(crate) fn measure(
    &self,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &Style,
    is_inline_children: bool,
  ) -> Size<f32> {
    if is_inline_children {
      let (max_width, max_height) =
        create_inline_constraint(&self.context, available_space, known_dimensions);

      let font_style = SizedFontStyle::from_style(&self.context.style, &self.context);

      let mut built = create_inline_layout(InlineLayoutRequest {
        items: collect_inline_items(self),
        available_space,
        max_width,
        max_height,
        style: &font_style,
        context: &self.context,
        mode: InlineLayoutMode::Measure,
        shape_cacheable: true,
      });

      let ceil_width = font_style.parent.resolved_text_wrap_mode() == TextWrapMode::Wrap;
      let parent_font_metrics = built.parent_font_metrics();
      return measure_inline_layout(
        &mut built.layout,
        &built.spans,
        &built.custom_inline_boxes,
        &built.line_scales,
        InlineMeasureOptions {
          max_width,
          ceil_width,
          parent_font_metrics,
        },
      );
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

/// Blink positions an outside marker against the item's first line box, so the
/// marker goes on the box that establishes that line, however deep it sits. An
/// inside marker is the item's own content and stays on the item.
fn attach_marker(node: &mut RenderNode, marker: RenderNode) {
  if node.should_create_inline_layout() {
    node.marker = Some(Box::new(marker));
    return;
  }

  if marker.context.style.list_style_position == ListStylePosition::Outside
    && let Some(block) = node.marker_host_child()
  {
    attach_marker(block, marker);
    return;
  }

  let has_block_content = node.children.as_deref().is_some_and(|children| {
    children
      .iter()
      .any(|child| !child.participates_in_inline_formatting_context())
  });

  if !has_block_content {
    // Text of its own, or nothing at all: the marker shares that line.
    node.force_inline_layout = true;
    node.marker = Some(Box::new(marker));
    return;
  }

  // Block-level content the marker may not join, so it gets a line of its own.
  let mut line = RenderNode::anonymous_block_container(&node.context, Vec::new());
  line.force_inline_layout = true;
  line.marker = Some(Box::new(marker));

  let mut children = Vec::from(node.children.take().unwrap_or_default());
  children.insert(0, line);
  node.children = Some(children.into_boxed_slice());
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

#[cfg(test)]
mod tests {
  use std::{str::FromStr, sync::Arc};

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
    let leaf = |children: Option<Box<[RenderNode]>>| RenderNode {
      context: context.clone(),
      node: None,
      origin: NodeOrigin::Anonymous,
      children,
      layout_style_override: None,
      anonymous_text_content: None,
      marker: None,
      force_inline_layout: false,
      table_header_lines: None,
    };

    let mut root = leaf(None);
    for _ in 0..500_000 {
      root = leaf(Some(Box::new([root])));
    }

    drop(root);
  }

  #[test]
  fn sort_children_by_order_keeps_source_order_for_equal_values() {
    let children = vec![
      TaffyNodeId::from(3usize),
      TaffyNodeId::from(1usize),
      TaffyNodeId::from(2usize),
    ];
    let sorted = sort_children_by_order(&children, |child_id| match usize::from(child_id) {
      1 => -1,
      _ => 0,
    });
    assert_eq!(
      sorted,
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
    Arc::make_mut(&mut parent.custom_properties).insert("--box-size".to_owned(), "50px".to_owned());

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
      Some(&"10px".to_owned())
    );
  }

  #[test]
  fn registered_custom_property_preserves_parent_value_when_inheriting() {
    let mut parent = ComputedStyle::default();
    Arc::make_mut(&mut parent.custom_properties).insert("--box-size".to_owned(), "50px".to_owned());

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
      Some(&"50px".to_owned())
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
      Some(&"10px".to_owned())
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
      Some(&"20px".to_owned())
    );
  }

  #[test]
  fn registered_custom_property_later_inheriting_rule_restores_parent_value() {
    let mut parent = ComputedStyle::default();
    Arc::make_mut(&mut parent.custom_properties).insert("--box-size".to_owned(), "50px".to_owned());

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
      Some(&"50px".to_owned())
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
    assert_eq!(
      resolved.custom_properties.get("--box-size"),
      Some(&"red".to_owned()) // syntax validation is skipped, so any value is accepted
    );
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
    assert_eq!(
      resolved.custom_properties.get("--box-size"),
      Some(&"24px".to_owned())
    );
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
      Some(&"var(--source)".to_owned())
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
      Some(&"auto".to_owned())
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
    assert_eq!(
      resolved.custom_properties.get("--accent"),
      Some(&"12px".to_owned())
    );
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
      Some(&"2s".to_owned())
    );
    assert_eq!(
      resolved.custom_properties.get("--move"),
      Some(&"rotate(45deg)".to_owned())
    );
    assert_eq!(
      resolved.custom_properties.get("--bg"),
      Some(&"url(hero.png)".to_owned())
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
    assert_eq!(resolved.width, Length::Px(10.0));
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
    assert_eq!(
      resolved.custom_properties.get("--move"),
      Some(&"red".to_owned()) // syntax validation is skipped, so any value is accepted
    );
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

    assert_eq!(tree.context.style.width, Length::Px(10.0));

    let children = tree.children.as_deref().expect("block children");
    assert_eq!(children[0].context.style.width, Length::Px(10.0));
    assert_eq!(children[1].context.style.width, Length::Px(20.0));
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

      child.context.style.width.to_px(&child.context.sizing, 0.0)
    }

    let content = Node::container([Node::container([]).with_id("child")]).with_id("root");
    assert_eq!(child_width(content), 16.0);

    let document = Node::container([Node::container([]).with_id("child")])
      .with_id("root")
      .with_tag_name("html");
    assert_eq!(child_width(document), 32.0);
  }
}
