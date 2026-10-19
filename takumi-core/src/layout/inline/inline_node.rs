//! Blink's [`InlineNode`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_node.cc).

use icu_properties::props::Script;

use crate::{
  fonts::{
    font_fallback_priority::FontFallbackPriority,
    shaping::run_segmenter::{RunSegmenter, RunSegmenterRange},
  },
  layout::{
    inline::{
      InlineContentKind,
      inline_item::{InlineItemType, LayoutObject, set_bidi_level, set_segment_data},
      inline_item_segment::InlineItemSegments,
      inline_items_builder::InlineItemsBuilder,
      inline_node_data::InlineNodeData,
    },
    node::{Node, NodeKind},
    tree::{NodeOrigin, RenderNode},
  },
  style::{Direction as TextDirection, Display, Float, ListStylePosition},
  text::{
    bidi_paragraph::BidiParagraph,
    character::{OBJECT_REPLACEMENT_CHARACTER, is_8bit},
  },
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

/// Blink's `InlineNode`: the inline formatting context `block` establishes.
#[derive(Clone, Copy)]
pub(crate) struct InlineNode<'c> {
  block: &'c RenderNode,
}

impl<'c> InlineNode<'c> {
  pub(crate) fn new(block: &'c RenderNode) -> Self {
    Self { block }
  }

  /// Blink's `CollectInlinesInternal`: the items and text content of the context.
  pub(crate) fn collect_inlines(self) -> InlineNodeData<'c> {
    let mut builder = InlineItemsBuilder::new();
    let mut stack = vec![Frame::new(self.block, None, false)];

    builder.enter_block(&self.block.context.style);

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

