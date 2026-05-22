use std::{mem::take, vec::IntoIter};

use taffy::{
  AvailableSpace, BlockContext, Cache, CacheTree, Display as TaffyDisplay, Layout,
  LayoutBlockContainer, LayoutFlexboxContainer, LayoutGridContainer, LayoutInput, LayoutOutput,
  LayoutPartialTree, NodeId, RequestedAxis, RoundTree, RunMode, Size, SizingMode, Style,
  TaffyError, TraversePartialTree, TraverseTree, compute_block_layout, compute_cached_layout,
  compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
  compute_root_layout, round_layout,
};

use crate::{
  GlobalContext, Result,
  layout::{
    Viewport,
    inline::{
      InlineContentKind, InlineLayoutMode, InlineLayoutRequest, InlineMeasureOptions,
      ProcessedInlineSpan, collect_inline_items, create_inline_constraint, create_inline_layout,
      get_parent_font_metrics, measure_inline_layout, resolve_inline_max_height,
    },
    node::{Node, NodeStyleLayers},
    style::{
      Affine, BlendMode, BoxSizing, Color, ComputedStyle, Display, Filters, Float, Isolation,
      LineHeight, Overflow, PercentageNumber, Position, Style as NodeStyle, StyleDeclaration,
      StyleSheet, TextWrapMode, apply_stylesheet_animations,
      matching::{MatchedDeclarationsView, match_stylesheets_view},
    },
  },
  rendering::{
    Canvas, RenderContext, Sizing,
    inline_drawing::{InlineLayoutDrawData, draw_inline_box, draw_inline_layout},
  },
};
use parley::fontique::Attributes;

pub(crate) struct LayoutResults {
  nodes: Vec<LayoutResultNode>,
}

struct LayoutResultNode {
  layout: Layout,
  first_baseline_y: Option<f32>,
  children: Box<[NodeId]>,
}

impl LayoutResults {
  pub(crate) const fn root_node_id(&self) -> NodeId {
    NodeId::new(0)
  }

  pub(crate) fn layout(&self, node_id: NodeId) -> std::result::Result<&Layout, TaffyError> {
    let idx: usize = node_id.into();
    self
      .nodes
      .get(idx)
      .map(|node| &node.layout)
      .ok_or(TaffyError::InvalidInputNode(node_id))
  }

  pub(crate) fn children(&self, node_id: NodeId) -> std::result::Result<&[NodeId], TaffyError> {
    let idx: usize = node_id.into();
    self
      .nodes
      .get(idx)
      .map(|node| node.children.as_ref())
      .ok_or(TaffyError::InvalidInputNode(node_id))
  }

  pub(crate) fn first_baseline_y(
    &self,
    node_id: NodeId,
  ) -> std::result::Result<Option<f32>, TaffyError> {
    let idx: usize = node_id.into();
    self
      .nodes
      .get(idx)
      .map(|node| node.first_baseline_y)
      .ok_or(TaffyError::InvalidInputNode(node_id))
  }
}

pub(crate) struct LayoutTree<'r, 'g> {
  nodes: Vec<LayoutNodeState>,
  render_nodes: Vec<&'r RenderNode<'g>>,
}

struct LayoutNodeState {
  style: Style,
  cache: Cache,
  unrounded_layout: Layout,
  final_layout: Layout,
  first_baseline_y: Option<f32>,
  is_inline_children: bool,
  children: Box<[NodeId]>,
}

