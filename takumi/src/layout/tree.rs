use std::{iter::Copied, mem::take, slice::Iter, vec::IntoIter};

use taffy::{
  AvailableSpace, Cache, CacheTree, Display as TaffyDisplay, Layout, LayoutBlockContainer,
  LayoutFlexboxContainer, LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree,
  NodeId, RequestedAxis, RoundTree, RunMode, Size, SizingMode, Style, TaffyError,
  TraversePartialTree, TraverseTree, compute_block_layout, compute_cached_layout,
  compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
  compute_root_layout, round_layout,
};

#[cfg(feature = "css_stylesheet_parsing")]
use crate::layout::style::matching::{MatchedDeclarations, match_stylesheets};
use crate::{
  Result,
  layout::{
    Viewport,
    inline::{
      InlineContentKind, InlineLayoutStage, ProcessedInlineSpan, collect_inline_items,
      create_inline_constraint, create_inline_layout, measure_inline_layout,
    },
    node::{Node, NodeStyleLayers},
    style::{
      Affine, BlendMode, Color, Display, Filters, Isolation, PercentageNumber, ResolvedStyle,
      Style as NodeStyle, apply_stylesheet_animations,
    },
  },
  rendering::{
    Canvas, MaxHeight, RenderContext, Sizing,
    inline_drawing::{draw_inline_box, draw_inline_layout},
  },
};

pub(crate) struct LayoutResults {
  nodes: Vec<LayoutResultNode>,
}

struct LayoutResultNode {
  layout: Layout,
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
}

pub(crate) struct LayoutTree<'r, 'g, N: Node<N>> {
  nodes: Vec<LayoutNodeState>,
  render_nodes: Vec<&'r RenderNode<'g, N>>,
}

struct LayoutNodeState {
  style: Style,
  cache: Cache,
  unrounded_layout: Layout,
  final_layout: Layout,
  is_inline_children: bool,
  children: Box<[NodeId]>,
}

#[derive(Clone)]
pub(crate) struct RenderNode<'g, N: Node<N>> {
  pub(crate) context: RenderContext<'g>,
  pub(crate) node: Option<N>,
  pub(crate) children: Option<Box<[RenderNode<'g, N>]>>,
  pub(crate) layout_style_override: Option<Style>,
  pub(crate) anonymous_text_content: Option<String>,
  pub(crate) force_inline_layout: bool,
}

fn build_style_layers(
  node_layers: NodeStyleLayers,
  #[cfg(feature = "css_stylesheet_parsing")] matched_declarations: &MatchedDeclarations,
  viewport: Viewport,
) -> NodeStyle {
  let mut style = NodeStyle::default();

  if let Some(preset) = node_layers.preset {
    style.merge_from(preset);
  }

  #[cfg(feature = "css_stylesheet_parsing")]
  for declaration in matched_declarations.normal.iter() {
    declaration.merge_into(&mut style);
  }

  if let Some(author_tw) = node_layers.author_tw {
    style.append_block(author_tw.into_declaration_block(viewport));
  }

  if let Some(inline) = node_layers.inline {
    style.merge_from(inline);
  }

  #[cfg(feature = "css_stylesheet_parsing")]
  for declaration in matched_declarations.important.iter() {
    declaration.merge_into(&mut style);
  }

  style
}

#[cfg(test)]
fn build_inherited_style(
  parent_style: &ResolvedStyle,
  node_layers: NodeStyleLayers,
  #[cfg(feature = "css_stylesheet_parsing")] matched_declarations: &MatchedDeclarations,
  viewport: Viewport,
) -> ResolvedStyle {
  build_style_layers(
    node_layers,
    #[cfg(feature = "css_stylesheet_parsing")]
    matched_declarations,
    viewport,
  )
  .inherit(parent_style)
}

