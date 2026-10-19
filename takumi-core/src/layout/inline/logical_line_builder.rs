//! Blink's [`LogicalLineBuilder`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/logical_line_builder.cc),
//! without ruby, initial letters, `text-combine-upright`, line-clamp ellipses, or
//! quirks mode.

use std::{mem, rc::Rc};

use super::{
  inline_box_state::{InlineBoxState, InlineLayoutStateStack, LineHeightMode},
  inline_item::{InlineItem, InlineItemType, LayoutObject},
  inline_item_result::{HyphenResult, InlineItemResult, find_text_scale},
  inline_node_data::StyleFonts,
  line_breaker::LineBreaker,
  line_info::LineInfo,
  logical_line_item::{LineFragment, LogicalLineItem, LogicalLineItems},
  text_fit_scale::TextFitBlockScale,
};
use crate::{
  fonts::shaping::shape_result_view::ShapeResultView,
  layout::{constraint_space::ConstraintSpace, logical_rect::LogicalOffset},
  layout_unit::LayoutUnit,
  style::Direction as TextDirection,
  text::bidi_paragraph::BidiParagraph,
};

/// The bidi level of items opaque to bidi reordering, above any resolved level.
const OPAQUE_BIDI_LEVEL: u8 = 0xff;

/// Blink's `LogicalLineBuilder`: places a broken line's items in its line box, the baseline at
/// zero, and reorders them visually.
pub(crate) struct LogicalLineBuilder<'s, 'a, 'c> {
  constraint_space: &'s ConstraintSpace,
  box_states: &'s mut InlineLayoutStateStack<'a, 'c>,
  should_scale_line_height: bool,
  pub(crate) has_out_of_flow_positioned_items: bool,
  pub(crate) has_floating_items: bool,
  pub(crate) has_relative_positioned_items: bool,
}

impl<'s, 'a, 'c> LogicalLineBuilder<'s, 'a, 'c> {
  pub(crate) fn new(
    constraint_space: &'s ConstraintSpace,
    box_states: &'s mut InlineLayoutStateStack<'a, 'c>,
    should_scale_line_height: bool,
  ) -> Self {
    Self {
      constraint_space,
      box_states,
      should_scale_line_height,
      has_out_of_flow_positioned_items: false,
      has_floating_items: false,
      has_relative_positioned_items: false,
    }
  }

  /// Blink's `RebuildBoxStates`: opens the boxes `items[start_item_index..end_item_index]` leaves
  /// open, for a line starting after them.
  pub(crate) fn rebuild_box_states(
    &mut self,
    line_info: &LineInfo<'a, 'c>,
    start_item_index: usize,
    end_item_index: usize,
  ) {
    let items_data = line_info.items_data;
    let mut line_box = LogicalLineItems::default();

    self
      .box_states
      .on_begin_place_items(line_info, LineHeightMode::Normal, &mut line_box);

    let open_items = items_data.open_tag_items(start_item_index..end_item_index);
    let num_open_items = open_items.len();

    for (index, item) in open_items.into_iter().enumerate() {
      let mut item_result = InlineItemResult::new(
        item,
        item.index,
        item.start_offset()..item.end_offset(),
        false,
        false,
        false,
      );

      LineBreaker::compute_open_tag_result(item, &mut item_result);

      let text_scale = find_text_scale(
        self.should_scale_line_height,
        &line_info.results,
        0,
        num_open_items - index - 1,
      );

      self.handle_open_tag(
        item,
        &item_result,
        &text_scale,
        &mut line_box,
        &items_data.fonts,
      );
    }
  }

  /// Blink's `CreateLine`.
  pub(crate) fn create_line(
    &mut self,
    line_info: &LineInfo<'a, 'c>,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    // Every item's height, with the baseline at zero; the line box's height moves it later.
    self.box_states.set_is_empty_line(line_info.is_empty_line);
    self
      .box_states
      .on_begin_place_items(line_info, LineHeightMode::Normal, line_box);
    self.handle_item_results(line_info, line_box);
    self
      .box_states
      .on_end_place_items(self.constraint_space, line_box);

    if line_info.items_data.is_bidi_enabled {
      self.box_states.prepare_for_reorder(line_box);
      line_box.bidi_reorder(line_info.base_direction);
      self.box_states.update_after_reorder(line_box);
    } else {
      debug_assert_eq!(line_info.base_direction, TextDirection::Ltr);
    }
  }

