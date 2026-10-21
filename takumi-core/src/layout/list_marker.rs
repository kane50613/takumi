use std::rc::Rc;

use crate::{
  context::RenderContext,
  geometry::{Point, Size},
  layout::{
    layout_style::LayoutLength,
    node::{Node, resolve_image},
    tree::{NodeOrigin, RenderNode},
  },
  layout_unit::LayoutUnit,
  matching::MatchedDeclarationsView,
  style::{
    BackgroundImage, Direction, Display, Length, ListStylePosition, ListStyleType, MakeComputed,
    TextWrapMode, WhiteSpaceCollapse,
  },
};

/// Blink's `kCMarkerPaddingPx`: the gap between a marker image and the item's content.
const MARKER_GAP_PX: f32 = 7.0;

/// Blink's `kCUAMarkerMarginEm`: the gap an inside symbol keeps from the
/// content that follows it on the line.
const INSIDE_SYMBOL_GAP_EM: f32 = 1.0;

/// Blink's `kClosureMarkerMarginEm`: the gap an inside disclosure triangle keeps.
const INSIDE_DISCLOSURE_GAP_EM: f32 = 0.4;

/// Blink's `DisclosureSymbolSize`, in ems.
const DISCLOSURE_SYMBOL_SIZE_EM: f32 = 0.66;

/// The marker box of a `display: list-item` box, per css-lists-3 §3: Blink's
/// `LayoutOutsideListMarker`, an atomic inline, or `LayoutInsideListMarker`, an inline box.
pub(super) fn list_marker(item_context: &RenderContext, ordinal: i32) -> Option<RenderNode> {
  let direction = item_context.style.inherited_data.direction;
  let position = item_context.style.inherited_data.list_style_position;
  let style_type = &item_context.style.misc_inherited1_data.list_style_type;
  let image = available_marker_image(item_context);
  let has_image = image.is_some();
  let (mut style, sizing, current_color) =
    item_context.resolve_pseudo_style(&MatchedDeclarationsView::default());

  match position {
    ListStylePosition::Outside => {
      // Blink's `AdjustStyleForMarker`: an outside marker is a block container that does not
      // break and keeps its trailing spaces.
      let inherited = style.inherited_data_mut();

      inherited.white_space_collapse = WhiteSpaceCollapse::Preserve;
      inherited.text_wrap_mode = TextWrapMode::NoWrap;
      style.base_data_mut().display = Display::InlineBlock;
    }
    ListStylePosition::Inside => {
      let (margin_start, margin_end) = inline_margins_for_inside(has_image, style_type);
      let box_data = style.box_data_mut();
      let (start, end) =
        direction.inline_sides(&mut box_data.margin_left, &mut box_data.margin_right);

      *start = margin_start;
      *end = margin_end;
      style.base_data_mut().display = Display::Inline;
    }
  }

  let mut context = RenderContext::from_parent(item_context, style, sizing, current_color);

  if position == ListStylePosition::Inside {
    context
      .text_decorations
      .clone_from(&item_context.text_decorations);
  }

  let content = match image {
    Some(image) => marker_image(&context, image),
    None => {
      let text = style_type.marker_text(ordinal, direction)?;

      if style_type.is_symbolic() {
        RenderNode::anonymous_text_item(&context, text.trim_end().to_owned())
      } else {
        RenderNode::anonymous_text_item(&context, text)
      }
    }
  };
  let mut marker = RenderNode::new(
    context,
    NodeOrigin::Marker,
    Some(Node::container([])),
    Some(Box::new([content])),
  );

  // Blink's `LayoutOutsideListMarker` lays its content out in lines, an image marker's too.
  marker.force_inline_layout = position == ListStylePosition::Outside;

  Some(marker)
}