fn push_layout_node<'r, 'g, N: Node<N>>(
  nodes: &mut Vec<LayoutNodeState>,
  render_nodes: &mut Vec<&'r RenderNode<'g, N>>,
  render_root: &'r RenderNode<'g, N>,
) -> NodeId {
  struct PendingNode<'r, 'g, N: Node<N>> {
    node_id: NodeId,
    next_child_index: usize,
    children: Option<&'r [RenderNode<'g, N>]>,
    child_ids: Vec<NodeId>,
  }

  fn push_node_state<'r, 'g, N: Node<N>>(
    nodes: &mut Vec<LayoutNodeState>,
    render_nodes: &mut Vec<&'r RenderNode<'g, N>>,
    render_node: &'r RenderNode<'g, N>,
  ) -> PendingNode<'r, 'g, N> {
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
      is_inline_children,
      children: Box::new([]),
    });

    PendingNode {
      node_id,
      next_child_index: 0,
      children,
      child_ids: Vec::with_capacity(children.map_or(0, <[RenderNode<'g, N>]>::len)),
    }
  }

  let root = push_node_state(nodes, render_nodes, render_root);
  let root_id = root.node_id;
  let mut stack = vec![root];

  while let Some(current) = stack.last_mut() {
    let Some(children) = current.children else {
      let Some(finished) = stack.pop() else {
        unreachable!();
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
      unreachable!();
    };
    let node_index: usize = finished.node_id.into();
    nodes[node_index].children = finished.child_ids.into_boxed_slice();

    if let Some(parent) = stack.last_mut() {
      parent.child_ids.push(finished.node_id);
    }
  }

  root_id
}

impl<'r, 'g, N: Node<N>> LayoutTree<'r, 'g, N> {
  pub(crate) fn from_render_node(render_root: &'r RenderNode<'g, N>) -> Self {
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
fn should_strip_flex_intrinsic_stretch_known_dimension<N: Node<N>>(
  render_node: &RenderNode<'_, N>,
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

impl<N: Node<N>> TraversePartialTree for LayoutTree<'_, '_, N> {
  type ChildIter<'a>
    = Copied<Iter<'a, NodeId>>
  where
    Self: 'a;

  fn child_ids(&self, parent_node_id: NodeId) -> Self::ChildIter<'_> {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      unreachable!()
    };

    node.children.iter().copied()
  }

  fn child_count(&self, parent_node_id: NodeId) -> usize {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      unreachable!()
    };

    node.children.len()
  }

  fn get_child_id(&self, parent_node_id: NodeId, child_index: usize) -> NodeId {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      unreachable!()
    };

    node.children[child_index]
  }
}

impl<N: Node<N>> TraverseTree for LayoutTree<'_, '_, N> {}

impl<N: Node<N>> LayoutPartialTree for LayoutTree<'_, '_, N> {
  type CoreContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type CustomIdent = String;

  fn get_core_container_style(&self, node_id: NodeId) -> Self::CoreContainerStyle<'_> {
    let Some(node) = self.get_layout_node_ref(node_id) else {
      unreachable!()
    };

    &node.style
  }

  fn set_unrounded_layout(&mut self, node_id: NodeId, layout: &Layout) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      unreachable!()
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
    self.update_node_style_for_available_space(
      node,
      inputs.available_space,
      inputs.known_dimensions,
    );

    if inputs.run_mode == RunMode::PerformHiddenLayout {
      return compute_hidden_layout(self, node);
    }

    compute_cached_layout(self, node, inputs, |tree, node, inputs| {
      let Some(node_data) = tree.get_layout_node_ref(node) else {
        unreachable!()
      };

      let display_mode = node_data.style.display;
      let has_children = !node_data.children.is_empty();

      match (display_mode, has_children) {
        (TaffyDisplay::None, _) => compute_hidden_layout(tree, node),
        (TaffyDisplay::Block, true) => compute_block_layout(tree, node, inputs),
        (TaffyDisplay::Flex, true) => compute_flexbox_layout(tree, node, inputs),
        (TaffyDisplay::Grid, true) => compute_grid_layout(tree, node, inputs),
        (_, false) => compute_leaf_layout(
          inputs,
          &node_data.style,
          |val, basis| tree.resolve_calc_value(val, basis),
          |known_dimensions, available_space| {
            let idx: usize = node.into();
            let Some(render_node) = tree.render_nodes.get(idx) else {
              unreachable!()
            };

            let known_dimensions = if should_strip_flex_intrinsic_stretch_known_dimension(
              render_node,
              inputs,
              known_dimensions,
            ) {
              Size::NONE
            } else {
              known_dimensions
            };
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
        ),
      }
    })
  }
}

impl<N: Node<N>> CacheTree for LayoutTree<'_, '_, N> {
  fn cache_get(
    &self,
    node_id: NodeId,
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    run_mode: RunMode,
  ) -> Option<LayoutOutput> {
    let Some(node) = self.get_layout_node_ref(node_id) else {
      unreachable!()
    };

    node.cache.get(known_dimensions, available_space, run_mode)
  }

  fn cache_store(
    &mut self,
    node_id: NodeId,
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    run_mode: RunMode,
    layout_output: LayoutOutput,
  ) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      unreachable!()
    };

    node
      .cache
      .store(known_dimensions, available_space, run_mode, layout_output);
  }

  fn cache_clear(&mut self, node_id: NodeId) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      unreachable!()
    };

    node.cache.clear();
  }
}