  /// Blink's `HandleItemResults`.
  fn handle_item_results(
    &mut self,
    line_info: &LineInfo<'a, 'c>,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    let fonts = &line_info.items_data.fonts;

    for (index, item_result) in line_info.results.iter().enumerate() {
      let item = item_result.item;

      match item.item_type {
        InlineItemType::Text => {
          // Empty or fully collapsed text takes no part in layout.
          if item_result.length() == 0 {
            continue;
          }

          let Some(shape_result) = &item_result.shape_result else {
            continue;
          };
          let box_state = self.box_states.current();
          let scale = item_result
            .text_fit_scale
            .as_ref()
            .map_or(1.0, |text_fit_scale| text_fit_scale.scale);

          // Under `line-height: normal`, every font the text used counts.
          if box_state.include_used_fonts {
            box_state.accumulate_used_fonts(shape_result, scale);
          }

          match &item_result.hyphen {
            Some(hyphen) if item_result.is_hyphenated => {
              let hyphen_inline_size = hyphen.inline_size();

              line_box.add_child(LogicalLineItem::text(
                item,
                item_result,
                box_state.text_top,
                LayoutUnit::from_float(
                  (item_result.inline_size - hyphen_inline_size).to_float() * scale,
                ),
                box_state.text_height,
              ));
              place_hyphen(item_result, hyphen, hyphen_inline_size, line_box, box_state);
            }
            _ => line_box.add_child(LogicalLineItem::text(
              item,
              item_result,
              box_state.text_top,
              LayoutUnit::from_float(item_result.inline_size.to_float() * scale),
              box_state.text_height,
            )),
          }
        }
        InlineItemType::Control => self.place_control_item(item, item_result, line_box),
        InlineItemType::OpenTag => {
          let text_scale = find_text_scale(
            self.should_scale_line_height,
            &line_info.results,
            index + 1,
            0,
          );

          self.handle_open_tag(item, item_result, &text_scale, line_box, fonts);
        }
        InlineItemType::CloseTag => self.handle_close_tag(item, line_box),
        InlineItemType::AtomicInline => {
          self.place_atomic_inline(item, item_result, line_box, fonts);
          self.has_relative_positioned_items |= is_relative_positioned(item);
        }
        // A list marker's metrics count only in quirks mode.
        InlineItemType::ListMarker => {}
        InlineItemType::OutOfFlowPositioned => {
          // An inline-level box takes its own direction; a block-level one, its container's.
          let is_inline_level = item
            .style()
            .is_some_and(|style| style.original_display.is_inline_level());
          let container_direction = if is_inline_level {
            item.direction()
          } else {
            self.constraint_space.direction
          };

          line_box.add_child(LogicalLineItem::out_of_flow_positioned(
            item,
            container_direction,
          ));
          self.has_out_of_flow_positioned_items = true;
        }
        InlineItemType::Floating => {
          line_box.add_child(match item_result.positioned_float {
            Some(positioned_float) => LogicalLineItem::positioned_float(item, positioned_float),
            None => LogicalLineItem::unpositioned_float(item),
          });
          self.has_floating_items = true;
          self.has_relative_positioned_items |= is_relative_positioned(item);
        }
        InlineItemType::BidiControl => {
          line_box.add_child(LogicalLineItem::bidi_control(item.bidi_level()));
        }
      }
    }
  }

  /// Blink's `HandleOpenTag`.
  fn handle_open_tag(
    &mut self,
    item: &'a InlineItem<'c>,
    item_result: &InlineItemResult<'a, 'c>,
    text_scale: &TextFitBlockScale,
    line_box: &mut LogicalLineItems<'a, 'c>,
    fonts: &StyleFonts,
  ) {
    let Some(object) = item.layout_object else {
      return;
    };

    // Even an empty box grows the line to its own line height.
    // <https://drafts.csswg.org/css2/visudet.html#line-height>
    self
      .box_states
      .on_open_tag_with_placeholder(
        item,
        object.context(),
        item_result,
        text_scale,
        line_box,
        fonts,
      )
      .compute_text_metrics(Some(text_scale));
  }