#[derive(Clone)]
pub(crate) struct RenderNode<'g> {
  pub(crate) context: RenderContext<'g>,
  pub(crate) node: Option<Node>,
  pub(crate) children: Option<Box<[RenderNode<'g>]>>,
  pub(crate) layout_style_override: Option<Style>,
  pub(crate) anonymous_text_content: Option<String>,
  pub(crate) force_inline_layout: bool,
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
  global: &GlobalContext,
  style: &ComputedStyle,
  font_size: f32,
) -> f32 {
  if !matches!(style.line_height, LineHeight::Normal) {
    return 0.0;
  }
  let attributes = Attributes {
    width: style.font_stretch.into(),
    style: style.font_style.into(),
    weight: style.font_weight.into(),
  };
  global
    .font_context
    .first_font_line_spacing(style.font_family.query_families(), attributes, font_size)
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

  for &declarations in &matched_declarations.normal {
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

  for &declarations in &matched_declarations.important {
    for declaration in declarations.iter() {
      declaration.merge_into_ref(&mut style);
    }
  }

  style
}

fn registered_custom_property_parent_style(
  parent_style: &ComputedStyle,
  stylesheets: &[StyleSheet],
  viewport: Viewport,
) -> ComputedStyle {
  let mut adjusted_parent = parent_style.clone();

  for sheet in stylesheets {
    for property_rule in &sheet.property_rules {
      if !property_rule
        .media_queries
        .iter()
        .all(|media_query| media_query.matches(viewport))
      {
        continue;
      }

      adjusted_parent
        .registered_custom_properties
        .insert(property_rule.name.clone(), property_rule.clone());

      if property_rule.inherits {
        if let Some(parent_value) = parent_style.custom_properties.get(&property_rule.name) {
          adjusted_parent
            .custom_properties
            .insert(property_rule.name.clone(), parent_value.clone());
        } else if let Some(initial_value) = &property_rule.initial_value {
          adjusted_parent
            .custom_properties
            .insert(property_rule.name.clone(), initial_value.clone());
        } else {
          adjusted_parent
            .custom_properties
            .remove(&property_rule.name);
        }
      } else {
        adjusted_parent
          .custom_properties
          .remove(&property_rule.name);
        if let Some(initial_value) = &property_rule.initial_value {
          adjusted_parent
            .custom_properties
            .insert(property_rule.name.clone(), initial_value.clone());
        }
      }
    }
  }

  adjusted_parent
}

fn push_layout_node<'r, 'g>(
  nodes: &mut Vec<LayoutNodeState>,
  render_nodes: &mut Vec<&'r RenderNode<'g>>,
  render_root: &'r RenderNode<'g>,
) -> NodeId {
  struct PendingNode<'r, 'g> {
    node_id: NodeId,
    next_child_index: usize,
    children: Option<&'r [RenderNode<'g>]>,
    child_ids: Vec<NodeId>,
  }

  fn push_node_state<'r, 'g>(
    nodes: &mut Vec<LayoutNodeState>,
    render_nodes: &mut Vec<&'r RenderNode<'g>>,
    render_node: &'r RenderNode<'g>,
  ) -> PendingNode<'r, 'g> {
    let node_index = nodes.len();
    let node_id = NodeId::from(node_index);
    let is_inline_children = render_node.should_create_inline_layout();
    let children = if is_inline_children {
      None
    } else {
      render_node.children.as_deref()
    };

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
    });

    PendingNode {
      node_id,
      next_child_index: 0,
      children,
      child_ids: Vec::with_capacity(children.map_or(0, <[RenderNode<'g>]>::len)),
    }
  }

  let root = push_node_state(nodes, render_nodes, render_root);
  let root_id = root.node_id;
  let mut stack = vec![root];

  while let Some(current) = stack.last_mut() {
    let Some(children) = current.children else {
      let Some(finished) = stack.pop() else {
        break;
      };
      if let Some(parent) = stack.last_mut() {
        parent.child_ids.push(finished.node_id);
      }
      continue;
    };

    if let Some(child) = children.get(current.next_child_index) {
      current.next_child_index += 1;
      stack.push(push_node_state(nodes, render_nodes, child));
      continue;
    }

    let Some(finished) = stack.pop() else {
      break;
    };
    let node_index: usize = finished.node_id.into();
    nodes[node_index].children = finished.child_ids.into_boxed_slice();

    if let Some(parent) = stack.last_mut() {
      parent.child_ids.push(finished.node_id);
    }
  }

  root_id
}

impl<'r, 'g> LayoutTree<'r, 'g> {
  pub(crate) fn from_render_node(render_root: &'r RenderNode<'g>) -> Self {
    let mut nodes = Vec::with_capacity(1);
    let mut render_nodes = Vec::with_capacity(1);
    let root_id = push_layout_node(&mut nodes, &mut render_nodes, render_root);

    debug_assert_eq!(root_id, NodeId::from(0usize));

    Self {
      nodes,
      render_nodes,
    }
  }

  pub(crate) fn root_node_id(&self) -> NodeId {
    NodeId::from(0usize)
  }

  pub(crate) fn compute_layout(&mut self, available_space: Size<AvailableSpace>) {
    let root_node_id = self.root_node_id();
    compute_root_layout(self, root_node_id, available_space);
    round_layout(self, root_node_id);
  }

  pub(crate) fn into_results(self) -> LayoutResults {
    LayoutResults {
      nodes: self
        .nodes
        .into_iter()
        .map(|node| LayoutResultNode {
          layout: node.final_layout,
          first_baseline_y: node.first_baseline_y,
          children: node.children,
        })
        .collect(),
    }
  }

  fn get_index(&self, node_id: NodeId) -> Option<usize> {
    let idx = node_id.into();
    (idx < self.nodes.len()).then_some(idx)
  }

  fn get_layout_node_ref(&self, node_id: NodeId) -> Option<&LayoutNodeState> {
    self.get_index(node_id).and_then(|idx| self.nodes.get(idx))
  }

  fn get_layout_node_mut_ref(&mut self, node_id: NodeId) -> Option<&mut LayoutNodeState> {
    self
      .get_index(node_id)
      .and_then(|idx| self.nodes.get_mut(idx))
  }

  fn update_node_style_for_available_space(
    &mut self,
    node_id: NodeId,
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
  render_node: &RenderNode<'_>,
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
  children: &[NodeId],
  mut child_order: impl FnMut(NodeId) -> i32,
) -> Vec<NodeId> {
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

impl TraversePartialTree for LayoutTree<'_, '_> {
  type ChildIter<'a>
    = IntoIter<NodeId>
  where
    Self: 'a;

  fn child_ids(&self, parent_node_id: NodeId) -> Self::ChildIter<'_> {
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

  fn child_count(&self, parent_node_id: NodeId) -> usize {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return 0;
    };

    node.children.len()
  }

  fn get_child_id(&self, parent_node_id: NodeId, child_index: usize) -> NodeId {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return NodeId::from(0usize);
    };

    if matches!(node.style.display, TaffyDisplay::Flex | TaffyDisplay::Grid) {
      let mut ordered_children = self.child_ids(parent_node_id);
      return ordered_children
        .nth(child_index)
        .unwrap_or_else(|| NodeId::from(0usize));
    }

    node.children[child_index]
  }
}

impl TraverseTree for LayoutTree<'_, '_> {}

impl LayoutPartialTree for LayoutTree<'_, '_> {
  type CoreContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type CustomIdent = String;

  fn get_core_container_style(&self, node_id: NodeId) -> Self::CoreContainerStyle<'_> {
    if let Some(node) = self.get_layout_node_ref(node_id) {
      return &node.style;
    }
    &self.nodes[0].style
  }

  fn set_unrounded_layout(&mut self, node_id: NodeId, layout: &Layout) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.unrounded_layout = *layout;
  }

  fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
    let Some(root) = self.render_nodes.first() else {
      return 0.0;
    };

    root
      .context
      .sizing
      .calc_arena
      .resolve_calc_value(val, basis)
  }

  fn compute_child_layout(&mut self, node: NodeId, inputs: LayoutInput) -> LayoutOutput {
    self.compute_child_layout_inner(node, inputs, None)
  }
}

