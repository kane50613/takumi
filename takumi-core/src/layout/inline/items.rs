//! Inline items flattened from a render subtree: text, spacers and boxes.

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Rect, Size},
  layout::{border::BorderProperties, node::Node, tree::RenderNode},
  style::{
    Color, Direction, Display, Float, Length, Position, ResolvedVerticalAlign, Sides,
    SizingContext, SpacePair, WhiteSpaceCollapse,
  },
  text_processing::{COLLAPSIBLE_WHITESPACE, HORIZONTAL_WHITESPACE},
};
use parley::{InlineBox, InlineBoxKind};
use smallvec::SmallVec;

use super::outline::InlineOutline;
use std::{borrow::Cow, ops::Range, rc::Rc, sync::Arc};

/// An out-of-flow box inside inline content.
#[derive(Clone)]
pub(crate) struct InlineOutOfFlow<'n> {
  /// Child-index path from the inline formatting context's root.
  pub(crate) path: SmallVec<[usize; 2]>,
  pub(crate) node: &'n RenderNode,
  /// The inline span that is its containing block, if one is.
  pub(crate) container: Option<&'n RenderNode>,
}

/// The innermost inline spans around a point in inline content that contain out-of-flow boxes.
#[derive(Clone, Copy, Default)]
struct InlineContainers<'n> {
  absolute: Option<&'n RenderNode>,
  fixed: Option<&'n RenderNode>,
}

impl<'n> InlineContainers<'n> {
  /// The containers inside `span`.
  fn within(self, span: &'n RenderNode) -> Self {
    Self {
      absolute: span
        .contains_absolute_as_inline()
        .then_some(span)
        .or(self.absolute),
      fixed: span
        .contains_fixed_as_inline()
        .then_some(span)
        .or(self.fixed),
    }
  }

  /// The span containing the out-of-flow `node`.
  fn of(self, node: &RenderNode) -> Option<&'n RenderNode> {
    match node.context.style.position {
      Position::Fixed => self.fixed,
      _ => self.absolute,
    }
  }
}

/// An inline box and its resolved box-model dimensions.
pub struct InlineBoxItem<'c> {
  /// The render node this box wraps.
  pub render_node: &'c RenderNode,
  /// Innermost enclosing decorated span, if any.
  pub(crate) decorations: Option<Rc<DecorationLink<'c>>>,
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
  /// Whether a float sits among the inline content this node lays out, found the way
  /// [`collect_inline_items`] walks it.
  pub fn has_inline_floats(&self) -> bool {
    self.children.iter().flatten().any(|child| {
      if child.participates_as_inline_box() {
        child.inline_box_kind() == InlineBoxKind::CustomOutOfFlow
      } else {
        child.has_inline_floats()
      }
    })
  }

  /// Whether an out-of-flow box sits among the inline content this node lays out, found the way
  /// [`collect_inline_items`] walks it.
  pub(crate) fn holds_inline_out_of_flow(&self) -> bool {
    self.children.iter().flatten().any(|child| {
      child.is_out_of_flow() || (child.is_inline_span() && child.holds_inline_out_of_flow())
    })
  }

  /// The out-of-flow boxes among the inline content this node lays out, found the way
  /// [`collect_inline_items`] walks it.
  pub(crate) fn inline_out_of_flow(&self) -> Vec<InlineOutOfFlow<'_>> {
    fn visit<'n>(
      node: &'n RenderNode,
      path: &mut SmallVec<[usize; 2]>,
      containers: InlineContainers<'n>,
      found: &mut Vec<InlineOutOfFlow<'n>>,
    ) {
      for (index, child) in node.children.iter().flatten().enumerate() {
        path.push(index);
        if child.is_out_of_flow() {
          found.push(InlineOutOfFlow {
            path: path.clone(),
            node: child,
            container: containers.of(child),
          });
        } else if child.is_inline_span() {
          visit(child, path, containers.within(child), found);
        }
        path.pop();
      }
    }

    let mut found = Vec::new();

    visit(
      self,
      &mut SmallVec::new(),
      InlineContainers::default(),
      &mut found,
    );
    found
  }

  /// Whether the node is an inline box whose content joins its parent's lines rather than an
  /// atomic box of its own.
  fn is_inline_span(&self) -> bool {
    self.context.style.display.is_inline() && !self.participates_as_inline_box()
  }

  /// Whether this inline span is the containing block of fixed-position descendants, as Blink's
  /// `LayoutObject::ComputeIsFixedContainer` decides for a box that is not atomic.
  fn contains_fixed_as_inline(&self) -> bool {
    let style = &self.context.style;

    !style.filter.is_empty() || !style.backdrop_filter.is_empty()
  }

  /// Whether this inline span is the containing block of absolutely positioned descendants, as
  /// Blink's `LayoutObject::ComputeIsAbsoluteContainer` decides.
  fn contains_absolute_as_inline(&self) -> bool {
    self.context.style.position.is_positioned() || self.contains_fixed_as_inline()
  }

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
    decorations: Option<Rc<DecorationLink<'c>>>,
  },
  /// An inline box.
  Box(InlineBoxItem<'c>),
  /// A zero-height box reserving an inline span's horizontal padding.
  Spacer {
    /// The box the spacer occupies in the layout.
    inline_box: InlineBox,
    /// Innermost enclosing decorated span, if any.
    decorations: Option<Rc<DecorationLink<'c>>>,
  },
}