  /// Blink's `HandleCloseTag`.
  fn handle_close_tag(&mut self, item: &InlineItem<'c>, line_box: &mut LogicalLineItems<'a, 'c>) {
    if item.layout_object.is_none() {
      return;
    }

    self
      .box_states
      .on_close_tag(self.constraint_space, line_box);
  }

  /// Blink's `PlaceControlItem`: a tab, a line feed, or a zero-width space.
  fn place_control_item(
    &mut self,
    item: &'a InlineItem<'c>,
    item_result: &InlineItemResult<'a, 'c>,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    debug_assert_eq!(item.item_type, InlineItemType::Control);

    // A break opportunity white space collapsing generated has no fragment.
    if item.is_generated_for_line_break || item_result.length() == 0 {
      return;
    }

    let box_state = self.box_states.current();

    line_box.add_child(LogicalLineItem::text(
      item,
      item_result,
      box_state.text_top,
      item_result.inline_size,
      box_state.text_height,
    ));
  }

  /// Blink's `PlaceAtomicInline`: the atomic inline as a box of its own, which `vertical-align`
  /// aligns.
  fn place_atomic_inline(
    &mut self,
    item: &'a InlineItem<'c>,
    item_result: &InlineItemResult<'a, 'c>,
    line_box: &mut LogicalLineItems<'a, 'c>,
    fonts: &StyleFonts,
  ) {
    let Some(object) = item.layout_object else {
      return;
    };
    let box_state =
      self
        .box_states
        .on_open_tag(item, object.context(), item_result, line_box, fonts);
    let inline_offset = box_state.margins.inline_start + item_result.spacing_before;

    place_layout_result(
      item_result,
      object,
      line_box,
      Some(box_state),
      inline_offset,
    );
    self
      .box_states
      .on_close_tag(self.constraint_space, line_box);
  }
}

/// Blink's `PlaceHyphen`.
fn place_hyphen<'a, 'c>(
  item_result: &InlineItemResult<'a, 'c>,
  hyphen: &HyphenResult,
  mut hyphen_inline_size: LayoutUnit,
  line_box: &mut LogicalLineItems<'a, 'c>,
  box_state: &InlineBoxState<'a, 'c>,
) {
  if let Some(text_fit_scale) = &item_result.text_fit_scale {
    hyphen_inline_size *= text_fit_scale.scale;
  }

  line_box.add_child(LogicalLineItem::generated_text(
    item_result.item,
    Rc::new(ShapeResultView::create_from_result(&hyphen.shape_result)),
    hyphen.text.clone(),
    item_result.text_fit_scale.clone(),
    box_state.text_top,
    hyphen_inline_size,
    box_state.text_height,
  ));
}

/// Blink's `PlaceLayoutResult`: the laid out box, its margin box around the baseline.
fn place_layout_result<'a, 'c>(
  item_result: &InlineItemResult<'a, 'c>,
  object: LayoutObject<'c>,
  line_box: &mut LogicalLineItems<'a, 'c>,
  box_state: Option<&mut InlineBoxState<'a, 'c>>,
  inline_offset: LayoutUnit,
) {
  let Some(layout_result) = item_result.layout_result else {
    return;
  };
  let metrics = layout_result.baseline_metrics(&item_result.margins);

  if let Some(box_state) = box_state {
    box_state.metrics.unite(&metrics);
  }

  let line_top = item_result.margins.line_over - metrics.ascent;

  line_box.add_child(LogicalLineItem::with_layout_result(
    LineFragment::AtomicInline {
      node: object.node(),
      layout_result,
    },
    LogicalOffset::new(inline_offset, line_top),
    item_result.inline_size,
    0,
    item_result.item.bidi_level(),
  ));
}

fn is_relative_positioned(item: &InlineItem<'_>) -> bool {
  item
    .style()
    .is_some_and(|style| style.base_data.position.is_relative())
}