impl<'r, 'g> LayoutTree<'r, 'g> {
  fn compute_child_layout_inner(
    &mut self,
    node: NodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    self.update_node_style_for_available_space(
      node,
      inputs.available_space,
      inputs.known_dimensions,
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

          let stripped_known_dimensions = |known_dimensions| {
            if should_strip_flex_intrinsic_stretch_known_dimension(
              render_node,
              inputs,
              known_dimensions,
            ) {
              Size::NONE
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

              if let Size {
                width: Some(width),
                height: Some(height),
              } = known_dimensions.maybe_apply_aspect_ratio(node_data.style.aspect_ratio)
              {
                return Size { width, height };
              }

              render_node.measure(
                available_space,
                known_dimensions,
                &node_data.style,
                node_data.is_inline_children,
              )
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

impl CacheTree for LayoutTree<'_, '_> {
  fn cache_get(&self, node_id: NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
    let node = self.get_layout_node_ref(node_id)?;
    node.cache.get(input)
  }

  fn cache_store(&mut self, node_id: NodeId, input: &LayoutInput, layout_output: LayoutOutput) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.cache.store(input, layout_output);
  }

  fn cache_clear(&mut self, node_id: NodeId) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.cache.clear();
  }
}

impl LayoutBlockContainer for LayoutTree<'_, '_> {
  type BlockContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type BlockItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_block_container_style(&self, node_id: NodeId) -> Self::BlockContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_block_child_style(&self, child_node_id: NodeId) -> Self::BlockItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }

  fn compute_block_child_layout(
    &mut self,
    node: NodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    self.compute_child_layout_inner(node, inputs, block_ctx)
  }
}

impl LayoutFlexboxContainer for LayoutTree<'_, '_> {
  type FlexboxContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type FlexboxItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_flexbox_container_style(&self, node_id: NodeId) -> Self::FlexboxContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_flexbox_child_style(&self, child_node_id: NodeId) -> Self::FlexboxItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }
}

impl LayoutGridContainer for LayoutTree<'_, '_> {
  type GridContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type GridItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_grid_container_style(&self, node_id: NodeId) -> Self::GridContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_grid_child_style(&self, child_node_id: NodeId) -> Self::GridItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }
}

