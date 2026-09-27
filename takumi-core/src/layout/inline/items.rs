//! Inline items flattened from a render subtree: text, spacers and boxes.

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Rect, Size},
  layout::{border::BorderProperties, node::Node, tree::RenderNode},
  style::{
    Color, Direction, Display, Float, Length, ResolvedVerticalAlign, Sides, SizingContext,
    SpacePair, WhiteSpaceCollapse,
  },
  text_processing::{COLLAPSIBLE_WHITESPACE, HORIZONTAL_WHITESPACE},
};
use parley::{InlineBox, InlineBoxKind};

use super::outline::InlineOutline;
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

impl RenderNode {
  /// How parley places the box standing in for this node.
  pub(super) fn inline_box_kind(&self) -> InlineBoxKind {
    if self.context.style.position.is_out_of_flow() {
      InlineBoxKind::OutOfFlow
    } else if self.context.style.float != Float::None {
      InlineBoxKind::CustomOutOfFlow
    } else {
      InlineBoxKind::InFlow
    }
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
#[derive(Clone)]
pub(crate) struct InlineDecoration {
  pub(crate) color: Color,
  pub(crate) padding: Rect<f32>,
  /// The border's widths, colours and styles; each fragment resolves its radii from `radius`.
  pub(crate) border: BorderProperties,
  /// The corner radii as specified, since a percentage resolves against each fragment.
  pub(crate) radius: Sides<SpacePair<Length>>,
  pub(crate) outline: Option<InlineOutline>,
  pub(crate) opacity: f32,
  /// The span's direction, which puts its start edge on the left or right.
  pub(crate) direction: Direction,
  /// The span's sizing. Runs at its font size set the fragment height (Blink sizes the box
  /// from its own text metrics); other sizes only when the span has no text of its own.
  pub(crate) sizing: SizingContext,
}

impl InlineDecoration {
  /// How far the span's fragments and outline reach past its glyph boxes.
  pub(crate) fn reach(&self) -> f32 {
    let widths = self.border.width;
    let edge = [
      self.padding.top + widths.top,
      self.padding.right + widths.right,
      self.padding.bottom + widths.bottom,
      self.padding.left + widths.left,
    ]
    .into_iter()
    .fold(0.0_f32, f32::max);

    edge + self.outline.map_or(0.0, |outline| outline.reach().max(0.0))
  }
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
  // The margins sit outside the span's own background, on its parent's.
  let outer_decorations = decorations;
  let decorations = own_decoration.as_ref().or(decorations);

  if let Some(marker) = node.marker.as_deref() {
    items.push(InlineItem::RenderNode {
      render_node: marker,
      decorations: None,
    });
  }

  let content_start = items.len();
  let (margin, border_padding) = inline_span_spacing(node, depth);

  if margin.left > 0.0 {
    items.push(InlineItem::Spacer {
      width: margin.left,
      decorations: outer_decorations.cloned(),
    });
  }
  if border_padding.left > 0.0 {
    items.push(InlineItem::Spacer {
      width: border_padding.left,
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

  if border_padding.right > 0.0 {
    items.push(InlineItem::Spacer {
      width: border_padding.right,
      decorations: decorations.cloned(),
    });
  }
  if margin.right > 0.0 {
    items.push(InlineItem::Spacer {
      width: margin.right,
      decorations: outer_decorations.cloned(),
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

/// The margins, and the borders with the padding, an inline span reserves on the line, each
/// side's only where it starts or ends.
///
/// Approximate: a negative margin reserves nothing, where Blink pulls the neighbouring content in.
fn inline_span_spacing(node: &RenderNode, depth: usize) -> (Rect<f32>, Rect<f32>) {
  if !is_inline_span(node, depth) {
    return Default::default();
  }

  let border = node.border_px();
  let padding = node.padding_px();

  (
    node.margin_px(),
    Rect {
      top: border.top + padding.top,
      right: border.right + padding.right,
      bottom: border.bottom + padding.bottom,
      left: border.left + padding.left,
    },
  )
}

/// The decoration an inline span paints, or `None` when it paints no background, border or
/// outline.
fn inline_span_decoration(node: &RenderNode, depth: usize) -> Option<InlineDecoration> {
  if !is_inline_span(node, depth) || !node.context.style.is_visible() {
    return None;
  }
  let style = &node.context.style;
  let color = style.background_color.resolve(node.context.current_color);
  let border = BorderProperties::from_context(&node.context, Size::ZERO, node.border_px());
  let outline = InlineOutline::of(&node.context);

  if color.0[3] == 0 && !border.has_visible_sides() && outline.is_none() {
    return None;
  }

  Some(InlineDecoration {
    color,
    padding: node.padding_px(),
    border,
    radius: Sides([
      style.border_top_left_radius,
      style.border_top_right_radius,
      style.border_bottom_right_radius,
      style.border_bottom_left_radius,
    ]),
    outline,
    opacity: style.opacity.0,
    direction: style.direction,
    sizing: node.context.sizing.clone(),
  })
}

pub(crate) enum InlineContentKind<'c> {
  Text(Cow<'c, str>),
  Box,
}

impl From<&InlineBoxItem<'_>> for ComputedLayout {
  fn from(value: &InlineBoxItem<'_>) -> Self {
    ComputedLayout::new(
      Point::ZERO,
      Size::new(value.paint_width, value.paint_height),
      value.border,
      value.padding,
    )
  }
}