impl LogicalLineItems<'_, '_> {
  /// Blink's `LogicalLineBuilder::BidiReorder`: the items in visual order, by runs rather than
  /// characters. UAX#9 L1 applies only to whole items of trailing spaces.
  pub(crate) fn bidi_reorder(&mut self, base_direction: TextDirection) {
    if self.is_empty() {
      return;
    }

    // Trailing items and items of only trailing spaces take the paragraph's level.
    let base_direction_level = base_direction.bidi_level();
    let mut levels: Vec<u8> = self
      .children
      .iter()
      .map(|item| {
        if item.is_opaque_to_bidi_reordering() {
          OPAQUE_BIDI_LEVEL
        } else if item.has_only_bidi_trailing_spaces {
          base_direction_level
        } else {
          item.bidi_level
        }
      })
      .collect();

    // Opaque items take the level after them.
    let mut last_level = base_direction_level;

    for level in levels.iter_mut().rev() {
      if *level == OPAQUE_BIDI_LEVEL {
        *level = last_level;
      } else {
        last_level = *level;
      }
    }

    let mut logical_items = mem::take(&mut self.children);

    self.children = BidiParagraph::indices_in_visual_order(&levels)
      .into_iter()
      .map(|logical_index| mem::take(&mut logical_items[logical_index]))
      .collect();
  }
}

#[cfg(test)]
mod tests {
  use super::LogicalLineBuilder;
  use crate::{
    layout::{
      constraint_space::ConstraintSpace,
      exclusions::{exclusion_space::ExclusionSpace, layout_opportunity::LineLayoutOpportunity},
      inline::{
        inline_box_state::InlineLayoutStateStack,
        inline_node::InlineNode,
        leading_floats::LeadingFloats,
        line_breaker::{LineBreaker, LineBreakerMode},
        line_info::LineInfo,
        logical_line_item::{LineFragment, LogicalLineItems},
        testing::{TestAtomicInline, render_tree, span},
      },
      node::Node,
      tree::RenderNode,
    },
    layout_unit::LayoutUnit,
  };

  /// The first line of `block`, as Blink's `InlineLayoutAlgorithm::CreateLine` places it before
  /// aligning: each item's text or kind, its inline offset and size, and its block offset from
  /// the baseline; then the line box's ascent and descent.
  fn place_first_line(block: &RenderNode, available_width: f32) -> (Vec<String>, (f32, f32)) {
    let node = InlineNode::new(block);
    let data = node.prepare_layout();
    let available_width = LayoutUnit::from_float(available_width);
    let space = ConstraintSpace::new(
      Some(available_width),
      block.context.style.inherited_data.direction,
    );
    let mut atomic_inline = TestAtomicInline;
    let mut exclusion_space = ExclusionSpace::default();
    let mut line_info = LineInfo::new(&data, &block.context);

    LineBreaker::new(
      node,
      &data,
      LineBreakerMode::Content,
      space,
      LineLayoutOpportunity::new(available_width),
      &LeadingFloats::default(),
      None,
      &mut atomic_inline,
      &mut exclusion_space,
    )
    .next_line(&mut line_info);

    let mut box_states = InlineLayoutStateStack::default();
    let mut line_box = LogicalLineItems::default();

    LogicalLineBuilder::new(&space, &mut box_states, false).create_line(&line_info, &mut line_box);
    box_states.compute_inline_positions(&mut line_box, LayoutUnit::ZERO, false);

    let metrics = box_states.line_box_state().metrics;

    box_states.apply_relative_positioning(&space, &mut line_box);
    box_states.create_box_fragments(&mut line_box);

    let items = line_box
      .children
      .iter()
      .map(|item| {
        let label = match &item.layout_result {
          Some(LineFragment::InlineBox(fragment)) => format!(
            "box{}{}",
            if fragment.has_line_left_edge { "[" } else { "" },
            if fragment.has_line_right_edge {
              "]"
            } else {
              ""
            },
          ),
          Some(LineFragment::AtomicInline { .. }) => "atomic".to_owned(),
          Some(LineFragment::Float { .. }) => "float".to_owned(),
          None if item.inline_item.is_some() => {
            format!("{:?}", &data.text_content[item.text_offset.clone()])
          }
          None => "placeholder".to_owned(),
        };
        let inline_size = if item.is_inline_box() {
          item.rect.size.inline_size
        } else {
          item.inline_size
        };

        format!(
          "{label} {}+{} @{}",
          item.rect.offset.inline_offset.to_float(),
          inline_size.to_float(),
          item.rect.offset.block_offset.to_float(),
        )
      })
      .collect();

    (
      items,
      (metrics.ascent.to_float(), metrics.descent.to_float()),
    )
  }