/// Blink's `InlineMarginsForInside`: the marker box's inline-start and inline-end margins.
fn inline_margins_for_inside(has_image: bool, style_type: &ListStyleType) -> (Length, Length) {
  if has_image {
    return (Length::zero(), Length::Px(MARKER_GAP_PX));
  }

  match style_type {
    ListStyleType::DisclosureOpen | ListStyleType::DisclosureClosed => {
      (Length::zero(), Length::Em(INSIDE_DISCLOSURE_GAP_EM))
    }
    _ if style_type.is_symbolic() => (Length::Px(-1.0), Length::Em(INSIDE_SYMBOL_GAP_EM)),
    _ => (Length::zero(), Length::zero()),
  }
}

/// Blink's `ListMarker::InlineMarginsForOutside`: the inline-start and inline-end margins that
/// hang an outside marker `marker_inline_size` wide outside its list item.
pub(crate) fn inline_margins_for_outside(
  context: &RenderContext,
  marker_inline_size: LayoutUnit,
) -> (LayoutUnit, LayoutUnit) {
  let style_type = &context.style.misc_inherited1_data.list_style_type;

  if available_marker_image(context).is_some() {
    let gap = LayoutUnit::from_float(MARKER_GAP_PX);

    return (-marker_inline_size - gap, gap);
  }
  if !style_type.is_symbolic() {
    return (-marker_inline_size, LayoutUnit::ZERO);
  }

  let Some(ascent) = primary_font_ascent(context) else {
    return (LayoutUnit::ZERO, LayoutUnit::ZERO);
  };
  let offset = match style_type {
    ListStyleType::DisclosureOpen | ListStyleType::DisclosureClosed => {
      disclosure_symbol_size(context)
    }
    _ => LayoutUnit::from_int(ascent * 2 / 3),
  };
  let gap = offset + LayoutUnit::from_float(MARKER_GAP_PX) + LayoutUnit::from_int(1);

  (-gap, gap - marker_inline_size)
}

/// Blink's `FontMetrics::Ascent` of the primary font, in whole px.
fn primary_font_ascent(context: &RenderContext) -> Option<i32> {
  context
    .primary_font_metrics(&context.style, context.sizing.font_size)
    .map(|metrics| metrics.ascent as i32)
}

/// Blink's `DisclosureSymbolSize`.
fn disclosure_symbol_size(context: &RenderContext) -> LayoutUnit {
  LayoutUnit::from_float(context.sizing.font_size * DISCLOSURE_SYMBOL_SIZE_EM)
}

/// Blink's `ListMarker::WidthOfSymbol`: the width a symbol marker, painted as a shape, takes.
pub(crate) fn width_of_symbol(context: &RenderContext) -> LayoutUnit {
  let Some(ascent) = primary_font_ascent(context) else {
    return LayoutUnit::ZERO;
  };

  match context.style.misc_inherited1_data.list_style_type {
    ListStyleType::DisclosureOpen | ListStyleType::DisclosureClosed => {
      disclosure_symbol_size(context)
    }
    _ => LayoutUnit::from_int((ascent * 2 / 3 + 1) / 2 + 2),
  }
}

/// The shape Blink's `TextFragmentPainter::PaintSymbol` paints a symbol marker as.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SymbolShape {
  /// `disc`: a filled ellipse.
  Disc,
  /// `circle`: an ellipse stroked 1px wide.
  Circle,
  /// `square`: a filled rectangle.
  Square,
  /// `disclosure-open` and `disclosure-closed`: a filled triangle.
  Disclosure(DisclosureOrientation),
}

/// Blink's `GetDisclosureOrientation` in `horizontal-tb`, the only writing mode takumi lays out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisclosureOrientation {
  /// Toward the inline end of right-to-left text.
  Left,
  /// Toward the inline end of left-to-right text.
  Right,
  /// Toward the block end.
  Down,
}

/// A symbol marker's shape and where it paints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SymbolMarker {
  pub(crate) shape: SymbolShape,
  /// Blink's `RelativeSymbolMarkerRect` top-left, from the marker text's start on its baseline.
  pub(crate) offset: Point<f32>,
  /// The side of that rect, which is square.
  pub(crate) side: f32,
}