impl RoundTree for LayoutTree<'_, '_> {
  fn get_unrounded_layout(&self, node_id: NodeId) -> Layout {
    let Some(node) = self.get_layout_node_ref(node_id) else {
      return Layout::new();
    };

    node.unrounded_layout
  }

  fn set_final_layout(&mut self, node_id: NodeId, layout: &Layout) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    let mut final_layout = *layout;
    if node.is_inline_children {
      final_layout.size.width = node.unrounded_layout.size.width;
    }
    node.final_layout = final_layout;
  }
}

impl<'g> RenderNode<'g> {
  fn anonymous_box_context(parent_context: &RenderContext<'g>) -> RenderContext<'g> {
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
    context
  }

  fn anonymous_text_item(parent_context: &RenderContext<'g>, text: String) -> Self {
    let context = Self::anonymous_box_context(parent_context);

    Self {
      context,
      node: None,
      children: None,
      layout_style_override: Some(Style {
        display: TaffyDisplay::Block,
        ..Style::default()
      }),
      anonymous_text_content: Some(text),
      force_inline_layout: true,
    }
  }

  fn anonymous_block_container(
    parent_context: &RenderContext<'g>,
    children: Vec<RenderNode<'g>>,
  ) -> Self {
    Self {
      context: Self::anonymous_box_context(parent_context),
      node: None,
      children: Some(children.into_boxed_slice()),
      layout_style_override: Some(Style {
        display: TaffyDisplay::Block,
        ..Style::default()
      }),
      anonymous_text_content: None,
      force_inline_layout: false,
    }
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

  fn has_anonymous_text_item_child(&self) -> bool {
    self
      .children
      .as_ref()
      .is_some_and(|children| children.iter().any(RenderNode::is_anonymous_text_item))
  }

  pub(crate) fn draw_shell(&self, canvas: &mut Canvas, layout: Layout) -> Result<()> {
    let Some(node) = &self.node else {
      return Ok(());
    };

    node.draw_outset_box_shadow(&self.context, canvas, layout)?;
    node.draw_background(&self.context, canvas, layout)?;
    node.draw_inset_box_shadow(&self.context, canvas, layout)?;
    node.draw_border(&self.context, canvas, layout)?;
    node.draw_outline(&self.context, canvas, layout)?;
    Ok(())
  }

  pub(crate) fn draw_content(&self, canvas: &mut Canvas, layout: Layout) -> Result<()> {
    if self.should_create_inline_layout() || self.has_anonymous_text_item_child() {
      return Ok(());
    }

    if let Some(node) = &self.node {
      node.draw_content(&self.context, canvas, layout)?;
    }
    Ok(())
  }

  pub fn draw_inline(&mut self, canvas: &mut Canvas, layout: Layout) -> Result<()> {
    if self.context.style.opacity.0 == 0.0 {
      return Ok(());
    }

    let font_style = self.context.style.to_sized_font_style(&self.context);

    let max_height = resolve_inline_max_height(&font_style, layout.content_box_height());

    let built = create_inline_layout(InlineLayoutRequest {
      items: collect_inline_items(self),
      available_space: Size {
        width: AvailableSpace::Definite(layout.content_box_width()),
        height: AvailableSpace::Definite(layout.content_box_height()),
      },
      max_width: layout.content_box_width(),
      max_height,
      style: &font_style,
      global: self.context.global,
      mode: InlineLayoutMode::Draw,
    });
    let inline_layout_box = layout;

    let boxes = built.spans.iter().filter_map(|span| match span {
      ProcessedInlineSpan::Box(item) => Some(item),
      _ => None,
    });

    let positioned_inline_boxes = draw_inline_layout(
      &self.context,
      canvas,
      inline_layout_box,
      built.layout,
      &font_style,
      InlineLayoutDrawData {
        spans: &built.spans,
        custom_inline_boxes: &built.custom_inline_boxes,
        line_scales: &built.line_scales,
      },
    )?;

    let inline_transform = Affine::translation(
      inline_layout_box.border.left + inline_layout_box.padding.left,
      inline_layout_box.border.top + inline_layout_box.padding.top,
    ) * self.context.transform;

    for (item, positioned) in boxes.zip(positioned_inline_boxes.iter()) {
      draw_inline_box(positioned, item, canvas, inline_transform)?;
    }
    Ok(())
  }

  pub fn is_inline_level(&self) -> bool {
    self.context.style.display.is_inline_level()
  }

  pub fn is_inline_atomic_container(&self) -> bool {
    matches!(
      self.context.style.display,
      Display::InlineBlock | Display::InlineFlex | Display::InlineGrid
    )
  }

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
      || matches!(self.context.style.position, Position::Absolute)
      || self.context.style.float != Float::None
  }

  pub fn should_create_inline_layout(&self) -> bool {
    self.force_inline_layout
      || (matches!(
        self.context.style.display,
        Display::Block | Display::InlineBlock
      ) && self.children.as_ref().is_some_and(|children| {
        children
          .iter()
          .any(RenderNode::participates_in_inflow_inline_formatting_context)
          && children
            .iter()
            .all(RenderNode::participates_in_inline_formatting_context)
      }))
  }

  pub fn from_node(parent_context: &RenderContext<'g>, node: Node) -> Self {
    let matched_styles = match_stylesheets_view(
      &node,
      &parent_context.stylesheet,
      parent_context.sizing.viewport,
    );
    let mut tree = Self::from_node_iterative(parent_context, node, &matched_styles);

    if tree.is_inline_level() {
      tree.context.style.display.blockify();
    }

    tree
  }

  fn from_node_iterative(
    parent_context: &RenderContext<'g>,
    root: Node,
    matched_declarations: &[MatchedDeclarationsView<'_>],
  ) -> Self {
    struct PendingRenderNode<'g> {
      context: RenderContext<'g>,
      node: Node,
      children_is_some: bool,
      pending_children: IntoIter<Node>,
      rendered_children: Vec<RenderNode<'g>>,
    }

    fn next_preorder_index(preorder_cursor: &mut usize) -> usize {
      let node_index = *preorder_cursor;
      *preorder_cursor += 1;
      node_index
    }

    fn take_children_vec(node: &mut Node) -> (bool, Vec<Node>) {
      let children = node.take_children();
      let children_is_some = children.is_some();
      let children = children.map_or_else(Vec::new, <[Node]>::into_vec);
      (children_is_some, children)
    }

    fn build_render_context<'g>(
      parent_context: &RenderContext<'g>,
      style: ComputedStyle,
      sizing: Sizing,
      current_color: Color,
    ) -> RenderContext<'g> {
      RenderContext {
        global: parent_context.global,
        transform: parent_context.transform,
        style: Box::new(style),
        current_color,
        time: parent_context.time,
        draw_debug_border: parent_context.draw_debug_border,
        fetched_resources: parent_context.fetched_resources.clone(),
        sizing,
        stylesheet: parent_context.stylesheet.clone(),
      }
    }