impl<N: Node<N>> LayoutBlockContainer for LayoutTree<'_, '_, N> {
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
}

impl<N: Node<N>> LayoutFlexboxContainer for LayoutTree<'_, '_, N> {
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

impl<N: Node<N>> LayoutGridContainer for LayoutTree<'_, '_, N> {
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

impl<N: Node<N>> RoundTree for LayoutTree<'_, '_, N> {
  fn get_unrounded_layout(&self, node_id: NodeId) -> Layout {
    let Some(node) = self.get_layout_node_ref(node_id) else {
      unreachable!()
    };

    node.unrounded_layout
  }

  fn set_final_layout(&mut self, node_id: NodeId, layout: &Layout) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      unreachable!()
    };

    node.final_layout = *layout;
  }
}

impl<'g, N: Node<N>> RenderNode<'g, N> {
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
    children: Vec<RenderNode<'g, N>>,
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

    let max_height = match font_style.parent.line_clamp.as_ref() {
      Some(clamp) => Some(MaxHeight::HeightAndLines(
        layout.content_box_height(),
        clamp.count,
      )),
      None => Some(MaxHeight::Absolute(layout.content_box_height())),
    };

    let (inline_layout, _, spans) = create_inline_layout(
      collect_inline_items(self).into_iter(),
      Size {
        width: AvailableSpace::Definite(layout.content_box_width()),
        height: AvailableSpace::Definite(layout.content_box_height()),
      },
      layout.content_box_width(),
      max_height,
      &font_style,
      self.context.global,
      InlineLayoutStage::Draw,
    );
    let inline_layout_box = layout;

    let boxes = spans.iter().filter_map(|span| match span {
      ProcessedInlineSpan::Box(item) => Some(item),
      _ => None,
    });