impl SymbolMarker {
  /// Blink's `ListMarker::RelativeSymbolMarkerRect` for marker text in `context`.
  pub(crate) fn of(context: &RenderContext) -> Option<Self> {
    let style = &context.style;
    let shape = match style.misc_inherited1_data.list_style_type {
      ListStyleType::Disc => SymbolShape::Disc,
      ListStyleType::Circle => SymbolShape::Circle,
      ListStyleType::Square => SymbolShape::Square,
      ListStyleType::DisclosureOpen => SymbolShape::Disclosure(DisclosureOrientation::Down),
      ListStyleType::DisclosureClosed => {
        SymbolShape::Disclosure(match style.inherited_data.direction {
          Direction::Ltr => DisclosureOrientation::Right,
          Direction::Rtl => DisclosureOrientation::Left,
        })
      }
      _ => return None,
    };
    let ascent = primary_font_ascent(context)?;
    let (offset, side) = match shape {
      SymbolShape::Disclosure(_) => {
        let side = disclosure_symbol_size(context).to_float();

        (Point { x: 0.0, y: -side }, side)
      }
      _ => {
        let top = 3 * (ascent - ascent * 2 / 3) / 2;

        (
          Point {
            x: 1.0,
            y: (top - ascent) as f32,
          },
          ((ascent * 2 / 3 + 1) / 2) as f32,
        )
      }
    };

    Some(Self {
      shape,
      offset,
      side,
    })
  }
}

/// css-lists-3 §3.1: an image that is not available leaves the counter style
/// to draw the marker. Only URL images qualify.
fn available_marker_image(item_context: &RenderContext) -> Option<BackgroundImage> {
  let image = item_context
    .style
    .misc_inherited1_data
    .list_style_image
    .image()?;
  let BackgroundImage::Url(url) = image else {
    return None;
  };

  resolve_image(url, item_context).ok()?;
  Some(image.clone())
}

/// A marker image at its natural size.
fn marker_image(context: &RenderContext, mut image: BackgroundImage) -> RenderNode {
  // Inheriting the image shares it, so its lengths resolve here rather than
  // once per node that inherited it.
  image.make_computed(&context.sizing);

  let mut item = RenderNode::anonymous_image_item(context, image);

  if let Some(layout_style) = &mut item.layout_style_override {
    Rc::make_mut(layout_style).max_size = Size::all(LayoutLength::Auto);
  }

  item
}

/// The running count a list hands to its items, honoring `start` and `value`.
#[derive(Debug, Clone, Copy)]
pub(super) struct ListCounter {
  next: i32,
}

impl ListCounter {
  pub(super) fn new(node: &Node) -> Self {
    Self {
      next: attribute_number(node, "start").unwrap_or(1),
    }
  }

  /// The item's own `value` attribute, else the list's running count.
  pub(super) fn take(&mut self, node: &Node) -> i32 {
    let ordinal = attribute_number(node, "value").unwrap_or(self.next);
    self.next = ordinal.saturating_add(1);
    ordinal
  }
}

/// Blink counts list items within their enclosing list, so a wrapper element
/// inside one keeps counting rather than starting over. A list element is the
/// one signal for where a list begins; a tree built in code carries no tags, so
/// there every parent counts the items it holds.
pub(super) fn owns_list_counter(node: &Node, inside_list: bool) -> bool {
  !inside_list || is_list_element(node)
}

pub(super) fn is_list_element(node: &Node) -> bool {
  node
    .tag_name()
    .is_some_and(|tag| tag.eq_ignore_ascii_case("ol") || tag.eq_ignore_ascii_case("ul"))
}

fn attribute_number(node: &Node, name: &str) -> Option<i32> {
  node
    .attribute(name)
    .and_then(|value| value.trim().parse::<i32>().ok())
}

#[cfg(test)]
mod tests {
  use std::{collections::BTreeMap, sync::Arc};

  use crate::{
    context::RenderContext,
    layout::{
      node::{Node, NodeKind},
      tree::RenderNode,
    },
    resources::font::Fonts,
    style::{SizingContext, StyleSheet},
    viewport::Viewport,
  };

