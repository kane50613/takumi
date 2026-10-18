//! Blink's [`InlineNode`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_node.cc).

use crate::{
  layout::{
    inline::{
      InlineContentKind,
      inline_item::{InlineItemType, InlineItems, LayoutObject},
      inline_items_builder::{CollectedInlines, InlineItemsBuilder},
    },
    node::{Node, NodeKind},
    tree::{NodeOrigin, RenderNode},
  },
  style::{Display, Float, ListStylePosition},
  text_processing::apply_text_transform,
};

/// A box's content in Blink's child order: the marker, the box's own text, then its children.
#[derive(Clone, Copy)]
enum Child<'c> {
  OwnText,
  Box(&'c RenderNode),
}

/// A box being walked, and whether it opened an inline box.
struct Frame<'c> {
  node: &'c RenderNode,
  parent: Option<&'c RenderNode>,
  is_inline: bool,
  children: Vec<Child<'c>>,
}

impl<'c> Frame<'c> {
  fn new(node: &'c RenderNode, parent: Option<&'c RenderNode>, is_inline: bool) -> Self {
    let mut children: Vec<Child<'c>> = node.marker.as_deref().map(Child::Box).into_iter().collect();

    children.push(Child::OwnText);
    children.extend(node.children.iter().flatten().map(Child::Box));
    children.reverse();

    Self {
      node,
      parent,
      is_inline,
      children,
    }
  }

  /// Blink's `FindSymbolMarkerLayoutText`: whether this box's text is a symbol marker's, which
  /// paints as a shape.
  fn is_symbol_marker_text(&self) -> bool {
    self
      .parent
      .is_some_and(|parent| parent.origin == NodeOrigin::Marker)
      && self
        .node
        .context
        .style
        .misc_inherited1_data
        .list_style_type
        .is_symbolic()
  }
}

/// Blink's `CollectInlinesInternal`: the items and text content of the inline formatting context
/// `block` establishes.
pub(crate) fn collect_inlines<'c>(
  block: &'c RenderNode,
  items: &mut InlineItems<'c>,
) -> CollectedInlines {
  let mut builder = InlineItemsBuilder::new(items);
  let mut stack = vec![Frame::new(block, None, false)];

  builder.enter_block(&block.context.style);

  while let Some(mut frame) = stack.pop() {
    let Some(child) = frame.children.pop() else {
      if frame.is_inline {
        builder.exit_inline(frame.node);
      }

      continue;
    };
    let parent = frame.node;

    match child {
      Child::OwnText => {
        if let Some(text) = frame.node.own_text() {
          let text =
            apply_text_transform(text, frame.node.context.style.inherited_data.text_transform);

          builder.append_text(&text, LayoutObject::Text(frame.node));

          if frame.is_symbol_marker_text() {
            builder.set_is_symbol_marker();
          }
        }

        stack.push(frame);
      }
      Child::Box(node) => {
        stack.push(frame);

        if node.context.style.base_data.float != Float::None {
          builder.append_floating(LayoutObject::Box(node));
        } else if node.is_out_of_flow() {
          builder.append_out_of_flow_positioned(LayoutObject::Box(node));
        } else if node.is_atomic_inline() {
          if node.is_outside_list_marker() {
            builder.append_opaque(InlineItemType::ListMarker, LayoutObject::Box(node));
          } else {
            builder.append_atomic_inline(LayoutObject::Box(node));
          }
        } else if node.context.style.base_data.display == Display::Inline {
          builder.enter_inline(node);
          stack.push(Frame::new(node, Some(parent), true));
        } else {
          stack.push(Frame::new(node, Some(parent), false));
        }
      }
    }
  }

  builder.exit_block();
  builder.finish()
}

impl RenderNode {
  /// The text of this box's own `LayoutText`, if it has one.
  fn own_text(&self) -> Option<&str> {
    if let Some(text) = self.anonymous_text_content.as_deref() {
      return Some(text);
    }

    match &self.node.as_ref()?.kind {
      NodeKind::Text(text) => Some(&text.text),
      _ => None,
    }
  }

