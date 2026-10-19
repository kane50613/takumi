//! Inline items flattened from a render subtree: text, spacers and boxes.

use std::{borrow::Cow, iter::successors, ops::Range, ptr, rc::Rc, sync::Arc};

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use super::{
  decorations::BoxFont, inline_item::InlineItemType, inline_node_data::InlineNodeData,
  metrics::InlineBoxKind, outline::InlineOutline,
};
use crate::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Rect, Size},
  layout::{border::BorderProperties, node::Node, tree::RenderNode},
  layout_unit::LayoutUnit,
  style::{Color, Direction, Display, Float, Length, Position, Sides, SizingContext, SpacePair},
};

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
    match node.context.style.base_data.position {
      Position::Fixed => self.fixed,
      _ => self.absolute,
    }
  }
}

/// An inline box and its resolved box-model dimensions.
pub struct InlineBoxItem<'c> {
  /// The render node this box wraps.
  pub render_node: &'c RenderNode,
  /// Innermost enclosing inline span, if any.
  pub(crate) decorations: Option<Rc<DecorationLink<'c>>>,
  pub(crate) paint_width: f32,
  pub(crate) paint_height: f32,
  /// Margin around the box.
  pub margin: Rect<f32>,
  pub(crate) padding: Rect<f32>,
  pub(crate) border: Rect<f32>,
  pub(crate) baseline_offset: Option<f32>,
}

impl RenderNode {
  /// Whether a float sits among the inline content this node lays out, found the way
  /// [`InlineNode::collect_inlines`](super::inline_node::InlineNode::collect_inlines) walks it.
  pub(crate) fn has_inline_floats(&self) -> bool {
    self.children.iter().flatten().any(|child| {
      if child.participates_as_inline_box() {
        child.inline_box_kind() == InlineBoxKind::Float
      } else {
        child.has_inline_floats()
      }
    })
  }

  /// Whether an out-of-flow box sits among the inline content this node lays out, found the way
  /// [`InlineNode::collect_inlines`](super::inline_node::InlineNode::collect_inlines) walks it.
  pub(crate) fn holds_inline_out_of_flow(&self) -> bool {
    self.children.iter().flatten().any(|child| {
      child.is_out_of_flow() || (child.is_inline_span() && child.holds_inline_out_of_flow())
    })
  }

  /// The out-of-flow boxes among the inline content this node lays out, found the way
  /// [`InlineNode::collect_inlines`](super::inline_node::InlineNode::collect_inlines) walks it.
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
    self.context.style.base_data.display.is_inline() && !self.participates_as_inline_box()
  }

  /// Whether this inline span is the containing block of fixed-position descendants, as Blink's
  /// `LayoutObject::ComputeIsFixedContainer` decides for a box that is not atomic.
  fn contains_fixed_as_inline(&self) -> bool {
    let style = &self.context.style;

    !style.misc1_data.filter.is_empty() || !style.misc1_data.backdrop_filter.is_empty()
  }

  /// Whether this inline span is the containing block of absolutely positioned descendants, as
  /// Blink's `LayoutObject::ComputeIsAbsoluteContainer` decides.
  fn contains_absolute_as_inline(&self) -> bool {
    self.context.style.base_data.position.is_positioned() || self.contains_fixed_as_inline()
  }

  /// How the box standing in for this node sits in its line.
  pub(super) fn inline_box_kind(&self) -> InlineBoxKind {
    if self.context.style.base_data.position.is_out_of_flow() {
      InlineBoxKind::OutOfFlow
    } else if self.context.style.base_data.float != Float::None {
      InlineBoxKind::Float
    } else {
      InlineBoxKind::InFlow
    }
  }
}

/// An inline item after text processing, ready for layout.
pub enum ProcessedInlineSpan<'c> {
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
    /// Innermost enclosing inline span, if any.
    decorations: Option<Rc<DecorationLink<'c>>>,
  },
  /// An inline box.
  Box(InlineBoxItem<'c>),
}

impl<'c> ProcessedInlineSpan<'c> {
  /// The spans around a text span, innermost first.
  pub(crate) fn text_chain(&self) -> Option<&Rc<DecorationLink<'c>>> {
    match self {
      Self::Text { decorations, .. } => decorations.as_ref(),
      _ => None,
    }
  }

  /// The spans around this one, innermost first.
  pub(crate) fn chain(&self) -> Option<&Rc<DecorationLink<'c>>> {
    match self {
      Self::Text { decorations, .. } => decorations.as_ref(),
      Self::Box(item) => item.decorations.as_ref(),
    }
  }
}

