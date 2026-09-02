//! Inline items flattened from a render subtree: text, spacers and boxes.

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Rect, Size},
  layout::{node::Node, tree::RenderNode},
  style::{
    Color, Direction, Display, Float, Length, ResolvedVerticalAlign, SpacePair, WhiteSpaceCollapse,
  },
};
use parley::{InlineBox, InlineBoxKind};
use std::{borrow::Cow, ops::Range, rc::Rc, sync::Arc};

/// An inline box and its resolved box-model dimensions.
pub struct InlineBoxItem<'c> {
  /// The render node this box wraps.
  pub render_node: &'c RenderNode,
  /// Innermost enclosing decorated span, if any.
  pub(crate) decorations: Option<Rc<DecorationLink>>,
  pub(crate) inline_box: InlineBox,
  pub(crate) paint_width: f32,
  pub(crate) paint_height: f32,
  /// Margin around the box.
  pub margin: Rect<f32>,
  pub(crate) padding: Rect<f32>,
  pub(crate) border: Rect<f32>,
  pub(crate) baseline_offset: Option<f32>,
  pub(crate) vertical_align: ResolvedVerticalAlign,
}

pub(super) fn inline_box_kind(render_node: &RenderNode) -> InlineBoxKind {
  if render_node.context.style.position.is_out_of_flow() {
    InlineBoxKind::OutOfFlow
  } else if render_node.context.style.float != Float::None {
    InlineBoxKind::CustomOutOfFlow
  } else {
    InlineBoxKind::InFlow
  }
}

/// An inline item after text processing, ready for layout.
pub enum ProcessedInlineSpan<'c> {
  /// The synthetic direction mark leading the paragraph.
  DirectionMark {
    /// Base direction the mark forces.
    direction: Direction,
    /// Resolved font style, borrowed from the first text span.
    style: Box<SizedFontStyle<'c>>,
  },
  /// A styled text span.
  Text {
    /// Byte range within the laid-out text.
    byte_range: Range<usize>,
    /// Processed text content.
    text: String,
    /// Resolved font style.
    style: Box<SizedFontStyle<'c>>,
    /// URI of the nearest enclosing anchor's `href`, if any.
    link: Option<Arc<str>>,
    /// Innermost enclosing decorated span, if any.
    decorations: Option<Rc<DecorationLink>>,
  },
  /// An inline box.
  Box(InlineBoxItem<'c>),
  /// A zero-height box reserving an inline span's horizontal padding.
  Spacer {
    /// The box the spacer occupies in the layout.
    inline_box: InlineBox,
    /// Innermost enclosing decorated span, if any.
    decorations: Option<Rc<DecorationLink>>,
  },
}

/// The box decoration a `display: inline` span paints along its line
/// fragments, resolved from its computed style.
#[derive(Clone, Copy)]
pub(crate) struct InlineDecoration {
  pub(crate) color: Color,
  pub(crate) padding: Rect<f32>,
  /// Corner radii as `(x, y)` pairs in paint order: top-left, top-right,
  /// bottom-right, bottom-left.
  pub(crate) radii: [(f32, f32); 4],
  pub(crate) opacity: f32,
  /// Whether the span's start edge sits on the right.
  pub(crate) rtl: bool,
  /// The span's own font size. Runs at this size set the fragment height
  /// (Blink sizes the box from its own text metrics); other sizes only when
  /// the span has no text of its own.
  pub(crate) font_size: f32,
}

/// One open decorated span in the chain of decorated ancestors around an
/// inline item, innermost last. Chains share their tails, so the `Rc` pointer
/// identifies the span across items.
pub struct DecorationLink {
  pub(crate) decoration: InlineDecoration,
  pub(crate) parent: Option<Rc<DecorationLink>>,
}

/// A piece of inline content collected from the tree.
pub enum InlineItem<'c> {
  /// An inline-level render node.
  RenderNode {
    /// The node.
    render_node: &'c RenderNode,
    /// Innermost enclosing decorated span, if any.
    decorations: Option<Rc<DecorationLink>>,
  },
  /// A run of text.
  Text {
    /// The text content.
    text: Cow<'c, str>,
    /// Render context for the text.
    context: &'c RenderContext,
    /// URI of the nearest enclosing anchor's `href`, if any.
    link: Option<Arc<str>>,
    /// Innermost enclosing decorated span, if any.
    decorations: Option<Rc<DecorationLink>>,
  },
  /// Advance an inline span's horizontal padding reserves at its edge.
  Spacer {
    /// The padding width in px.
    width: f32,
    /// Innermost enclosing decorated span (the padded span itself when it is decorated), if any.
    decorations: Option<Rc<DecorationLink>>,
  },
}

/// Flatten a render node subtree into its inline items.
pub fn collect_inline_items<'n>(root: &'n RenderNode) -> Vec<InlineItem<'n>> {
  let mut items = Vec::new();
  collect_inline_items_impl(root, 0, None, None, &mut items);
  items
}

/// The whitespace CSS collapses, matching [`crate::layout::node::Node::is_whitespace_only_text`].
const COLLAPSIBLE_WHITESPACE: [char; 5] = [' ', '\t', '\n', '\r', '\u{c}'];

/// The subset `white-space-collapse: preserve-breaks` still collapses.
const HORIZONTAL_WHITESPACE: [char; 3] = [' ', '\t', '\u{c}'];