    fn resolve_computed_style<'g>(
      parent_context: &RenderContext<'g>,
      node: &mut Node,
      node_index: usize,
      matched_declarations: &[MatchedDeclarationsView<'_>],
    ) -> (ComputedStyle, Sizing, Color) {
      let default_matched = MatchedDeclarationsView::default();
      let matched = matched_declarations
        .get(node_index)
        .unwrap_or(&default_matched);
      let layers = node.take_style_layers();
      let style_layers = build_style_layers(layers, matched, parent_context.sizing.viewport);
      let inherited_parent = registered_custom_property_parent_style(
        &parent_context.style,
        std::slice::from_ref(parent_context.stylesheet.as_ref()),
        parent_context.sizing.viewport,
      );
      let mut style = style_layers.inherit(&inherited_parent);

      // On the root element, `parent_root_font_size` is `None` so `rem` inside
      // its own `font-size` falls back to `viewport.font_size`, per CSS Values 4
      // §6.1. Descendants see `Some(root_font_size)`.
      let parent_root_font_size = parent_context.sizing.root_font_size;
      let parent_root_line_height = parent_context.sizing.root_line_height;
      let font_size = style
        .font_size
        .to_px(&parent_context.sizing, parent_context.sizing.font_size);
      let normal_basis = resolve_normal_line_height(parent_context.global, &style, font_size);
      let line_height = style
        .line_height
        .to_px(&parent_context.sizing, normal_basis);
      let child_sizing = Sizing {
        font_size,
        root_font_size: parent_root_font_size,
        line_height,
        root_line_height: parent_root_line_height,
        ..parent_context.sizing.clone()
      };
      let child_current_color = style.color.resolve(parent_context.current_color);
      let child_context = build_render_context(
        parent_context,
        style.clone(),
        child_sizing.clone(),
        child_current_color,
      );
      style = apply_stylesheet_animations(style, &child_context);

      for &declarations in &matched.important {
        for declaration in declarations.iter() {
          declaration.apply_to_computed(&mut style);
        }
      }

      let font_size = style.font_size.to_px(&child_sizing, child_sizing.font_size);
      let normal_basis = resolve_normal_line_height(parent_context.global, &style, font_size);
      let line_height = style
        .line_height
        .to_px(&parent_context.sizing, normal_basis);
      let sizing = Sizing {
        font_size,
        root_font_size: Some(parent_root_font_size.unwrap_or(font_size)),
        line_height,
        root_line_height: Some(parent_root_line_height.unwrap_or(line_height)),
        ..parent_context.sizing.clone()
      };
      let current_color = style.color.resolve(parent_context.current_color);
      style.make_computed(&sizing);
      (style, sizing, current_color)
    }