/// The inline box a `display: inline` span opens: what its line fragments paint and how far it
/// grows the lines it is open on, resolved from its computed style.
#[derive(Clone)]
pub(crate) struct InlineDecoration<'c> {
  /// The span, whose fragments also bound the out-of-flow boxes it contains.
  pub(crate) owner: &'c RenderNode,
  /// Unique among the spans one collection opens.
  pub(crate) id: usize,
  /// The font the span's fragments and its children align by.
  pub(crate) font: BoxFont,
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
  /// The span's sizing, which each fragment resolves its radii against.
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

/// One open inline span in the chain of span ancestors around an inline item,
/// innermost last. Chains share their tails, so the `Rc` pointer identifies the
/// span across items.
pub struct DecorationLink<'c> {
  pub(crate) decoration: InlineDecoration<'c>,
  pub(crate) parent: Option<Rc<DecorationLink<'c>>>,
}

impl<'c> DecorationLink<'c> {
  /// This span and the spans around it, innermost first.
  pub(crate) fn ancestors(&self) -> impl Iterator<Item = &DecorationLink<'c>> {
    successors(Some(self), |link| link.parent.as_deref())
  }
}

/// Whether this node is a non-replaced `display: inline` span (the inline formatting context's root
/// does not count).
fn is_inline_span(node: &RenderNode, depth: usize) -> bool {
  depth > 0
    && node.context.style.base_data.display == Display::Inline
    && !matches!(
      node.node.as_ref().and_then(Node::inline_content),
      Some(InlineContentKind::Box)
    )
}

/// The inline box an inline span opens with `id`, or `None` for a node that is not one.
fn inline_span_decoration(
  node: &RenderNode,
  depth: usize,
  id: usize,
) -> Option<InlineDecoration<'_>> {
  if !is_inline_span(node, depth) {
    return None;
  }
  let style = &node.context.style;
  let color = style
    .background_data
    .background_color
    .resolve(node.context.current_color);
  let border = BorderProperties::from_context(&node.context, Size::ZERO, node.border_px());
  let outline = InlineOutline::of(&node.context);
  let has_images = style
    .background_data
    .background_image
    .as_deref()
    .is_some_and(|images| !images.is_empty());
  let paints = style.is_visible()
    && (color.0[3] != 0 || has_images || border.has_visible_sides() || outline.is_some());

  Some(InlineDecoration {
    owner: node,
    id,
    font: BoxFont::of(&node.context),
    paints,
    color,
    padding: node.padding_px(),
    border,
    radius: Sides([
      style.surround_data.border_top_left_radius,
      style.surround_data.border_top_right_radius,
      style.surround_data.border_bottom_right_radius,
      style.surround_data.border_bottom_left_radius,
    ]),
    outline,
    opacity: style.svg_data.opacity.0,
    direction: style.inherited_data.direction,
    sizing: node.context.sizing.clone(),
  })
}

pub(crate) enum InlineContentKind<'c> {
  Text(Cow<'c, str>),
  Box,
}

impl InlineBoxItem<'_> {
  /// Paints the box around a border box `inline_size` by `block_size`; its margin box's size.
  pub(crate) fn paint_border_box(
    &mut self,
    inline_size: LayoutUnit,
    block_size: LayoutUnit,
  ) -> Size<f32> {
    self.paint_width = inline_size.to_float() + self.margin.left + self.margin.right;
    self.paint_height = block_size.to_float() + self.margin.top + self.margin.bottom;

    Size {
      width: self.paint_width,
      height: self.paint_height,
    }
  }
}

/// The box's border box: its line footprint less its margins.
impl From<&InlineBoxItem<'_>> for ComputedLayout {
  fn from(value: &InlineBoxItem<'_>) -> Self {
    ComputedLayout::new(
      Point::ZERO,
      Size::new(
        (value.paint_width - value.margin.horizontal()).max(0.0),
        (value.paint_height - value.margin.vertical()).max(0.0),
      ),
      value.border,
      value.padding,
    )
  }
}

/// The spans painting reads for an inline formatting context the inline layout port laid out:
/// a text span per text, and a box span per atomic inline, float, and out-of-flow box, each with
/// the spans and the link around it.
pub(crate) struct InlineSpans<'c> {
  pub(crate) spans: Vec<ProcessedInlineSpan<'c>>,
  /// The span of each item, by item index.
  pub(crate) item_spans: Vec<Option<usize>>,
  /// The first span of each node, for a fragment an item no longer holds, such as an ellipsis or
  /// a laid out atomic inline.
  node_spans: FxHashMap<*const RenderNode, usize>,
  /// The decoration each inline box opens, by its node.
  node_decorations: FxHashMap<*const RenderNode, Rc<DecorationLink<'c>>>,
}

