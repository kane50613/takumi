use std::{borrow::Cow, mem::take};

use parley::InlineBox;
use taffy::{AvailableSpace, Layout, NodeId, Size, TaffyTree};

use crate::{
  GlobalContext, Result,
  layout::{
    inline::{InlineContentKind, InlineItem, InlineLayout, break_lines, create_inline_constraint},
    node::Node,
    style::{Display, InheritedStyle, SizedFontStyle},
  },
  rendering::{
    Canvas, MaxHeight, RenderContext, Sizing,
    inline_drawing::{draw_inline_box, draw_inline_layout},
  },
};

pub(crate) struct NodeTree<'g, N: Node<N>> {
  pub(crate) context: RenderContext<'g>,
  pub(crate) node: Option<N>,
  children: Option<Vec<NodeTree<'g, N>>>,
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
    if self.context.opacity == 0 {
      return Ok(());
    }

    let (inline_layout, _, boxes) = self.create_inline_layout(layout.content_box_size());
    let font_style = self.context.style.to_sized_font_style(&self.context);

    // Draw the inline layout without a callback first
    let positioned_inline_boxes =
      draw_inline_layout(&self.context, canvas, layout, inline_layout, &font_style)?;

    // Then handle the inline boxes directly by zipping the node refs with their positioned boxes
    for ((node, context, _), positioned) in boxes.iter().zip(positioned_inline_boxes.iter()) {
      draw_inline_box(positioned, *node, context, canvas, self.context.transform)?;
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

    // currentColor itself should NOT have opacity applied yet,
    // otherwise it will cause double applying.
    let current_color = style.color.resolve(parent_context.current_color, 255);

    let opacity = (style.opacity.0 * parent_context.opacity as f32) as u8;

    let mut context = RenderContext {
      style,
      current_color,
      opacity,
      fetched_resources: parent_context.fetched_resources.clone(),
      sizing: Sizing {
        font_size,
        ..parent_context.sizing
      },
      ..*parent_context
    };

    let children = node.take_children().map(|children| {
      children
        .into_iter()
        .map(|child| Self::from_node_impl(&context, child))
        .collect::<Vec<_>>()
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
      children: Some(final_children),
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

      let mut boxes = Vec::new();

      let (mut layout, _) =
        self
          .context
          .global
          .font_context
          .tree_builder((&font_style).into(), |builder| {
            let mut idx = 0;
            let mut index_pos = 0;

            for (item, context) in self.inline_items_iter() {
              match item {
                InlineItem::Text(text) => {
                  builder.push_style_span((&context.style.to_sized_font_style(context)).into());
                  builder.push_text(&text);
                  builder.pop_style_span();

                  index_pos += text.len();
                }
                InlineItem::Node(node) => {
                  let size = node.measure(
                    context,
                    available_space,
                    Size::NONE,
                    &taffy::Style::default(),
                  );

                  boxes.push(size);

                  builder.push_inline_box(InlineBox {
                    index: index_pos,
                    id: idx,
                    width: size.width,
                    height: size.height,
                  });

                  idx += 1;
                }
              }
            }
          });

      break_lines(&mut layout, max_width, max_height);

      let (max_run_width, total_height) =
        layout
          .lines()
          .fold((0.0, 0.0), |(max_run_width, total_height), line| {
            let metrics = line.metrics();
            (
              metrics.advance.max(max_run_width),
              total_height + metrics.line_height,
            )
          });

      return taffy::Size {
        width: max_run_width.ceil().min(max_width),
        height: total_height.ceil(),
      };
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

  pub(crate) fn create_inline_layout(
    &self,
    size: Size<f32>,
  ) -> (
    InlineLayout,
    String,
    Vec<(&N, &RenderContext<'g>, InlineBox)>,
  ) {
    let font_style = self.context.style.to_sized_font_style(&self.context);
    let mut boxes = Vec::new();
    let mut text_spans = Vec::new();

    let (mut layout, text) =
      self
        .context
        .global
        .font_context
        .tree_builder((&font_style).into(), |builder| {
          let mut index_pos = 0;

          for (item, context) in self.inline_items_iter() {
            match item {
              InlineItem::Text(text) => {
                let text_style = context.style.to_sized_font_style(context);

                builder.push_style_span((&text_style).into());
                builder.push_text(&text);
                builder.pop_style_span();

                index_pos += text.len();

                text_spans.push((text, text_style));
              }
              InlineItem::Node(node) => {
                let size = node.measure(
                  context,
                  Size {
                    width: AvailableSpace::Definite(size.width),
                    height: AvailableSpace::Definite(size.height),
                  },
                  Size::NONE,
                  &taffy::Style::default(),
                );

                let inline_box = InlineBox {
                  index: index_pos,
                  id: boxes.len() as u64,
                  width: size.width,
                  height: size.height,
                };

                builder.push_inline_box(inline_box.clone());

                boxes.push((node, context, inline_box));
              }
            }
          }
        });

    let max_height = match font_style.parent.line_clamp.as_ref() {
      Some(clamp) => Some(MaxHeight::HeightAndLines(size.height, clamp.count)),
      None => Some(MaxHeight::Absolute(size.height)),
    };

    break_lines(&mut layout, size.width, max_height);

    let should_handle_ellipsis = font_style.parent.should_handle_ellipsis();

    if let Some(last_line) = layout.lines().last() {
      let is_overflowing =
        last_line.text_range().end < text.len() || layout.inline_boxes().len() < boxes.len();

      if should_handle_ellipsis && is_overflowing {
        boxes.truncate(layout.inline_boxes().len());

        let mut text_length = 0;
        let mut spans_length = 0;

        for (text, _) in &text_spans {
          text_length += text.len();

          if text_length >= last_line.text_range().end {
            break;
          }

          spans_length += text.len();
        }

        text_spans.truncate(spans_length);

        layout = create_ellipsis_layout(
          self.context.global,
          &mut boxes,
          &mut text_spans,
          &font_style,
          size.width,
          max_height,
        );
      }
    }

    layout.align(
      Some(size.width),
      self.context.style.text_align.into(),
      Default::default(),
    );

    (layout, text, boxes)
  }

  fn inline_items_iter(&self) -> InlineItemIterator<'_, 'g, N> {
    InlineItemIterator {
      stack: vec![(self, 0)], // (node, depth)
      current_node_content: None,
    }
  }
}

fn create_ellipsis_layout<N: Node<N>>(
  global: &GlobalContext,
  boxes: &mut Vec<(&N, &RenderContext, InlineBox)>,
  text_spans: &mut Vec<(Cow<str>, SizedFontStyle)>,
  root_font_style: &SizedFontStyle,
  max_width: f32,
  max_height: Option<MaxHeight>,
) -> InlineLayout {
  loop {
    let (mut layout, text) = global
      .font_context
      .tree_builder(root_font_style.into(), |builder| {
        for (text, style) in text_spans.iter() {
          builder.push_style_span((style).into());
          builder.push_text(text);
          builder.pop_style_span();
        }

        for (_, _, inline_box) in boxes.iter() {
          builder.push_inline_box(inline_box.clone());
        }

        builder.push_text(root_font_style.parent.ellipsis_char());
      });

    break_lines(&mut layout, max_width, max_height);

    if text_spans.is_empty() && boxes.is_empty() {
      return layout;
    }

    if let Some(last_line) = layout.lines().last()
      && last_line.text_range().end == text.len()
    {
      return layout;
    }

    if boxes
      .last()
      .is_some_and(|(_, _, inline_box)| inline_box.index == text.len())
    {
      boxes.pop();
      continue;
    }

    let Some((last_span, _)) = text_spans.last_mut() else {
      return layout;
    };

    if let Some((char_idx, _)) = last_span.char_indices().last() {
      match last_span {
        Cow::Borrowed(span) => {
          *last_span = Cow::Borrowed(&span[..char_idx]);
        }
        Cow::Owned(span) => {
          span.truncate(char_idx);
        }
      }
    } else {
      text_spans.pop();
    }
  }
}

/// Iterator for traversing inline items in document order
pub(crate) struct InlineItemIterator<'n, 'g, N: Node<N>> {
  stack: Vec<(&'n NodeTree<'g, N>, usize)>, // (node, depth)
  current_node_content: Option<(InlineItem<'n, N>, &'n RenderContext<'g>)>,
}

impl<'n, 'g, N: Node<N>> Iterator for InlineItemIterator<'n, 'g, N> {
  type Item = (InlineItem<'n, N>, &'n RenderContext<'g>);

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      // If we have current node content to return, return it
      if let Some(content) = self.current_node_content.take() {
        return Some(content);
      }

      // Get the next node from the stack
      let (node, depth) = self.stack.pop()?;

      // Push children onto stack in reverse order (so they process in forward order)
      if let Some(children) = &node.children {
        for child in children.iter().rev() {
          self.stack.push((child, depth + 1));
        }
      }

      // Prepare the current node's content
      if let Some(inline_content) = node
        .node
        .as_ref()
        .and_then(|n| n.inline_content(&node.context))
      {
        match inline_content {
          InlineContentKind::Box => {
            if let Some(n) = &node.node {
              self.current_node_content = Some((InlineItem::Node(n), &node.context));
            }
          }
          InlineContentKind::Text(text) => {
            self.current_node_content = Some((InlineItem::Text(text.into()), &node.context));
          }
        }
      }
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
      children: Some(take(inline_group)),
      node: None,
    });
  }
}
