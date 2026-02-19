use std::{iter::Copied, mem::take, slice::Iter};

use taffy::{
  AvailableSpace, Cache, CacheTree, Display as TaffyDisplay, Layout, LayoutBlockContainer,
  LayoutFlexboxContainer, LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree,
  NodeId, RoundTree, RunMode, Size, Style, TaffyError, TraversePartialTree, TraverseTree,
  compute_block_layout, compute_cached_layout, compute_flexbox_layout, compute_grid_layout,
  compute_hidden_layout, compute_leaf_layout, compute_root_layout, round_layout,
};

use crate::{
  Result,
  layout::{
    inline::{
      InlineLayoutStage, ProcessedInlineSpan, collect_inline_items, create_inline_constraint,
      create_inline_layout, measure_inline_layout,
    },
    node::Node,
    style::{Affine, Display, InheritedStyle},
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
}

fn push_layout_node<'r, 'g, N: Node<N>>(
  nodes: &mut Vec<LayoutNodeState>,
  render_nodes: &mut Vec<&'r RenderNode<'g, N>>,
  render_node: &'r RenderNode<'g, N>,
) -> NodeId {
  let node_index = nodes.len();
  let node_id = NodeId::from(node_index);
  render_nodes.push(render_node);

  nodes.push(LayoutNodeState {
    style: render_node
      .context
      .style
      .to_taffy_style(&render_node.context),
    cache: Cache::new(),
    unrounded_layout: Layout::new(),
    final_layout: Layout::new(),
    is_inline_children: render_node.should_create_inline_layout(),
    children: Box::new([]),
  });

  if nodes[node_index].is_inline_children {
    return node_id;
  }

  if let Some(children) = render_node.children.as_deref() {
    nodes.reserve(children.len());
    render_nodes.reserve(children.len());
    nodes[node_index].children = Box::from_iter(
      children
        .iter()
        .map(|child| push_layout_node(nodes, render_nodes, child)),
    );
  }

  node_id
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
            if let Size {
              width: Some(width),
              height: Some(height),
            } = known_dimensions.maybe_apply_aspect_ratio(node_data.style.aspect_ratio)
            {
              return Size { width, height };
            }

            let idx: usize = node.into();
            let Some(render_node) = tree.render_nodes.get(idx) else {
              unreachable!()
            };

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

    let boxes = spans.iter().filter_map(|span| match span {
      ProcessedInlineSpan::Box(item) => Some(item),
      _ => None,
    });

    let positioned_inline_boxes = draw_inline_layout(
      &self.context,
      canvas,
      layout,
      inline_layout,
      &font_style,
      &spans,
    )?;

    let inline_transform = Affine::translation(
      layout.border.left + layout.padding.left,
      layout.border.top + layout.padding.top,
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
    matches!(
      self.context.style.display,
      Display::Block | Display::InlineBlock
    ) && self.children.as_ref().is_some_and(|children| {
      !children.is_empty() && children.iter().all(RenderNode::is_inline_level)
    })
  }

  pub fn from_node(parent_context: &RenderContext<'g>, node: N) -> Self {
    let mut tree = Self::from_node_impl(parent_context, node);

    if tree.is_inline_level() {
      tree.context.style.display.blockify();
    }

    tree
  }

  fn from_node_impl(parent_context: &RenderContext<'g>, mut node: N) -> Self {
    let mut style =
      node.create_inherited_style(&parent_context.style, parent_context.sizing.viewport);

    let font_size = style
      .font_size
      .map(|font_size| font_size.to_px(&parent_context.sizing, parent_context.sizing.font_size))
      .unwrap_or(parent_context.sizing.font_size);

    let current_color = style.color.resolve(parent_context.current_color);

    let sizing = Sizing {
      font_size,
      ..parent_context.sizing.clone()
    };

    style.make_computed(&sizing);

    let mut render_context = RenderContext {
      global: parent_context.global,
      transform: parent_context.transform,
      style,
      current_color,
      draw_debug_border: parent_context.draw_debug_border,
      fetched_resources: parent_context.fetched_resources.clone(),
      sizing,
    };

    let children = node.take_children().map(|children| {
      Box::from_iter(
        children
          .into_iter()
          .map(|child| Self::from_node_impl(&render_context, child)),
      )
    });

    let Some(mut children) = children else {
      return Self {
        context: render_context,
        node: Some(node),
        children: None,
      };
    };

    if render_context.style.display.should_blockify_children() {
      for child in &mut children {
        child.context.style.display.blockify();
      }

      return Self {
        context: render_context,
        node: Some(node),
        children: Some(children),
      };
    }

    let has_inline = children.iter().any(RenderNode::is_inline_level);
    let has_block = children.iter().any(|child| !child.is_inline_level());
    let needs_anonymous_boxes =
      !render_context.style.display.is_inline() && has_inline && has_block;

    if !needs_anonymous_boxes {
      return Self {
        context: render_context,
        node: Some(node),
        children: Some(children),
      };
    }

    render_context.style.display = render_context.style.display.as_blockified();

    let mut final_children = Vec::new();
    let mut inline_group = Vec::new();

    let anonymous_box_style = InheritedStyle {
      display: Display::Block,
      ..InheritedStyle::default()
    };

    for item in children {
      if item.is_inline_level() {
        inline_group.push(item);
        continue;
      }

      flush_inline_group(
        &mut inline_group,
        &mut final_children,
        &anonymous_box_style,
        &render_context,
      );

      final_children.push(item);
    }

    flush_inline_group(
      &mut inline_group,
      &mut final_children,
      &anonymous_box_style,
      &render_context,
    );

    Self {
      context: render_context,
      node: Some(node),
      children: Some(final_children.into_boxed_slice()),
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
      let max_content = measure_with(AvailableSpace::MaxContent);
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
    style: &taffy::Style,
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
  anonymous_box_style: &InheritedStyle,
  parent_render_context: &RenderContext<'g>,
) {
  if inline_group.is_empty() {
    return;
  }

  if inline_group.len() == 1 {
    let Some(mut child) = inline_group.pop() else {
      unreachable!();
    };

    child.context.style.display.blockify();

    final_children.push(child);
  } else {
    final_children.push(RenderNode {
      context: RenderContext {
        style: anonymous_box_style.clone(),
        global: parent_render_context.global,
        transform: parent_render_context.transform,
        sizing: parent_render_context.sizing.clone(),
        current_color: parent_render_context.current_color,
        draw_debug_border: parent_render_context.draw_debug_border,
        fetched_resources: Default::default(),
      },
      children: Some(take(inline_group).into_boxed_slice()),
      node: None,
    });
  }
}