impl<'c> InlineSpans<'c> {
  /// The spans of the items `block` lays out.
  pub(crate) fn of(block: &'c RenderNode, items_data: &InlineNodeData<'c>) -> Self {
    let mut spans: Vec<ProcessedInlineSpan<'c>> = Vec::new();
    let mut item_spans = Vec::with_capacity(items_data.items.len());
    // The spans and links open at each item, the formatting context's own first.
    let mut chains: Vec<Option<Rc<DecorationLink<'c>>>> = vec![None];
    let mut links: Vec<Option<Arc<str>>> = vec![link_of(block)];
    let mut next_span = 0;
    let mut last_text: Option<(*const RenderNode, usize)> = None;
    let mut node_spans = FxHashMap::default();
    let mut node_decorations = FxHashMap::default();

    for item in &items_data.items {
      let chain = chains.last().cloned().flatten();
      let link = links.last().cloned().flatten();
      let Some(object) = item.layout_object else {
        item_spans.push(None);

        continue;
      };
      let node = object.node();
      let span = match item.item_type {
        InlineItemType::OpenTag => {
          let decoration =
            inline_span_decoration(node, chains.len(), next_span).map(|decoration| {
              next_span += 1;

              Rc::new(DecorationLink {
                decoration,
                parent: chain.clone(),
              })
            });

          if let Some(decoration) = &decoration {
            node_decorations.insert(ptr::from_ref(node), Rc::clone(decoration));
          }

          chains.push(decoration.or(chain));
          links.push(link_of(node).or(link));
          None
        }
        InlineItemType::CloseTag => {
          if chains.len() > 1 {
            chains.pop();
            links.pop();
          }

          None
        }
        InlineItemType::Text | InlineItemType::Control => {
          let range = item.start_offset()..item.end_offset();
          let span_index = match last_text {
            Some((last_node, index)) if ptr::eq(last_node, node) => {
              if let Some(ProcessedInlineSpan::Text {
                byte_range, text, ..
              }) = spans.get_mut(index)
              {
                byte_range.end = range.end;
                text.push_str(&items_data.text_content[range]);
              }

              index
            }
            _ => {
              spans.push(ProcessedInlineSpan::Text {
                text: items_data.text_content[range.clone()].to_owned(),
                byte_range: range,
                style: Box::new(SizedFontStyle::from_style(
                  &node.context.style,
                  &node.context,
                )),
                link: link_of(node).or(link),
                decorations: chain,
              });
              node_spans
                .entry(ptr::from_ref(node))
                .or_insert(spans.len() - 1);
              spans.len() - 1
            }
          };

          last_text = Some((ptr::from_ref(node), span_index));
          Some(span_index)
        }
        InlineItemType::AtomicInline
        | InlineItemType::Floating
        | InlineItemType::OutOfFlowPositioned
        | InlineItemType::ListMarker => {
          spans.push(ProcessedInlineSpan::Box(InlineBoxItem {
            render_node: node,
            decorations: chain,
            paint_width: 0.0,
            paint_height: 0.0,
            margin: node.margin_px(),
            padding: node.padding_px(),
            border: node.border_px(),
            baseline_offset: None,
          }));
          node_spans
            .entry(ptr::from_ref(node))
            .or_insert(spans.len() - 1);
          Some(spans.len() - 1)
        }
        InlineItemType::BidiControl => None,
      };

      item_spans.push(span);
    }

    Self {
      spans,
      item_spans,
      node_spans,
      node_decorations,
    }
  }

  /// The decoration the inline box `node` opens.
  pub(crate) fn decoration_of(&self, node: &RenderNode) -> Option<&Rc<DecorationLink<'c>>> {
    self.node_decorations.get(&ptr::from_ref(node))
  }

  /// The first span of `node`.
  pub(crate) fn span_of(&self, node: &RenderNode) -> Option<usize> {
    self.node_spans.get(&ptr::from_ref(node)).copied()
  }
}

/// The `href` of `node`, if it is an anchor.
fn link_of(node: &RenderNode) -> Option<Arc<str>> {
  node
    .node
    .as_ref()
    .and_then(Node::href)
    .map(Arc::<str>::from)
}
