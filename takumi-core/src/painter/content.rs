//! What a node paints inside its box, besides its children.

use std::borrow::Cow;

use crate::{
  font_style::SizedFontStyle,
  layout::{
    inline::{InlineItem, collect_inline_items},
    node::{ImageData, NodeKind},
    tree::RenderNode,
  },
};

/// What a node paints inside its box decorations.
pub enum OwnContent<'n> {
  /// Lines of inline content: the node's inline formatting context, or a text node's own text.
  Inline(&'n RenderNode),
  /// A replaced image.
  Image(&'n ImageData),
  /// Nothing, including a box whose text an anonymous child box lays out.
  None,
}

impl<'n> OwnContent<'n> {
  /// What `node` paints inside its box.
  pub fn of(node: &'n RenderNode) -> Self {
    if node.should_create_inline_layout() {
      return Self::Inline(node);
    }
    if node.has_anonymous_text_item_child() {
      return Self::None;
    }

    match node.node.as_ref().map(|input| &input.kind) {
      Some(NodeKind::Text(_)) => Self::Inline(node),
      Some(NodeKind::Image(image)) => Self::Image(image),
      _ => Self::None,
    }
  }

  /// The items the inline content lays out in the node's own `font_style`, or `None` when it
  /// has none. A text node at `font-size: 0` shows nothing, but an inline formatting context
  /// still lays out the children that set their own size.
  pub fn inline_items(&self, font_style: &SizedFontStyle) -> Option<Vec<InlineItem<'n>>> {
    let Self::Inline(node) = *self else {
      return None;
    };

    if node.should_create_inline_layout() {
      return Some(collect_inline_items(node));
    }

    match &node.node.as_ref()?.kind {
      NodeKind::Text(text) if font_style.sizing.font_size != 0.0 => Some(vec![InlineItem::Text {
        text: Cow::Borrowed(text.text.as_str()),
        context: &node.context,
        link: None,
        decorations: None,
      }]),
      _ => None,
    }
  }
}
