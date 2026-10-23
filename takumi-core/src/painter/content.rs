//! What a node paints inside its box, besides its children.

use std::rc::Rc;

use crate::{
  font_style::SizedFontStyle,
  geometry::ComputedLayout,
  layout::{
    inline::{FragmentItems, InlineRunLayout, ProcessedInlineSpan},
    node::{ImageData, NodeKind},
    tree::RenderNode,
  },
  resources::font::FontError,
};

/// A node's inline content laid out in its content box for painting.
pub struct PaintedInline<'c> {
  /// The spans its items process into.
  pub spans: Vec<ProcessedInlineSpan<'c>>,
  /// Its runs, inline boxes and span fragments, resolved for painting.
  pub runs: InlineRunLayout<'c>,
  items: Rc<FragmentItems>,
}

impl PaintedInline<'_> {
  /// The text it laid out.
  pub fn text(&self) -> &str {
    &self.items.text
  }
}

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

    match node.node.as_ref().map(|input| &input.kind) {
      Some(NodeKind::Text(_) | NodeKind::Image(_)) if node.has_anonymous_text_item_child() => {
        Self::None
      }
      Some(NodeKind::Text(_)) => Self::Inline(node),
      Some(NodeKind::Image(image)) if node.context.style.is_visible() => Self::Image(image),
      _ => Self::None,
    }
  }

  /// Whether the inline content lays anything out in the node's own `font_style`. A text node at
  /// `font-size: 0` shows nothing, but an inline formatting context still lays out the children
  /// that set their own size.
  pub(crate) fn has_inline_content(&self, font_style: &SizedFontStyle) -> bool {
    let Self::Inline(node) = *self else {
      return false;
    };

    node.should_create_inline_layout()
      || matches!(
        node.node.as_ref().map(|input| &input.kind),
        Some(NodeKind::Text(_)) if font_style.sizing.font_size != 0.0
      )
  }

  /// The fragment items the inline content lays out to in the content box of `layout`, styled
  /// by `font_style`, its node's own, without resolving a glyph; `None` when it has none.
  pub fn fragment_items(
    &self,
    font_style: &SizedFontStyle<'_>,
    layout: ComputedLayout,
  ) -> Option<Rc<FragmentItems>> {
    let Self::Inline(node) = *self else {
      return None;
    };

    if !self.has_inline_content(font_style) {
      return None;
    }

    node.fragment_items_in(layout)
  }

  /// The inline content laid out in the content box of `layout`, styled by `font_style`, its
  /// node's own; `None` when it has none. A node keeps the fragment items it laid out to, so
  /// painting it again at the same layout only collects its spans.
  pub fn lay_out_inline<'c>(
    &self,
    font_style: &'c SizedFontStyle<'c>,
    layout: ComputedLayout,
  ) -> Option<Result<PaintedInline<'c>, FontError>>
  where
    'n: 'c,
  {
    let Self::Inline(node) = *self else {
      return None;
    };
    let items = self.fragment_items(font_style, layout)?;
    let spans = node.inline_spans(&items);

    Some(
      items
        .resolve_runs(&spans, &node.context, layout)
        .map(|runs| PaintedInline {
          spans: spans.spans,
          runs,
          items,
        }),
    )
  }
}