  /// Blink's `IsAtomicInline`.
  fn is_atomic_inline(&self) -> bool {
    self.is_inline_atomic_container()
      || matches!(
        self.node.as_ref().and_then(Node::inline_content),
        Some(InlineContentKind::Box)
      )
  }

  /// Blink's `IsLayoutOutsideListMarker`.
  fn is_outside_list_marker(&self) -> bool {
    self.origin == NodeOrigin::Marker
      && self.context.style.inherited_data.list_style_position == ListStylePosition::Outside
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use crate::{
    context::RenderContext,
    layout::{
      inline::{
        inline_item::{InlineItemType, InlineItems},
        inline_node::collect_inlines,
      },
      node::Node,
      tree::RenderNode,
    },
    resources::font::Fonts,
    style::{SizingContext, StyleSheet},
    viewport::Viewport,
  };

  use InlineItemType::*;

  const CSS: &str = ".block { display: block } .span { display: inline } \
    .atomic { display: inline-block } .float { float: left } .abs { position: absolute } \
    .list { list-style-type: disc } .item { display: list-item }";

  fn render_tree(root: Node, css: &str) -> RenderNode {
    let context = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .stylesheet(Arc::new(
        StyleSheet::parse(&format!("{CSS} {css}")).expect("stylesheet parses"),
      ))
      .build();

    RenderNode::from_node(&context, root)
  }

  /// The text content and each item's type and text.
  fn collect(block: &RenderNode) -> (String, Vec<(InlineItemType, String)>) {
    let mut items = InlineItems::new();
    let collected = collect_inlines(block, &mut items);
    let text = &collected.text_content;
    let items = items
      .iter()
      .map(|item| {
        (
          item.item_type,
          text[item.start_offset()..item.end_offset()].to_owned(),
        )
      })
      .collect();

    (collected.text_content, items)
  }

  fn span(text: &str) -> Node {
    Node::text(text).with_class_name("span")
  }

  fn block(children: impl Into<Vec<Node>>) -> Node {
    Node::container(children).with_class_name("block")
  }

  #[test]
  fn spaces_collapse_across_inline_boxes() {
    let tree = render_tree(block([span("a  "), span(" b "), span("  c")]), "");

    assert_eq!(
      collect(&tree),
      (
        "a b c".to_owned(),
        vec![
          (OpenTag, String::new()),
          (Text, "a ".to_owned()),
          (CloseTag, String::new()),
          (OpenTag, String::new()),
          (Text, "b ".to_owned()),
          (CloseTag, String::new()),
          (OpenTag, String::new()),
          (Text, "c".to_owned()),
          (CloseTag, String::new()),
        ]
      )
    );
  }

  #[test]
  fn a_br_is_a_forced_break() {
    let tree = render_tree(
      block([span("a"), Node::text("\n").with_tag_name("br"), span("b")]),
      "",
    );
    let (text, items) = collect(&tree);

    assert_eq!(text, "a\nb");
    assert!(items.contains(&(Control, "\n".to_owned())));
  }

  #[test]
  fn an_atomic_inline_is_an_object_replacement_character() {
    let tree = render_tree(
      block([span("a "), block([]).with_class_name("atomic"), span(" b")]),
      "",
    );
    let (text, items) = collect(&tree);

    assert_eq!(text, "a \u{FFFC} b");
    assert!(items.contains(&(AtomicInline, "\u{FFFC}".to_owned())));
  }

  /// Floats and out-of-flow boxes are opaque, so the spaces around them collapse together.
  #[test]
  fn floats_and_out_of_flow_boxes_are_opaque_to_collapsing() {
    let tree = render_tree(
      block([
        span("a "),
        block([]).with_class_name("float"),
        span(" b "),
        block([]).with_class_name("abs"),
        span(" c"),
      ]),
      "",
    );
    let (text, items) = collect(&tree);

    assert_eq!(text, "a b c");
    assert!(items.contains(&(Floating, String::new())));
    assert!(items.contains(&(OutOfFlowPositioned, String::new())));
  }

  #[test]
  fn text_transform_applies_before_collapsing() {
    let tree = render_tree(
      block([span("abc  def")]),
      ".span { text-transform: uppercase }",
    );

    assert_eq!(collect(&tree).0, "ABC DEF");
  }

  #[test]
  fn an_rtl_block_enables_bidi() {
    let mut items = InlineItems::new();
    let tree = render_tree(block([span("a")]), ".block { direction: rtl }");

    assert!(collect_inlines(&tree, &mut items).is_bidi_enabled);
  }

  /// Tabs and newlines take control items of their own, so the line breaker needs no special case.
  #[test]
  fn preserved_tabs_and_newlines_are_control_items() {
    let tree = render_tree(block([span("a\t\tb\nc")]), ".span { white-space: pre }");

    assert_eq!(
      collect(&tree).1,
      vec![
        (OpenTag, String::new()),
        (Text, "a".to_owned()),
        (Control, "\t\t".to_owned()),
        (Text, "b".to_owned()),
        (Control, "\n".to_owned()),
        (Text, "c".to_owned()),
        (CloseTag, String::new()),
      ]
    );
  }

  /// `pre-line` keeps its newlines and collapses the spaces around them.
  #[test]
  fn preserved_breaks_drop_the_spaces_around_them() {
    let tree = render_tree(block([span("a  \n  b")]), ".span { white-space: pre-line }");

    assert_eq!(collect(&tree).0, "a\nb");
  }

  /// Text after a preserved newline may start with a multi-byte character.
  #[test]
  fn preserved_breaks_before_non_ascii_text() {
    let tree = render_tree(
      block([span("\u{E9}\na\n\u{4E2D}")]),
      ".span { white-space: pre-line }",
    );

    assert_eq!(collect(&tree).0, "\u{E9}\na\n\u{4E2D}");
  }

  /// css-text-3 §4.1.3: a segment break next to a zero width space goes.
  #[test]
  fn a_newline_next_to_a_zero_width_space_goes() {
    let tree = render_tree(block([span("a\u{200B}\nb")]), "");

    assert_eq!(collect(&tree).0, "a\u{200B}b");
  }

  /// Spaces leading a wrapping `pre-wrap` line end with a soft wrap opportunity.
  #[test]
  fn leading_preserved_spaces_end_with_a_break_opportunity() {
    let tree = render_tree(block([span("  a")]), ".span { white-space: pre-wrap }");

    assert_eq!(collect(&tree).0, "  \u{200B}a");
  }

  #[test]
  fn an_empty_inline_box_creates_a_box_fragment() {
    let mut items = InlineItems::new();
    let tree = render_tree(
      block([Node::container([]).with_class_name("span"), span("a")]),
      "",
    );

    collect_inlines(&tree, &mut items);

    assert!(items[0].should_create_box_fragment);
    assert!(!items[3].should_create_box_fragment);
  }

  #[test]
  fn a_decorated_inline_box_creates_a_box_fragment() {
    let mut items = InlineItems::new();
    let tree = render_tree(
      block([
        span("a").with_class_name("span padded"),
        span("b").with_class_name("span positioned"),
        span("c").with_class_name("span painted"),
        span("d"),
      ]),
      ".padded { padding-left: 1px } .positioned { position: relative } \
        .painted { background-color: red }",
    );

    collect_inlines(&tree, &mut items);

    let open_tags: Vec<_> = items
      .iter()
      .filter(|item| item.item_type == InlineItemType::OpenTag)
      .map(|item| item.should_create_box_fragment)
      .collect();

    assert_eq!(open_tags, [true, true, true, false]);
  }

  /// Blink's `LayoutOutsideListMarker` is an opaque `kListMarker` item ahead of the content.
  #[test]
  fn an_outside_marker_is_a_list_marker_item() {
    let list = Node::container([Node::text(" a").with_class_name("item")]).with_class_name("list");
    let tree = render_tree(list, "");
    let (text, items) = collect(&tree.children.as_deref().expect("children")[0]);

    assert_eq!(text, "a");
    assert_eq!(items[0], (ListMarker, String::new()));
  }
}