    let positioned_inline_boxes = draw_inline_layout(
      &self.context,
      canvas,
      inline_layout_box,
      inline_layout,
      &font_style,
      &spans,
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

  pub fn should_create_inline_layout(&self) -> bool {
    self.force_inline_layout
      || (matches!(
        self.context.style.display,
        Display::Block | Display::InlineBlock
      ) && self.children.as_ref().is_some_and(|children| {
        !children.is_empty() && children.iter().all(RenderNode::is_inline_level)
      }))
  }

  pub fn from_node(parent_context: &RenderContext<'g>, node: N) -> Self {
    #[cfg(feature = "css_stylesheet_parsing")]
    let matched_styles = match_stylesheets(&node, &parent_context.stylesheets);
    let mut tree = Self::from_node_iterative(
      parent_context,
      node,
      #[cfg(feature = "css_stylesheet_parsing")]
      &matched_styles,
    );

    if tree.is_inline_level() {
      tree.context.style.display.blockify();
    }

    tree
  }

  fn from_node_iterative(
    parent_context: &RenderContext<'g>,
    root: N,
    #[cfg(feature = "css_stylesheet_parsing")] matched_declarations: &[MatchedDeclarations],
  ) -> Self {
    struct PendingRenderNode<'g, N: Node<N>> {
      context: RenderContext<'g>,
      node: N,
      children_is_some: bool,
      pending_children: IntoIter<N>,
      rendered_children: Vec<RenderNode<'g, N>>,
    }

    fn next_preorder_index(preorder_cursor: &mut usize) -> usize {
      let node_index = *preorder_cursor;
      *preorder_cursor += 1;
      node_index
    }

    fn take_children_vec<N: Node<N>>(node: &mut N) -> (bool, Vec<N>) {
      let children = node.take_children();
      let children_is_some = children.is_some();
      let children = children.map_or_else(Vec::new, <[N]>::into_vec);
      (children_is_some, children)
    }

    fn build_render_context<'g>(
      parent_context: &RenderContext<'g>,
      style: ResolvedStyle,
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
        #[cfg(feature = "css_stylesheet_parsing")]
        stylesheets: parent_context.stylesheets.clone(),
      }
    }

    fn resolve_computed_style<'g, N: Node<N>>(
      parent_context: &RenderContext<'g>,
      node: &mut N,
      node_index: usize,
      #[cfg(feature = "css_stylesheet_parsing")] matched_declarations: &[MatchedDeclarations],
    ) -> (ResolvedStyle, Sizing, Color) {
      #[cfg(feature = "css_stylesheet_parsing")]
      let default_matched = MatchedDeclarations::default();
      #[cfg(feature = "css_stylesheet_parsing")]
      let matched = matched_declarations
        .get(node_index)
        .unwrap_or(&default_matched);
      let layers = node.take_style_layers();
      let style_layers = build_style_layers(
        layers,
        #[cfg(feature = "css_stylesheet_parsing")]
        matched,
        parent_context.sizing.viewport,
      );
      let mut style = style_layers.inherit(&parent_context.style);

      let font_size = style
        .font_size
        .to_px(&parent_context.sizing, parent_context.sizing.font_size);
      let child_sizing = Sizing {
        font_size,
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

      #[cfg(feature = "css_stylesheet_parsing")]
      for declaration in matched.important.iter() {
        declaration.apply_to_resolved(&mut style);
      }

      let font_size = style.font_size.to_px(&child_sizing, child_sizing.font_size);
      let sizing = Sizing {
        font_size,
        ..parent_context.sizing.clone()
      };
      let current_color = style.color.resolve(parent_context.current_color);
      style.make_computed(&sizing);
      (style, sizing, current_color)
    }

    fn build_pending_node<'g, N: Node<N>>(
      parent_context: &RenderContext<'g>,
      mut node: N,
      #[cfg(feature = "css_stylesheet_parsing")] matched_declarations: &[MatchedDeclarations],
      preorder_cursor: &mut usize,
    ) -> PendingRenderNode<'g, N> {
      let node_index = next_preorder_index(preorder_cursor);
      let (style, sizing, current_color) = resolve_computed_style(
        parent_context,
        &mut node,
        node_index,
        #[cfg(feature = "css_stylesheet_parsing")]
        matched_declarations,
      );
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
      #[cfg(feature = "css_stylesheet_parsing")]
      matched_declarations,
      &mut preorder_cursor,
    )];

    loop {
      let Some(current) = stack.last_mut() else {
        unreachable!();
      };

      if let Some(child) = current.pending_children.next() {
        let child_pending = build_pending_node(
          &current.context,
          child,
          #[cfg(feature = "css_stylesheet_parsing")]
          matched_declarations,
          &mut preorder_cursor,
        );
        stack.push(child_pending);
        continue;
      }

      let Some(mut finished) = stack.pop() else {
        unreachable!();
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
          let has_inline = children.iter().any(RenderNode::is_inline_level);
          let has_block = children.iter().any(|child| !child.is_inline_level());
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
              if item.is_inline_level() {
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
            children: Some(vec![anonymous_text_item].into_boxed_slice()),
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

  pub(crate) fn measure_atomic_subtree(&self, available_space: Size<AvailableSpace>) -> Size<f32> {
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

    if self.is_inline_atomic_container() {
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

      return results
        .layout(results.root_node_id())
        .map_or(Size::zero(), |layout| layout.size);
    }

    measure_with(available_space.width)
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

      let (mut layout, _, _) = create_inline_layout(
        collect_inline_items(self).into_iter(),
        available_space,
        max_width,
        max_height,
        &font_style,
        self.context.global,
        InlineLayoutStage::Measure,
      );

      return measure_inline_layout(&mut layout, max_width);
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

fn flush_inline_group<'g, N: Node<N>>(
  inline_group: &mut Vec<RenderNode<'g, N>>,
  final_children: &mut Vec<RenderNode<'g, N>>,
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
  #[cfg(feature = "css_stylesheet_parsing")]
  use smallvec::smallvec;

  use super::build_inherited_style;
  #[cfg(feature = "css_stylesheet_parsing")]
  use crate::layout::style::{
    LonghandId, StyleDeclaration, StyleDeclarationBlock, matching::MatchedDeclarations,
  };
  use crate::layout::{
    Viewport,
    node::NodeStyleLayers,
    style::{Length, ResolvedStyle, Style},
  };

  #[test]
  fn stylesheet_important_overrides_inline_normal() {
    let parent = ResolvedStyle::default();
    let layers = NodeStyleLayers {
      inline: Some(Style::default().with(StyleDeclaration::width(Length::Px(20.0)))),
      ..Default::default()
    };

    #[cfg(feature = "css_stylesheet_parsing")]
    let matched = MatchedDeclarations {
      normal: StyleDeclarationBlock {
        declarations: smallvec![StyleDeclaration::width(Length::Px(20.0))],
        importance_set: Default::default(),
      },
      important: StyleDeclarationBlock {
        declarations: smallvec![StyleDeclaration::width(Length::Px(30.0))],
        importance_set: [LonghandId::Width].into_iter().collect(),
      },
    };

    let resolved = build_inherited_style(
      &parent,
      layers,
      #[cfg(feature = "css_stylesheet_parsing")]
      &matched,
      Viewport::new(Some(1200), Some(630)),
    );

    assert_eq!(resolved.width, Length::Px(30.0));
  }
}