/// A marker holds the start of the line, so the whitespace that a line start
/// would have collapsed is collapsed against the marker instead.
fn trim_leading_whitespace(item: &mut InlineItem<'_>) {
  let InlineItem::Text { text, context, .. } = item else {
    return;
  };

  // `preserve-breaks` keeps its newlines but still collapses spaces and tabs.
  let collapsible: &[char] = match context.style.white_space_collapse {
    WhiteSpaceCollapse::Collapse => &COLLAPSIBLE_WHITESPACE,
    WhiteSpaceCollapse::PreserveBreaks => &HORIZONTAL_WHITESPACE,
    WhiteSpaceCollapse::Preserve | WhiteSpaceCollapse::PreserveSpaces => return,
  };

  let trimmed = text.trim_start_matches(collapsible);
  if trimmed.len() != text.len() {
    *text = Cow::Owned(trimmed.to_owned());
  }
}

fn collect_inline_items_impl<'n>(
  node: &'n RenderNode,
  depth: usize,
  link: Option<&Arc<str>>,
  decorations: Option<&Rc<DecorationLink>>,
  items: &mut Vec<InlineItem<'n>>,
) {
  if depth > 0 && node.participates_as_inline_box() {
    items.push(InlineItem::RenderNode {
      render_node: node,
      decorations: decorations.cloned(),
    });
    return;
  }
  let anchor = node
    .node
    .as_ref()
    .and_then(Node::href)
    .map(Arc::<str>::from);
  let link = anchor.as_ref().or(link);
  let own_decoration = inline_span_decoration(node, depth).map(|decoration| {
    Rc::new(DecorationLink {
      decoration,
      parent: decorations.cloned(),
    })
  });
  let decorations = own_decoration.as_ref().or(decorations);

  if let Some(marker) = node.marker.as_deref() {
    items.push(InlineItem::RenderNode {
      render_node: marker,
      decorations: None,
    });
  }

  let content_start = items.len();
  let padding = inline_span_padding(node, depth);

  if padding.left > 0.0 {
    items.push(InlineItem::Spacer {
      width: padding.left,
      decorations: decorations.cloned(),
    });
  }

  if let Some(text) = node.anonymous_text_content.as_deref() {
    items.push(InlineItem::Text {
      text: Cow::Borrowed(text),
      context: &node.context,
      link: link.cloned(),
      decorations: decorations.cloned(),
    });
  }

  if let Some(inline_content) = node.node.as_ref().and_then(Node::inline_content) {
    match inline_content {
      InlineContentKind::Box => items.push(InlineItem::RenderNode {
        render_node: node,
        decorations: decorations.cloned(),
      }),
      InlineContentKind::Text(text) => items.push(InlineItem::Text {
        text,
        context: &node.context,
        link: link.cloned(),
        decorations: decorations.cloned(),
      }),
    }
  }

  if let Some(children) = &node.children {
    for child in children {
      collect_inline_items_impl(child, depth + 1, link, decorations, items);
    }
  }

  if padding.right > 0.0 {
    items.push(InlineItem::Spacer {
      width: padding.right,
      decorations: decorations.cloned(),
    });
  }

  if node.marker.is_some()
    && let Some(first) = items.get_mut(content_start)
  {
    trim_leading_whitespace(first);
  }
}

/// Whether this node is a non-replaced `display: inline` span (the inline formatting context's root
/// does not count).
fn is_inline_span(node: &RenderNode, depth: usize) -> bool {
  depth > 0
    && node.context.style.display == Display::Inline
    && !matches!(
      node.node.as_ref().and_then(Node::inline_content),
      Some(InlineContentKind::Box)
    )
}

/// The horizontal padding an inline span reserves on the line.
fn inline_span_padding(node: &RenderNode, depth: usize) -> Rect<f32> {
  if !is_inline_span(node, depth) {
    return Rect::default();
  }
  let sizing = &node.context.sizing;

  Rect {
    top: node.context.style.padding_top.to_px(sizing, 0.0),
    right: node.context.style.padding_right.to_px(sizing, 0.0),
    bottom: node.context.style.padding_bottom.to_px(sizing, 0.0),
    left: node.context.style.padding_left.to_px(sizing, 0.0),
  }
}

/// The decoration an inline span paints, or `None` when its background is invisible.
fn inline_span_decoration(node: &RenderNode, depth: usize) -> Option<InlineDecoration> {
  if !is_inline_span(node, depth) {
    return None;
  }
  let style = &node.context.style;
  let color = style.background_color.resolve(node.context.current_color);

  if color.0[3] == 0 {
    return None;
  }
  let sizing = &node.context.sizing;
  let radius = |pair: &SpacePair<Length>| {
    (
      pair.x.to_px(sizing, 0.0).max(0.0),
      pair.y.to_px(sizing, 0.0).max(0.0),
    )
  };

  Some(InlineDecoration {
    color,
    padding: inline_span_padding(node, depth),
    radii: [
      radius(&style.border_top_left_radius),
      radius(&style.border_top_right_radius),
      radius(&style.border_bottom_right_radius),
      radius(&style.border_bottom_left_radius),
    ],
    opacity: style.opacity.0,
    rtl: style.direction == Direction::Rtl,
    font_size: sizing.font_size,
  })
}

pub(crate) enum InlineContentKind<'c> {
  Text(Cow<'c, str>),
  Box,
}

impl From<&InlineBoxItem<'_>> for ComputedLayout {
  fn from(value: &InlineBoxItem<'_>) -> Self {
    ComputedLayout {
      location: Point::ZERO,
      size: Size::new(value.paint_width, value.paint_height),
      border: value.border,
      padding: value.padding,
    }
  }
}