  fn attributes(pairs: &[(&str, &str)]) -> BTreeMap<Box<str>, Box<str>> {
    pairs
      .iter()
      .map(|(name, value)| ((*name).into(), (*value).into()))
      .collect()
  }

  fn render_tree(root: Node, css: &str) -> RenderNode {
    let stylesheet = StyleSheet::parse(css).expect("stylesheet parses");
    let fonts = Fonts::default();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .stylesheet(Arc::new(stylesheet))
      .build();

    RenderNode::from_node(&context, root)
  }

  fn text_runs(node: &RenderNode) -> Vec<String> {
    let mut collected: Vec<String> = node.anonymous_text_content.clone().into_iter().collect();

    for child in node.children.iter().flat_map(|children| children.iter()) {
      collected.extend(text_runs(child));
    }

    collected
  }

  /// Every marker in the tree, in document order.
  fn collect_markers(node: &RenderNode, collected: &mut Vec<String>) {
    if let Some(marker) = &node.marker {
      collected.extend(text_runs(marker));
    }

    for child in node.children.iter().flat_map(|children| children.iter()) {
      collect_markers(child, collected);
    }
  }

  fn markers(root: Node, css: &str) -> Vec<String> {
    let mut collected = Vec::new();
    collect_markers(&render_tree(root, css), &mut collected);
    collected
  }

  fn first_child(node: &RenderNode) -> &RenderNode {
    &node.children.as_deref().expect("children")[0]
  }

  fn marker_of(node: &RenderNode) -> &RenderNode {
    node.marker.as_deref().expect("marker")
  }

  fn item(children: impl Into<Vec<Node>>) -> Node {
    Node::container(children).with_class_name("item")
  }

  const LIST_CSS: &str = ".list { list-style-type: decimal } .item { display: list-item }";

  /// A 1x1 PNG, so the marker image resolves without a registered resource.
  const PIXEL_DATA_URI: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

  #[test]
  fn items_are_numbered_in_document_order() {
    let list = Node::container([item([]), item([]), item([])]).with_class_name("list");

    assert_eq!(markers(list, LIST_CSS), ["1. ", "2. ", "3. "]);
  }

  #[test]
  fn a_nested_list_restarts_at_one() {
    let list = Node::container([
      item([]),
      item([Node::container([item([]), item([])]).with_class_name("list")]),
    ])
    .with_class_name("list");

    assert_eq!(markers(list, LIST_CSS), ["1. ", "2. ", "1. ", "2. "]);
  }

  /// A list element encloses its items, so a wrapper inside one keeps counting.
  #[test]
  fn a_wrapper_element_does_not_restart_the_count() {
    let list = Node::container([
      item([]),
      Node::container([item([]), item([])]).with_class_name("block"),
    ])
    .with_class_name("list")
    .with_tag_name("ol");
    let css = format!("{LIST_CSS} .block {{ display: block }}");

    assert_eq!(markers(list, &css), ["1. ", "2. ", "3. "]);
  }

  #[test]
  fn start_and_value_attributes_move_the_count() {
    let list = Node::container([
      item([]),
      item([]).with_attributes(attributes(&[("value", "9")])),
      item([]),
    ])
    .with_class_name("list")
    .with_attributes(attributes(&[("start", "3")]));

    assert_eq!(markers(list, LIST_CSS), ["3. ", "9. ", "10. "]);
  }

  /// A wrapper is not a list, so its `start` leaves the enclosing count alone.
  #[test]
  fn a_wrapper_start_attribute_does_not_restart_the_count() {
    let list = Node::container([
      item([]),
      Node::container([item([])])
        .with_class_name("block")
        .with_attributes(attributes(&[("start", "9")])),
    ])
    .with_class_name("list")
    .with_tag_name("ol");
    let css = format!("{LIST_CSS} .block {{ display: block }}");

    assert_eq!(markers(list, &css), ["1. ", "2. "]);
  }