          // An out-of-flow box does not float: CSS 2.1 §9.7 computes its `float` to `none`, which
          // takumi's computed style leaves as specified.
          if node.is_out_of_flow() {
            builder.append_out_of_flow_positioned(LayoutObject::Box(node));
          } else if node.context.style.base_data.float != Float::None {
            builder.append_floating(LayoutObject::Box(node));
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

  /// Blink's `SegmentText`, without font orientation.
  pub(crate) fn segment_text(self, data: &mut InlineNodeData<'c>) {
    self.segment_bidi_runs(data);
    self.segment_script_runs(data);

    if let Some(segments) = &mut data.segments {
      segments.compute_item_index(&data.items);
    }
  }

  /// Blink's `SegmentBidiRuns`: resolves the bidi levels, splitting items where they change.
  /// <http://unicode.org/reports/tr9/#Resolving_Embedding_Levels>
  fn segment_bidi_runs(self, data: &mut InlineNodeData<'c>) {
    if !data.is_bidi_enabled {
      data.base_direction = TextDirection::Ltr;

      return;
    }

    let base_direction = self.block.context.style.inherited_data.direction;
    let mut bidi = BidiParagraph::new(&data.text_content, Some(base_direction));

    data.base_direction = bidi.base_direction();

    if bidi.is_unidirectional() && bidi.base_direction() == TextDirection::Ltr {
      data.is_bidi_enabled = false;

      return;
    }

    // Floats and out-of-flow boxes act as U+FFFC neutrals, as in every browser, though css-text-3
    // ignores them. <https://drafts.csswg.org/css-text-3/#text-encoding>
    let mut out_of_flow_offsets = Vec::new();
    let mut text_length = data.text_content.len();

    if data.has_floating_or_out_of_flow_positioned() {
      let mut text = String::with_capacity(data.text_content.len() + 16);
      let mut last_offset = 0;

      for item in &data.items {
        if item.is_floating_or_out_of_flow_positioned() {
          let offset = item.start_offset();

          text.push_str(&data.text_content[last_offset..offset]);
          last_offset = offset;
          out_of_flow_offsets.push(text.len());
          text.push(OBJECT_REPLACEMENT_CHARACTER);
        }
      }

      if !out_of_flow_offsets.is_empty() {
        text.push_str(&data.text_content[last_offset..]);
        bidi.set_paragraph(&text, Some(base_direction));
        text_length = text.len();
        out_of_flow_offsets.push(usize::MAX);
      }
    }

    let character_length = OBJECT_REPLACEMENT_CHARACTER.len_utf8();
    let mut out_of_flow_index = 0;
    let mut item_index = 0;
    let mut start = 0;

    while start < text_length {
      let (end, level) = bidi.get_logical_run(start);
      let mut num_out_of_flow = 0;

      while !out_of_flow_offsets.is_empty() && end > out_of_flow_offsets[out_of_flow_index] {
        out_of_flow_index += 1;
        num_out_of_flow += 1;
      }

      item_index = set_bidi_level(
        &mut data.items,
        item_index,
        end - out_of_flow_index * character_length,
        level,
        num_out_of_flow,
      );
      start = end;
    }
  }

  /// Blink's `SegmentScriptRuns`: the script and font fallback priority of each run, segmented as
  /// `RunSegmenter` does.
  fn segment_script_runs(self, data: &mut InlineNodeData<'c>) {
    if data.text_content.is_empty() {
      data.segments = None;

      return;
    }

    if (is_8bit(&data.text_content) || !data.has_non_orc_16bit) && !data.is_bidi_enabled {
      let range = RunSegmenterRange {
        start: 0,
        end: data.text_content.len(),
        script: Script::Latin,
        font_fallback_priority: FontFallbackPriority::Text,
      };

      set_segment_data(&range, &mut data.items);
      data.segments = None;

      return;
    }

    let mut segmenter = RunSegmenter::new(&data.text_content);
    let Some(range) = segmenter.consume() else {
      return;
    };

    if range.end == data.text_content.len() {
      set_segment_data(&range, &mut data.items);
      data.segments = None;

      return;
    }

    data.segments = Some(InlineItemSegments::compute_segments(&mut segmenter, range));
  }
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
        inline_item::{InlineItem, InlineItemType, LayoutObject},
        inline_node::InlineNode,
        inline_node_data::InlineNodeData,
      },
      node::Node,
      tree::RenderNode,
    },
    resources::font::Fonts,
    style::{Direction as TextDirection, SizingContext, StyleSheet},
    viewport::Viewport,
  };

  use InlineItemType::*;
  use TextDirection::{Ltr, Rtl};

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
    let data = InlineNode::new(block).collect_inlines();
    let text = &data.text_content;
    let items = data
      .items
      .iter()
      .map(|item| {
        (
          item.item_type,
          text[item.start_offset()..item.end_offset()].to_owned(),
        )
      })
      .collect();

    (data.text_content, items)
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
  fn an_absolute_float_is_out_of_flow() {
    let tree = render_tree(block([block([]).with_class_name("abs float")]), "");
    let (_, items) = collect(&tree);

    assert_eq!(items, [(OutOfFlowPositioned, String::new())]);
  }

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
    let tree = render_tree(block([span("a")]), ".block { direction: rtl }");

    assert!(InlineNode::new(&tree).collect_inlines().is_bidi_enabled);
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
    let tree = render_tree(
      block([Node::container([]).with_class_name("span"), span("a")]),
      "",
    );
    let items = InlineNode::new(&tree).collect_inlines().items;

    assert!(items[0].should_create_box_fragment);
    assert_eq!(items[2].item_type, InlineItemType::OpenTag);
    assert!(!items[2].should_create_box_fragment);
  }

  #[test]
  fn a_decorated_inline_box_creates_a_box_fragment() {
    let tree = render_tree(
      block([
        span("a").with_class_name("span padded"),
        span("b").with_class_name("span positioned"),
        span("c").with_class_name("span painted"),
        span("d"),
        span("e").with_class_name("span unpainted"),
      ]),
      ".padded { padding-left: 1px } .positioned { position: relative } \
        .painted { background-color: red } .unpainted { background-image: none }",
    );

    let items = InlineNode::new(&tree).collect_inlines().items;
    let open_tags: Vec<_> = items
      .iter()
      .filter(|item| item.item_type == InlineItemType::OpenTag)
      .map(|item| item.should_create_box_fragment)
      .collect();

    assert_eq!(open_tags, [true, true, true, false, false]);
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

  /// Blink's `LayoutInsideListMarker` is an inline box, and a symbol's text paints as a shape.
  #[test]
  fn an_inside_marker_is_an_inline_box() {
    let list = Node::container([Node::text("a").with_class_name("item")]).with_class_name("list");
    let tree = render_tree(list, ".list { list-style-position: inside }");
    let item = &tree.children.as_deref().expect("children")[0];
    let data = InlineNode::new(item).collect_inlines();
    let items = &data.items;
    let types: Vec<_> = items.iter().map(|item| item.item_type).collect();

    assert_eq!(data.text_content, "\u{2022}a");
    assert_eq!(types, [OpenTag, Text, CloseTag, Text]);
    assert!(items[1].is_symbol_marker());
    assert!(!items[3].is_symbol_marker());
  }

  /// Blink's `InlineNodeForTest`: items appended as they are, without collapsing.
  struct TestNode<'c> {
    block: &'c RenderNode,
    data: InlineNodeData<'c>,
  }

  impl<'c> TestNode<'c> {
    fn new(block: &'c RenderNode) -> Self {
      Self {
        block,
        data: InlineNodeData::default(),
      }
    }

    fn append(&mut self, text: &str) {
      let start = self.data.text_content.len();

      self.data.text_content.push_str(text);
      self.data.items.push(InlineItem::new(
        Text,
        start,
        self.data.text_content.len(),
        Some(LayoutObject::Text(self.block)),
      ));
    }

    fn append_control(&mut self, character: char) {
      let start = self.data.text_content.len();

      self.data.text_content.push(character);
      self.data.items.push(InlineItem::new(
        BidiControl,
        start,
        self.data.text_content.len(),
        None,
      ));
    }

    /// Each item's offsets and direction once segmented.
    fn segment_text(mut self) -> Vec<(usize, usize, TextDirection)> {
      self.data.is_bidi_enabled = true;
      InlineNode::new(self.block).segment_text(&mut self.data);

      self
        .data
        .items
        .iter()
        .map(|item| (item.start_offset(), item.end_offset(), item.direction()))
        .collect()
    }
  }

  const HEBREW: &str = "\u{05E2}\u{05D1}\u{05E8}\u{05D9}\u{05EA}";

  #[test]
  fn segment_ascii() {
    let tree = render_tree(block([]), "");
    let mut node = TestNode::new(&tree);

    node.append("Hello");

    assert_eq!(node.segment_text(), [(0, 5, Ltr)]);
  }

  #[test]
  fn segment_hebrew() {
    let tree = render_tree(block([]), "");
    let mut node = TestNode::new(&tree);

    node.append(HEBREW);

    assert_eq!(node.segment_text(), [(0, 10, Rtl)]);
  }

  #[test]
  fn segment_split_1_to_2() {
    let tree = render_tree(block([]), "");
    let mut node = TestNode::new(&tree);

    node.append(&format!("Hello {HEBREW}"));

    assert_eq!(node.segment_text(), [(0, 6, Ltr), (6, 16, Rtl)]);
  }

  #[test]
  fn segment_split_3_to_4() {
    let tree = render_tree(block([]), "");
    let mut node = TestNode::new(&tree);

    node.append("Hel");
    node.append("lo \u{05E2}");
    node.append("\u{05D1}\u{05E8}\u{05D9}\u{05EA}");

    assert_eq!(
      node.segment_text(),
      [(0, 3, Ltr), (3, 6, Ltr), (6, 8, Rtl), (8, 16, Rtl)]
    );
  }

  #[test]
  fn segment_bidi_override() {
    let tree = render_tree(block([]), "");
    let mut node = TestNode::new(&tree);

    node.append("Hello ");
    node.append_control('\u{202E}');
    node.append("ABC");
    node.append_control('\u{202C}');

    assert_eq!(
      node.segment_text(),
      [(0, 6, Ltr), (6, 9, Rtl), (9, 12, Rtl), (12, 15, Ltr)]
    );
  }

  #[test]
  fn segment_bidi_isolate() {
    let tree = render_tree(block([]), "");
    let mut node = TestNode::new(&tree);

    node.append("Hello ");
    node.append_control('\u{2067}');
    node.append(&format!("{HEBREW} "));
    node.append_control('\u{2066}');
    node.append("A");
    node.append_control('\u{2069}');
    node.append(HEBREW);
    node.append_control('\u{2069}');
    node.append(" World");

    assert_eq!(
      node.segment_text(),
      [
        (0, 6, Ltr),
        (6, 9, Ltr),
        (9, 20, Rtl),
        (20, 23, Rtl),
        (23, 24, Ltr),
        (24, 27, Rtl),
        (27, 37, Rtl),
        (37, 40, Ltr),
        (40, 46, Ltr),
      ]
    );
  }

  /// Floats and out-of-flow boxes resolve as U+FFFC, so they take the level around them.
  #[test]
  fn segment_bidi_with_out_of_flow() {
    let tree = render_tree(
      block([span(HEBREW), block([]).with_class_name("abs"), span(HEBREW)]),
      ".block { direction: rtl }",
    );
    let node = InlineNode::new(&tree);
    let mut data = node.collect_inlines();

    node.segment_text(&mut data);

    let out_of_flow = data
      .items
      .iter()
      .find(|item| item.item_type == OutOfFlowPositioned)
      .expect("an out-of-flow item");

    assert_eq!(out_of_flow.bidi_level(), 1);
    assert_eq!(data.base_direction, Rtl);
  }

  #[test]
  fn segment_ranges() {
    let tree = render_tree(block([span("\u{306F}orange\u{304C}"), span("text")]), "");
    let node = InlineNode::new(&tree);
    let mut data = node.collect_inlines();

    node.segment_text(&mut data);

    let segments = data.segments.as_ref().expect("several segments");
    let second = data
      .items
      .iter()
      .rposition(|item| item.item_type == OpenTag)
      .expect("the second span");
    let ends = |start, end, item_index| -> Vec<usize> {
      segments
        .ranges(start, end, item_index)
        .map(|range| range.end)
        .collect()
    };

    assert_eq!(data.text_content, "\u{306F}orange\u{304C}text");
    assert_eq!(ends(0, 16, 1), [3, 9, 12, 16]);
    assert_eq!(ends(0, 3, 1), [3]);
    assert_eq!(ends(4, 6, 1), [6]);
    assert_eq!(ends(9, 12, 1), [12]);
    assert_eq!(ends(0, 12, 1), [3, 9, 12]);
    assert_eq!(ends(4, 12, 1), [9, 12]);
    assert_eq!(ends(4, 14, 1), [9, 12, 14]);
    assert_eq!(ends(9, 14, 1), [12, 14]);
    assert_eq!(ends(12, 13, second), [13]);
    assert_eq!(ends(12, 14, second), [14]);
    assert_eq!(ends(13, 16, second), [16]);
  }
}