/// The box decoration a `display: inline` span paints along its line
/// fragments, resolved from its computed style.
#[derive(Clone)]
pub(crate) struct InlineDecoration<'c> {
  /// The span, whose fragments also bound the out-of-flow boxes it contains.
  pub(crate) owner: &'c RenderNode,
  /// Whether the fragments paint, which a span kept only as a containing block does not.
  pub(crate) paints: bool,
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

impl InlineDecoration<'_> {
  /// How far the span's fragments and outline reach past its glyph boxes.
  pub(crate) fn reach(&self) -> f32 {
    if !self.paints {
      return 0.0;
    }

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
pub struct DecorationLink<'c> {
  pub(crate) decoration: InlineDecoration<'c>,
  pub(crate) parent: Option<Rc<DecorationLink<'c>>>,
}

/// A piece of inline content collected from the tree.
pub enum InlineItem<'c> {
  /// An inline-level render node.
  RenderNode {
    /// The node.
    render_node: &'c RenderNode,
    /// Innermost enclosing decorated span, if any.
    decorations: Option<Rc<DecorationLink<'c>>>,
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
    decorations: Option<Rc<DecorationLink<'c>>>,
  },
  /// Advance an inline span's horizontal padding reserves at its edge.
  Spacer {
    /// The padding width in px.
    width: f32,
    /// Innermost enclosing decorated span (the padded span itself when it is decorated), if any.
    decorations: Option<Rc<DecorationLink<'c>>>,
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
  decorations: Option<&Rc<DecorationLink<'n>>>,
  items: &mut Vec<InlineItem<'n>>,
) {
  if depth > 0 && (node.participates_as_inline_box() || node.is_out_of_flow()) {
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
/// outline and cannot be the containing block of out-of-flow boxes.
fn inline_span_decoration(node: &RenderNode, depth: usize) -> Option<InlineDecoration<'_>> {
  if !is_inline_span(node, depth) {
    return None;
  }
  let style = &node.context.style;
  let color = style.background_color.resolve(node.context.current_color);
  let border = BorderProperties::from_context(&node.context, Size::ZERO, node.border_px());
  let outline = InlineOutline::of(&node.context);
  let paints =
    style.is_visible() && (color.0[3] != 0 || border.has_visible_sides() || outline.is_some());

  if !paints && !node.contains_absolute_as_inline() {
    return None;
  }

  Some(InlineDecoration {
    owner: node,
    paints,
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