  /// Text sits on the baseline, its top the font's ascent above; the line box starts from the
  /// strut, Geist Mono's 10px ascent and 3px descent with `line-height: 1` taking 3px of leading.
  #[test]
  fn an_inline_box_adds_its_edges() {
    let block = render_tree(
      [
        Node::text("abc"),
        span("de").with_class_name("span edged"),
        Node::text("f"),
      ],
      ".edged { padding-left: 2px; padding-right: 2px; border-left-width: 1px; \
        border-left-style: solid; margin-left: 3px }",
    );

    assert_eq!(
      place_first_line(&block, 200.0),
      (
        vec![
          "\"abc\" 0+18 @-10".to_owned(),
          "box[] 21+17 @-10".to_owned(),
          "\"de\" 24+12 @-10".to_owned(),
          "\"f\" 38+6 @-10".to_owned(),
        ],
        (8.0, 2.0)
      )
    );
  }

  /// `super` raises the box by a third of its parent's font size, plus one pixel.
  #[test]
  fn vertical_align_super_raises_the_box() {
    let block = render_tree(
      [Node::text("a"), span("b").with_class_name("span raised")],
      ".raised { vertical-align: super }",
    );

    assert_eq!(
      place_first_line(&block, 200.0),
      (
        vec![
          "\"a\" 0+6 @-10".to_owned(),
          "\"b\" 6+6 @-14.328125".to_owned(),
        ],
        (12.328125, 2.0)
      )
    );
  }

  /// `top` and `bottom` wait for the line box's size. The taller `top` box keeps the line's top
  /// and extends its bottom, where the `bottom` box then sits.
  #[test]
  fn vertical_align_top_and_bottom_align_to_the_line_box() {
    let block = render_tree(
      [
        Node::text("a"),
        span("b").with_class_name("span tall"),
        span("c").with_class_name("span low"),
      ],
      ".tall { vertical-align: top; line-height: 30px } .low { vertical-align: bottom }",
    );

    assert_eq!(
      place_first_line(&block, 200.0),
      (
        vec![
          "\"a\" 0+6 @-10".to_owned(),
          "\"b\" 6+6 @0".to_owned(),
          "\"c\" 12+6 @10".to_owned(),
        ],
        (8.0, 22.0)
      )
    );
  }

  /// `position: relative` moves the box and what it holds.
  #[test]
  fn a_relative_box_moves_with_its_children() {
    let block = render_tree(
      [Node::text("a"), span("b").with_class_name("span shifted")],
      ".shifted { position: relative; left: 5px; top: 3px }",
    );

    assert_eq!(
      place_first_line(&block, 200.0),
      (
        vec![
          "\"a\" 0+6 @-10".to_owned(),
          "box[] 11+6 @-7".to_owned(),
          "\"b\" 11+6 @-7".to_owned(),
        ],
        (8.0, 2.0)
      )
    );
  }

  /// An atomic inline without a baseline sits on its margin box's bottom edge.
  #[test]
  fn an_atomic_inline_without_a_baseline_sits_on_its_bottom() {
    let block = render_tree(
      [
        Node::text("abc"),
        Node::container([]).with_class_name("atomic"),
      ],
      "",
    );

    assert_eq!(
      place_first_line(&block, 200.0),
      (
        vec![
          "\"abc\" 0+18 @-10".to_owned(),
          "atomic 18+18 @-10".to_owned()
        ],
        (10.0, 2.0)
      )
    );
  }

  /// The span's RTL text reorders before the text ahead of it, splitting the span in two: its
  /// line-left edge stays on the first part, and its line-right edge moves to the last.
  #[test]
  fn bidi_reordering_splits_a_box() {
    let block = render_tree(
      [
        Node::text("\u{5D0}\u{5D1} "),
        span("\u{5D2}\u{5D3} ef").with_class_name("span edged"),
      ],
      ".edged { padding-left: 1px; padding-right: 2px }",
    );

    assert_eq!(
      place_first_line(&block, 200.0),
      (
        vec![
          "box[ 0+11 @-10".to_owned(),
          "\"\u{5D2}\u{5D3}\" 1+10 @-10".to_owned(),
          "placeholder 11+0 @-10".to_owned(),
          "\"\u{5D0}\u{5D1} \" 11+16 @-10".to_owned(),
          "box] 27+20 @-10".to_owned(),
          "\" ef\" 27+18 @-10".to_owned(),
        ],
        (8.0, 2.0)
      )
    );
  }
}
