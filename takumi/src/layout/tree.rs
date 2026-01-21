use std::mem::take;

use taffy::{AvailableSpace, Layout, NodeId, Size, TaffyTree};

use crate::{
  Result,
  layout::{
    inline::{
      InlineItemIterator, InlineLayoutStage, ProcessedInlineSpan, create_inline_constraint,
      create_inline_layout, measure_inline_layout,
    },
    node::Node,
    style::{Display, InheritedStyle},
  },
  rendering::{
    Canvas, MaxHeight, RenderContext, Sizing,
    inline_drawing::{draw_inline_box, draw_inline_layout},
  },
};

pub(crate) struct NodeTree<'g, N: Node<N>> {
  pub(crate) context: RenderContext<'g>,
  pub(crate) node: Option<N>,
  pub(crate) children: Option<Box<[NodeTree<'g, N>]>>,
}

impl<'g, N: Node<N>> NodeTree<'g, N> {
  pub(crate) fn draw_shell(&self, canvas: &mut Canvas, layout: Layout) -> Result<()> {
    let Some(node) = &self.node else {
      return Ok(());
    };

    node.draw_outset_box_shadow(&self.context, canvas, layout)?;
    node.draw_background_color(&self.context, canvas, layout)?;
    node.draw_background_image(&self.context, canvas, layout)?;
    node.draw_inset_box_shadow(&self.context, canvas, layout)?;
    node.draw_border(&self.context, canvas, layout)?;
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
      self.inline_items_iter(),
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
      ProcessedInlineSpan::Box { node, .. } => Some(node),
      _ => None,
    });

    // Draw the inline layout without a callback first
    let positioned_inline_boxes =
      draw_inline_layout(&self.context, canvas, layout, inline_layout, &font_style)?;

    // Then handle the inline boxes directly by zipping the node refs with their positioned boxes
    for (node, positioned) in boxes.zip(positioned_inline_boxes.iter()) {
      draw_inline_box(positioned, node, canvas, self.context.transform)?;
    }
    Ok(())
  }

  pub fn is_inline(&self) -> bool {
    self.context.style.display == Display::Inline
  }

  pub fn should_create_inline_layout(&self) -> bool {
    self.context.style.display == Display::Block
      && self
        .children
        .as_ref()
        .is_some_and(|children| !children.is_empty() && children.iter().all(NodeTree::is_inline))
  }

  pub fn from_node(parent_context: &RenderContext<'g>, node: N) -> Self {
    let mut tree = Self::from_node_impl(parent_context, node);

    // https://www.w3.org/TR/css-display-3/#root
    // The root element’s display type is always blockified.
    if tree.is_inline() {
      tree.context.style.display.blockify();
    }

    tree
  }

  fn from_node_impl(parent_context: &RenderContext<'g>, mut node: N) -> Self {
    let style = node.create_inherited_style(&parent_context.style, parent_context.sizing.viewport);

    let font_size = style
      .font_size
      .map(|font_size| font_size.to_px(&parent_context.sizing, parent_context.sizing.font_size))
      .unwrap_or(parent_context.sizing.font_size);

    let current_color = style.color.resolve(parent_context.current_color);

    let mut context = RenderContext {
      style,
      current_color,
      fetched_resources: parent_context.fetched_resources.clone(),
      sizing: Sizing {
        font_size,
        ..parent_context.sizing
      },
      ..*parent_context
    };

    let children = node.take_children().map(|children| {
      Box::from_iter(
        children
          .into_iter()
          .map(|child| Self::from_node_impl(&context, child)),
      )
    });

    let Some(mut children) = children else {
      return Self {
        context,
        node: Some(node),
        children: None,
      };
    };

    if context.style.display.should_blockify_children() {
      for child in &mut children {
        child.context.style.display.blockify();
      }

      return Self {
        context,
        node: Some(node),
        children: Some(children),
      };
    }

    let has_inline = children.iter().any(NodeTree::is_inline);
    let has_block = children.iter().any(|child| !child.is_inline());
    let needs_anonymous_boxes = !context.style.display.is_inline() && has_inline && has_block;

    if !needs_anonymous_boxes {
      return Self {
        context,
        node: Some(node),
        children: Some(children),
      };
    }

    context.style.display = context.style.display.as_blockified();

    let mut final_children = Vec::new();
    let mut inline_group = Vec::new();

    // Anonymous block box style.
    let anonymous_box_style = InheritedStyle {
      display: Display::Block,
      ..InheritedStyle::default()
    };

    for item in children {
      if item.is_inline() {
        inline_group.push(item);
        continue;
      }

      flush_inline_group(
        &mut inline_group,
        &mut final_children,
        &anonymous_box_style,
        &context,
      );

      final_children.push(item);
    }

    flush_inline_group(
      &mut inline_group,
      &mut final_children,
      &anonymous_box_style,
      &context,
    );

    Self {
      context,
      node: Some(node),
      children: Some(final_children.into_boxed_slice()),
    }
  }

  pub(crate) fn insert_into_taffy(
    mut self,
    tree: &mut TaffyTree<NodeTree<'g, N>>,
  ) -> Result<NodeId> {
    assert_ne!(
      self.context.style.display,
      Display::Inline,
      "Inline nodes should be wrapped in anonymous block boxes"
    );

    if self.should_create_inline_layout() {
      return Ok(
        tree.new_leaf_with_context(self.context.style.to_taffy_style(&self.context), self)?,
      );
    }

    let children = self.children.take();

    let node_id =
      tree.new_leaf_with_context(self.context.style.to_taffy_style(&self.context), self)?;

    if let Some(children) = children {
      let children_ids = children
        .into_iter()
        .map(|child| child.insert_into_taffy(tree))
        .collect::<Result<Vec<_>>>()?;

      tree.set_children(node_id, &children_ids)?;
    }

    Ok(node_id)
  }

  pub(crate) fn measure(
    &self,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &taffy::Style,
  ) -> Size<f32> {
    if self.should_create_inline_layout() {
      let (max_width, max_height) =
        create_inline_constraint(&self.context, available_space, known_dimensions);

      let font_style = self.context.style.to_sized_font_style(&self.context);

      let (mut layout, _, _) = create_inline_layout(
        self.inline_items_iter(),
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

  pub(crate) fn inline_items_iter(&self) -> InlineItemIterator<'_, 'g, N> {
    InlineItemIterator {
      stack: vec![(self, 0)], // (node, depth)
      current_node_content: None,
    }
  }
}

fn flush_inline_group<'g, N: Node<N>>(
  inline_group: &mut Vec<NodeTree<'g, N>>,
  final_children: &mut Vec<NodeTree<'g, N>>,
  anonymous_box_style: &InheritedStyle,
  context: &RenderContext<'g>,
) {
  if inline_group.is_empty() {
    return;
  }

  if inline_group.len() == 1 {
    if let Some(mut child) = take(inline_group).into_iter().next() {
      child.context.style.display.blockify();
      final_children.push(child);
    }
  } else {
    final_children.push(NodeTree {
      context: RenderContext {
        style: anonymous_box_style.clone(),
        fetched_resources: Default::default(), // anonymous box has nothing to render, so provide an empty map.
        ..*context
      },
      children: Some(take(inline_group).into_boxed_slice()),
      node: None,
    });
  }
}
