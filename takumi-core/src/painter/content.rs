//! What a node paints inside its box, besides its children.

use std::{borrow::Cow, rc::Rc};

use crate::{
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, ComputedLayout, Size},
  layout::{
    inline::{
      BuiltInlineLayout, FragmentItems, InlineItem, InlineLayoutMode, InlineLayoutRequest,
      InlineRunLayout, ProcessedInlineSpan, collect_inline_items, create_inline_layout,
      process_inline_spans,
    },
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
    if node.has_anonymous_text_item_child() {
      return Self::None;
    }

    match node.node.as_ref().map(|input| &input.kind) {
      Some(NodeKind::Text(_)) => Self::Inline(node),
      Some(NodeKind::Image(image)) if node.context.style.is_visible() => Self::Image(image),
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

  /// The inline content laid out in the content box of `layout`, styled by `font_style`, its
  /// node's own; `None` when it has none. A node keeps the fragment items it laid out to, so
  /// painting it again at the same layout only processes its spans.
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
    let items = self.inline_items(font_style)?;
    let context = &node.context;

    if let Some(fragment) = node.fragment_items_in(layout) {
      let content = layout.content_box_size();
      let spans = process_inline_spans(
        &items,
        Size {
          width: AvailableSpace::Definite(content.width),
          height: AvailableSpace::Definite(content.height),
        },
        context,
      );

      return Some(
        fragment
          .resolve_runs(&spans, context, layout)
          .map(|runs| PaintedInline {
            spans,
            runs,
            items: fragment,
          }),
      );
    }

    let built = create_inline_layout(InlineLayoutRequest::in_content_box(
      items,
      layout.content_box_size(),
      font_style,
      context,
      InlineLayoutMode::Draw,
    ));
    let fragment = Rc::new(built.fragment_items(layout));
    let runs = fragment.resolve_runs(&built.spans, context, layout);

    if !fragment.ellipsized {
      *node.fragment_items.borrow_mut() = Some(Rc::clone(&fragment));
    }
    let BuiltInlineLayout { spans, .. } = built;

    Some(runs.map(|runs| PaintedInline {
      spans,
      runs,
      items: fragment,
    }))
  }
}