    fn build_pending_node<'g>(
      parent_context: &RenderContext<'g>,
      mut node: Node,
      matched_declarations: &[MatchedDeclarationsView<'_>],
      preorder_cursor: &mut usize,
    ) -> PendingRenderNode<'g> {
      let node_index = next_preorder_index(preorder_cursor);
      let (style, sizing, current_color) =
        resolve_computed_style(parent_context, &mut node, node_index, matched_declarations);
      let (children_is_some, children) = take_children_vec(&mut node);
      let context = build_render_context(parent_context, style, sizing, current_color);

      PendingRenderNode {
        context,
        node,
        children_is_some,
        rendered_children: Vec::with_capacity(children.len()),
        pending_children: children.into_iter(),
      }
    }

    let mut preorder_cursor = 0;
    let mut stack = vec![build_pending_node(
      parent_context,
      root,
      matched_declarations,
      &mut preorder_cursor,
    )];

    loop {
      let Some(current) = stack.last_mut() else {
        return RenderNode {
          context: parent_context.clone(),
          node: Some(Node::container([])),
          children: None,
          layout_style_override: None,
          anonymous_text_content: None,
          force_inline_layout: false,
        };
      };

      if let Some(child) = current.pending_children.next() {
        // CSS Grid L1 §6 / Flexbox L1 §4: whitespace-only anonymous items
        // are not rendered. Advance the preorder cursor to keep
        // `matched_declarations` indices aligned with the original tree.
        if current.context.style.display.should_blockify_children()
          && child.is_whitespace_only_text()
        {
          next_preorder_index(&mut preorder_cursor);
          continue;
        }

        let child_pending = build_pending_node(
          &current.context,
          child,
          matched_declarations,
          &mut preorder_cursor,
        );
        stack.push(child_pending);
        continue;
      }

      let Some(mut finished) = stack.pop() else {
        return RenderNode {
          context: parent_context.clone(),
          node: Some(Node::container([])),
          children: None,
          layout_style_override: None,
          anonymous_text_content: None,
          force_inline_layout: false,
        };
      };

      let children = if finished.children_is_some {
        Some(finished.rendered_children.into_boxed_slice())
      } else {
        None
      };

      let render_node = if let Some(mut children) = children {
        if finished.context.style.display.should_blockify_children() {
          for child in &mut children {
            child.context.style.display.blockify();
          }

          RenderNode {
            context: finished.context,
            node: Some(finished.node),
            children: Some(children),
            layout_style_override: None,
            anonymous_text_content: None,
            force_inline_layout: false,
          }
        } else {
          // https://github.com/kane50613/takumi/issues/711
          let has_block_level = children
            .iter()
            .any(|child| !child.participates_in_inline_formatting_context());
          if has_block_level
            && children
              .iter()
              .any(RenderNode::is_whitespace_only_text_node)
          {
            children = Vec::from(children)
              .into_iter()
              .filter(|child| !child.is_whitespace_only_text_node())
              .collect();
          }

          let has_inline = children
            .iter()
            .any(RenderNode::participates_in_inline_formatting_context);
          let has_block = children
            .iter()
            .any(|child| !child.participates_in_inline_formatting_context());
          let requires_inline_parent_blockification =
            finished.context.style.display.is_inline() && has_block;
          let needs_anonymous_boxes = has_inline && has_block;

          if requires_inline_parent_blockification {
            finished.context.style.display = finished.context.style.display.as_blockified();
          }

          if !needs_anonymous_boxes {
            RenderNode {
              context: finished.context,
              node: Some(finished.node),
              children: Some(children),
              layout_style_override: None,
              anonymous_text_content: None,
              force_inline_layout: false,
            }
          } else {
            let mut final_children = Vec::new();
            let mut inline_group = Vec::new();

            for item in children {
              if item.participates_in_inline_formatting_context() {
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
              children: Some(final_children.into_boxed_slice()),
              layout_style_override: None,
              anonymous_text_content: None,
              force_inline_layout: false,
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
            children: Some([anonymous_text_item].into()),
            layout_style_override: None,
            anonymous_text_content: None,
            force_inline_layout: false,
          }
        } else {
          RenderNode {
            context: finished.context,
            node: Some(finished.node),
            children: None,
            layout_style_override: None,
            anonymous_text_content: None,
            force_inline_layout: false,
          }
        }
      };

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
      height += self.context.style.border_top_width.to_px(sizing, 0.0)
        + self.context.style.border_bottom_width.to_px(sizing, 0.0)
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
        self.context.style.border_left_width.to_px(sizing, 0.0)
      }
      + if !self.context.style.border_right_style.is_rendered() {
        0.0
      } else {
        self.context.style.border_right_width.to_px(sizing, 0.0)
      };
    let vertical_insets = self.context.style.padding_top.to_px(sizing, 0.0)
      + self.context.style.padding_bottom.to_px(sizing, 0.0)
      + if !self.context.style.border_top_style.is_rendered() {
        0.0
      } else {
        self.context.style.border_top_width.to_px(sizing, 0.0)
      }
      + if !self.context.style.border_bottom_style.is_rendered() {
        0.0
      } else {
        self.context.style.border_bottom_width.to_px(sizing, 0.0)
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

    let font_style = self.context.style.to_sized_font_style(&self.context);
    let max_width = size.width.max(0.0);
    let built = create_inline_layout(InlineLayoutRequest {
      items: collect_inline_items(self),
      available_space: Size {
        width: AvailableSpace::Definite(max_width),
        height: available_space.height,
      },
      max_width,
      max_height: None,
      style: &font_style,
      global: self.context.global,
      mode: InlineLayoutMode::Measure,
    });
    let line = if use_last_line {
      built.layout.lines().last()?
    } else {
      built.layout.lines().next()?
    };
    let metrics = line.metrics();
    let sizing = &self.context.sizing;
    let margin_top = self.context.style.margin_top.to_px(sizing, 0.0);
    let border_top = self.context.style.border_top_width.to_px(sizing, 0.0);
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
        let overflow_hidden_inline_block = display == Display::InlineBlock
          && (self.context.style.overflow_x != Overflow::Visible
            || self.context.style.overflow_y != Overflow::Visible);

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
        size: Size::zero(),
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
        .layout(results.root_node_id())
        .map_or(Size::zero(), |layout| layout.size)
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
        if let Some(node) = tree.get_layout_node_mut_ref(tree.root_node_id())
          && node.style.display == TaffyDisplay::Block
        {
          node.style.display = TaffyDisplay::Flex;
          node.style.flex_direction = taffy::FlexDirection::Row;
          node.style.justify_content = Some(taffy::JustifyContent::Start);
        }

        tree.compute_layout(Size {
          width: AvailableSpace::MaxContent,
          height: available_space.height,
        });

        let results = tree.into_results();
        results
          .layout(results.root_node_id())
          .map_or(Size::zero(), |layout| layout.size)
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
      let root_node_id = results.root_node_id();

      return results.layout(root_node_id).map_or(
        AtomicInlineMetrics {
          size: Size::zero(),
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

      let font_style = self.context.style.to_sized_font_style(&self.context);

      let mut built = create_inline_layout(InlineLayoutRequest {
        items: collect_inline_items(self),
        available_space,
        max_width,
        max_height,
        style: &font_style,
        global: self.context.global,
        mode: InlineLayoutMode::Measure,
      });

      let ceil_width = font_style.parent.text_wrap_mode_and_line_clamp().0 == TextWrapMode::Wrap;
      let parent_font_metrics = get_parent_font_metrics(&built.layout);
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
      return Size::zero();
    };

    node.measure(&self.context, available_space, known_dimensions, style)
  }
}

fn flush_inline_group<'g>(
  inline_group: &mut Vec<RenderNode<'g>>,
  final_children: &mut Vec<RenderNode<'g>>,
  parent_render_context: &RenderContext<'g>,
) {
  if inline_group.is_empty() {
    return;
  }

  final_children.push(RenderNode::anonymous_block_container(
    parent_render_context,
    take(inline_group),
  ));
}

#[cfg(test)]
mod tests {
  use cssparser::{Parser, ParserInput};
  use taffy::NodeId;

  use super::{registered_custom_property_parent_style, sort_children_by_order};
  use crate::layout::style::{PropertyRule, StyleDeclaration, StyleDeclarationBlock, StyleSheet};
  use crate::layout::{
    Viewport,
    style::{ComputedStyle, Length, Style},
  };

  fn parse_stylesheet(css: &str) -> StyleSheet {
    let result = StyleSheet::parse(css);
    assert!(result.is_ok(), "expected stylesheet to parse: {result:?}");
    result.unwrap_or_default()
  }

  #[test]
  fn sort_children_by_order_keeps_source_order_for_equal_values() {
    let children = vec![
      NodeId::from(3usize),
      NodeId::from(1usize),
      NodeId::from(2usize),
    ];
    let sorted = sort_children_by_order(&children, |child_id| match usize::from(child_id) {
      1 => -1,
      _ => 0,
    });
    assert_eq!(
      sorted,
      vec![
        NodeId::from(1usize),
        NodeId::from(3usize),
        NodeId::from(2usize)
      ]
    );
  }

  #[test]
  fn registered_custom_property_can_disable_inheritance() {
    let mut parent = ComputedStyle::default();
    parent
      .custom_properties
      .insert("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet {
      property_rules: vec![PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: false,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      }],
      ..StyleSheet::default()
    }];

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
    parent
      .custom_properties
      .insert("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet {
      property_rules: vec![PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      }],
      ..StyleSheet::default()
    }];

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

    let stylesheets = [StyleSheet {
      property_rules: vec![PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      }],
      ..StyleSheet::default()
    }];

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

    let stylesheets = [StyleSheet {
      property_rules: vec![
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
      ],
      ..StyleSheet::default()
    }];

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
    parent
      .custom_properties
      .insert("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet {
      property_rules: vec![
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
      ],
      ..StyleSheet::default()
    }];

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

    let stylesheets = [StyleSheet {
      property_rules: vec![
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
      ],
      ..StyleSheet::default()
    }];

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
    let mut input = ParserInput::new("var(--box-size)");
    let mut parser = Parser::new(&mut input);
    let declarations = StyleDeclarationBlock::parse("width", &mut parser);
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
}