  /// A block chain leads to the item's first line, wherever that line sits.
  #[test]
  fn a_nested_block_chain_hosts_the_marker() {
    let paragraph = Node::container([Node::text("hello")]).with_class_name("block");
    let list = Node::container([item(
      [Node::container([paragraph]).with_class_name("block")],
    )])
    .with_class_name("list");
    let css = format!("{LIST_CSS} .block {{ display: block }}");

    let tree = render_tree(list, &css);
    let paragraph = first_child(first_child(first_child(&tree)));

    assert_eq!(
      text_runs(marker_of(paragraph)),
      ["1. "],
      "the marker did not reach the paragraph"
    );
  }

  #[test]
  fn a_none_counter_style_generates_no_marker() {
    let list = Node::container([item([])]).with_class_name("list");
    let css = ".list { list-style-type: none } .item { display: list-item }";

    assert!(markers(list, css).is_empty());
  }

  /// A text list item folds its text into the item node, and the marker still
  /// has to come first.
  #[test]
  fn a_folded_text_item_keeps_the_marker_first() {
    let list =
      Node::container([Node::text("line 1").with_class_name("item")]).with_class_name("list");

    let tree = render_tree(list, LIST_CSS);
    let item = first_child(&tree);

    assert_eq!(text_runs(marker_of(item)), ["1. "]);
    assert!(
      matches!(
        item.node.as_ref().map(|node| &node.kind),
        Some(NodeKind::Text(text)) if text.text == "line 1"
      ),
      "the item's own text was moved instead of staying put"
    );
  }

  fn block_only_item(css: &str) -> RenderNode {
    let list = Node::container([item([
      Node::container([Node::text("hello")]).with_class_name("block")
    ])])
    .with_class_name("list");

    render_tree(list, css)
  }

  /// Blink puts an outside marker on the item's first line, which for
  /// block-level content lives inside the first block.
  #[test]
  fn a_block_only_item_hosts_the_marker_in_its_first_block() {
    let tree = block_only_item(&format!("{LIST_CSS} .block {{ display: block }}"));
    let block = first_child(first_child(&tree));

    assert_eq!(text_runs(marker_of(block)), ["1. "]);
  }

  /// An inside marker is the item's own content, so it keeps its place before
  /// the block instead of joining that block's line.
  #[test]
  fn an_inside_marker_stays_outside_the_block_child() {
    let tree = block_only_item(&format!(
      "{LIST_CSS} .block {{ display: block }} .item {{ list-style-position: inside }}"
    ));
    let item = first_child(&tree);

    assert!(item.marker.is_none(), "the marker joined the block's line");
    assert_eq!(text_runs(marker_of(first_child(item))), ["1. "]);
  }

  /// css-lists-3 §3.1: an unavailable image leaves the counter style to draw.
  #[test]
  fn an_unavailable_marker_image_falls_back_to_the_counter() {
    let list = Node::container([item([])]).with_class_name("list");
    let css = format!("{LIST_CSS} .list {{ list-style-image: url(missing.png) }}");

    assert_eq!(markers(list, &css), ["1. "]);
  }

  /// An image marker replaces the counter style, even when that style is `none`.
  #[test]
  fn a_marker_image_becomes_an_image_child() {
    let list = Node::container([item([])]).with_class_name("list");
    let css = format!(
      ".list {{ list-style: none url({PIXEL_DATA_URI}) }} \
       .item {{ display: list-item; font-size: 20px }}"
    );
    let css = css.as_str();

    let tree = render_tree(list, css);
    let marker_image = first_child(marker_of(first_child(&tree)));

    assert!(marker_image.anonymous_text_content.is_none());
    assert!(matches!(
      marker_image.node.as_ref().map(|node| &node.kind),
      Some(NodeKind::Image(_))
    ));
  }

  /// A gradient is not an available marker image, so the counter style draws.
  #[test]
  fn a_gradient_marker_image_falls_back_to_the_counter() {
    let list = Node::container([item([])]).with_class_name("list");
    let css = ".list { list-style-image: radial-gradient(red, blue) }                .item { display: list-item; list-style-type: decimal }";

    assert_eq!(markers(list, css), ["1. "]);
  }
}
