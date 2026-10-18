//! Blink's [`InlineItemsBuilder`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_items_builder.cc).

use super::outline::InlineOutline;
use super::{
  inline_item::{
    CollapseType, InlineItem, InlineItemType, InlineItems, LayoutObject, TextItemType,
  },
  inline_node_data::InlineNodeData,
};
use crate::{
  geometry::{Rect, Size},
  layout::{border::BorderProperties, tree::RenderNode},
  style::{BackgroundImage, ComputedStyle, Direction as TextDirection, VerticalAlign},
  text::character::{
    CARRIAGE_RETURN, FORM_FEED, LINE_FEED, OBJECT_REPLACEMENT_CHARACTER, TAB,
    ZERO_WIDTH_NON_JOINER, ZERO_WIDTH_SPACE, maybe_bidi_rtl,
  },
};

/// Blink's `IsCollapsibleSpace`.
fn is_collapsible_space(c: char) -> bool {
  matches!(c, ' ' | LINE_FEED | TAB | CARRIAGE_RETURN)
}

/// Blink's `ShouldIgnore`: carriage return and form feed.
/// <https://drafts.csswg.org/css-text-3/#white-space-processing>
fn should_ignore(c: char) -> bool {
  matches!(c, CARRIAGE_RETURN | FORM_FEED)
}

/// Blink's `IsControlItemCharacter`: characters taking a control item of their own.
fn is_control_item_character(c: char) -> bool {
  matches!(c, LINE_FEED | TAB | ZERO_WIDTH_NON_JOINER) || should_ignore(c)
}

/// Blink's `MoveToEndOfCollapsibleSpaces`: the end of the space run at `offset`, and whether it
/// holds a newline.
fn move_to_end_of_collapsible_spaces(string: &str, offset: usize) -> (usize, bool) {
  let end = string[offset..]
    .find(|c| !is_collapsible_space(c))
    .map_or(string.len(), |run| offset + run);

  (end, string[offset..end].contains(LINE_FEED))
}

/// Blink's `ShouldRemoveNewline`, per the segment break transformation rules: a newline next to a
/// zero width space goes. <https://drafts.csswg.org/css-text-3/#line-break-transform>
fn should_remove_newline(before: &str, space_index: usize, after: &str) -> bool {
  before[..space_index].ends_with(ZERO_WIDTH_SPACE) || after.starts_with(ZERO_WIDTH_SPACE)
}

/// Blink's `BoxInfo`: an inline box open while collecting.
struct BoxInfo<'c> {
  style: &'c ComputedStyle,
  item_index: usize,
  should_create_box_fragment: bool,
  /// The primary font's ascent and descent.
  text_metrics: Option<(f32, f32)>,
}

impl<'c> BoxInfo<'c> {
  fn new(item_index: usize, node: &'c RenderNode) -> Self {
    let style = &node.context.style;
    let text_metrics = node
      .context
      .primary_font_metrics(style, node.context.sizing.font_size)
      .map(|metrics| (metrics.ascent, metrics.descent));

    Self {
      style,
      item_index,
      should_create_box_fragment: node.needs_own_box_fragment(),
      text_metrics,
    }
  }

  /// Blink's `ShouldCreateBoxFragmentForChild`: a child that changes the box's size or position
  /// needs this box's fragment.
  fn should_create_box_fragment_for_child(
    &self,
    child: &BoxInfo<'_>,
    child_node: &RenderNode,
  ) -> bool {
    child_node.may_have_margin()
      || child.style.base_data.vertical_align != VerticalAlign::default()
      || self.text_metrics != child.text_metrics
  }
}

impl RenderNode {
  /// Whether this inline box paints, offsets, or contains something of its own, so it needs a
  /// fragment whatever it holds, as Blink's `LayoutInline::ComputeInitialShouldCreateBoxFragment`
  /// decides for the properties takumi has.
  fn needs_own_box_fragment(&self) -> bool {
    let style = &self.context.style;

    self.has_box_decoration_background()
      || self.may_have_padding()
      || self.may_have_margin()
      || InlineOutline::of(&self.context).is_some()
      || style.base_data.position.is_positioned()
      || style.creates_stacking_context(false)
      || style.contains_fixed_descendants()
  }

  /// Blink's `MayHaveMargin`.
  fn may_have_margin(&self) -> bool {
    self.margin_px() != Rect::ZERO
  }

  /// Blink's `MayHavePadding`.
  fn may_have_padding(&self) -> bool {
    self.padding_px() != Rect::ZERO
  }

  /// Blink's `HasBoxDecorationBackground`: a background, a border, or a shadow.
  pub(crate) fn has_box_decoration_background(&self) -> bool {
    let style = &self.context.style;
    let has_background_image = style
      .background_data
      .background_image
      .as_deref()
      .is_some_and(|images| images.iter().any(BackgroundImage::paints));

    style
      .background_data
      .background_color
      .resolve(self.context.current_color)
      .0[3]
      != 0
      || has_background_image
      || BorderProperties::from_context(&self.context, Size::ZERO, self.border_px())
        .has_visible_sides()
      || style
        .misc1_data
        .box_shadow
        .as_deref()
        .is_some_and(|shadows| !shadows.is_empty())
  }
}

/// Blink's `InlineItemsBuilder`, without reusing earlier items, offset mapping, SVG text chunks,
/// `text-combine-upright`, ruby, initial letters, or `unicode-bidi`.
pub(crate) struct InlineItemsBuilder<'c> {
  items: InlineItems<'c>,
  text: String,
  boxes: Vec<BoxInfo<'c>>,
  has_bidi_controls: bool,
  is_block_level: bool,
  has_floats: bool,
  has_out_of_flow_positioned: bool,
  is_bisect_line_break_disabled: bool,
  is_score_line_break_disabled: bool,
}

impl<'c> InlineItemsBuilder<'c> {
  pub(crate) fn new() -> Self {
    Self {
      items: InlineItems::new(),
      text: String::new(),
      boxes: Vec::new(),
      has_bidi_controls: false,
      is_block_level: true,
      has_floats: false,
      has_out_of_flow_positioned: false,
      is_bisect_line_break_disabled: false,
      is_score_line_break_disabled: false,
    }
  }

  /// Blink's `AppendItem`.
  fn append_item(
    &mut self,
    item_type: InlineItemType,
    start: usize,
    end: usize,
    layout_object: LayoutObject<'c>,
  ) -> &mut InlineItem<'c> {
    self
      .items
      .push(InlineItem::new(item_type, start, end, Some(layout_object)));

    let index = self.items.len() - 1;

    &mut self.items[index]
  }

  /// Blink's `LastItemToCollapseWith`: the last item not opaque to white space collapsing.
  fn last_item_to_collapse_with(&self) -> Option<usize> {
    self
      .items
      .iter()
      .rposition(|item| item.end_collapse_type != CollapseType::OpaqueToCollapsing)
  }

  /// Blink's `AppendTextItem`.
  fn append_text_item(
    &mut self,
    item_type: InlineItemType,
    string: &str,
    layout_object: LayoutObject<'c>,
  ) -> usize {
    let start_offset = self.text.len();

    self.text.push_str(string);

    let end = self.text.len();

    self.append_item(item_type, start_offset, end, layout_object);
    self.is_block_level = false;

    self.items.len() - 1
  }

  /// Blink's `AppendEmptyTextItem`: an item that keeps the text's box in the content.
  fn append_empty_text_item(&mut self, layout_object: LayoutObject<'c>) {
    let offset = self.text.len();
    let item = self.append_item(InlineItemType::Text, offset, offset, layout_object);

    item.end_collapse_type = CollapseType::OpaqueToCollapsing;
    item.is_empty_item = true;
    item.is_block_level = true;
  }

  /// Blink's `AppendGeneratedBreakOpportunity`.
  fn append_generated_break_opportunity(&mut self, layout_object: LayoutObject<'c>) {
    let index = self.append_break_opportunity(layout_object);
    let item = &mut self.items[index];

    item.is_generated_for_line_break = true;
    item.end_collapse_type = CollapseType::OpaqueToCollapsing;
  }

  /// Blink's `DidAppendForcedBreak`: a forced break makes several paragraphs, which bisecting
  /// widths cannot handle.
  fn did_append_forced_break(&mut self) {
    self.is_bisect_line_break_disabled = true;
  }

  /// Blink's `AppendText`: `string` of `layout_object` after `text-transform`.
  pub(crate) fn append_text(&mut self, string: &str, layout_object: LayoutObject<'c>) {
    if string.is_empty() {
      self.append_empty_text_item(layout_object);

      return;
    }

    let collapse = layout_object.style().inherited_data.white_space_collapse;

    self.restore_trailing_collapsible_space_if_removed();

    if collapse.should_preserve_white_spaces() {
      self.append_preserve_whitespace(string, layout_object);
    } else if collapse.should_preserve_breaks() {
      self.append_preserve_newline(string, layout_object);
    } else {
      self.append_collapse_whitespace(string, layout_object);
    }
  }

  /// Blink's `AppendCollapseWhitespace`: the first run, space or not, may interact with the last
  /// item; the rest alternate between runs of collapsible spaces and runs of other text.
  fn append_collapse_whitespace(&mut self, string: &str, layout_object: LayoutObject<'c>) {
    debug_assert!(!string.is_empty());

    let style = layout_object.style();
    let start_offset;
    let mut end_collapse = CollapseType::NotCollapsible;
    let mut i = 0;
    let mut space_run_has_newline = false;

    if string.starts_with(is_collapsible_space) {
      (i, space_run_has_newline) = move_to_end_of_collapsible_spaces(string, 0);

      // `<br>` preserves its newline whatever `white-space` says.
      if space_run_has_newline && string.len() == 1 && layout_object.is_br() {
        self.append_forced_break_collapse_whitespace(layout_object);

        return;
      }

      let mut insert_space;

      if let Some(index) = self.last_item_to_collapse_with() {
        let item = &self.items[index];
        let item_type = item.item_type;
        let item_start = item.start_offset();
        let item_end = item.end_offset();
        let item_has_newline = item.is_end_collapsible_newline;
        let item_wraps = item
          .style()
          .is_some_and(|style| style.inherited_data.text_wrap_mode.should_wrap_line());

        if item.end_collapse_type == CollapseType::NotCollapsible {
          // A space stands for this run.
          insert_space = true;
        } else {
          // The run collapses into the last item's space.
          debug_assert_eq!(item.end_collapse_type, CollapseType::Collapsible);

          insert_space = false;

          if (space_run_has_newline || item_has_newline)
            && item_type == InlineItemType::Text
            && should_remove_newline(&self.text, item_end - 1, &string[i..])
          {
            self.remove_trailing_collapsible_space(index);
            space_run_has_newline = false;
          } else if !item_wraps && style.inherited_data.text_wrap_mode.should_wrap_line() {
            // A collapsed space keeps its soft wrap opportunity, which matters only when the
            // first space is `nowrap` and this one wraps; never right after a forced break.
            // https://drafts.csswg.org/css-text-3/#collapse
            let after_forced_break =
              item_type == InlineItemType::Control && self.text.as_bytes()[item_start] == b'\n';

            if !after_forced_break {
              self.append_generated_break_opportunity(layout_object);
            }
          }
        }
      } else {
        // Leading spaces of the paragraph go, as CSS requires.
        insert_space = false;
      }

      if space_run_has_newline && should_remove_newline(&self.text, self.text.len(), &string[i..]) {
        insert_space = false;
        space_run_has_newline = false;
      }

      start_offset = self.text.len();

      if insert_space {
        self.text.push(' ');
      }

      if i == string.len() {
        end_collapse = CollapseType::Collapsible;
      }
    } else {
      // A newline in the last item's trailing space run may go by the segment break rules.
      if let Some(index) = self.last_item_to_collapse_with() {
        let item = &self.items[index];

        if item.end_collapse_type == CollapseType::Collapsible
          && item.is_end_collapsible_newline
          && should_remove_newline(&self.text, item.end_offset() - 1, string)
        {
          self.remove_trailing_collapsible_space(index);
        }
      }

      start_offset = self.text.len();
    }

    while i < string.len() {
      // Text up to the next collapsible space.
      debug_assert!(!string[i..].starts_with(is_collapsible_space));

      let start_of_non_space = i;

      i = string[i..]
        .find(is_collapsible_space)
        .map_or(string.len(), |run| i + run);
      self.text.push_str(&string[start_of_non_space..i]);

      if i == string.len() {
        end_collapse = CollapseType::NotCollapsible;

        break;
      }

      // A collapsible space run, which needs no look at the last item.
      (i, space_run_has_newline) = move_to_end_of_collapsible_spaces(string, i);

      let remove_newline =
        space_run_has_newline && should_remove_newline(&self.text, self.text.len(), &string[i..]);

      if remove_newline {
        // Removed, not collapsed.
        end_collapse = CollapseType::NotCollapsible;
        space_run_has_newline = false;
      } else {
        self.text.push(' ');
        end_collapse = CollapseType::Collapsible;
      }
    }

    debug_assert!(self.text.len() >= start_offset);

    if self.text.len() == start_offset {
      self.append_empty_text_item(layout_object);

      return;
    }

    let end = self.text.len();
    let item = self.append_item(InlineItemType::Text, start_offset, end, layout_object);

    item.end_collapse_type = end_collapse;
    item.is_end_collapsible_newline = space_run_has_newline;
    self.is_block_level = false;
  }

  /// Blink's `ShouldInsertBreakOpportunityAfterLeadingPreservedSpaces`: preserved spaces leading
  /// the first line or following a forced break end with a soft wrap opportunity.
  fn should_insert_break_opportunity_after_leading_preserved_spaces(
    &self,
    string: &str,
    style: &ComputedStyle,
    index: usize,
  ) -> bool {
    if style
      .inherited_data
      .white_space_collapse
      .should_collapse_white_spaces()
      || !style.inherited_data.text_wrap_mode.should_wrap_line()
      || index >= string.len()
      || string.as_bytes()[index] != b' '
    {
      return false;
    }

    if index != 0 {
      return string.as_bytes()[index - 1] == b'\n';
    }

    self.text.is_empty() || self.text.ends_with(LINE_FEED)
  }

  /// Blink's `InsertBreakOpportunityAfterLeadingPreservedSpaces`.
  fn insert_break_opportunity_after_leading_preserved_spaces(
    &mut self,
    string: &str,
    layout_object: LayoutObject<'c>,
    start: &mut usize,
  ) {
    if self.should_insert_break_opportunity_after_leading_preserved_spaces(
      string,
      layout_object.style(),
      *start,
    ) {
      let end = string[*start..]
        .find(|c| c != ' ')
        .map_or(string.len(), |run| *start + run);

      self.append_text_item(InlineItemType::Text, &string[*start..end], layout_object);
      self.append_generated_break_opportunity(layout_object);
      *start = end;
    }
  }

  /// Blink's `AppendPreserveWhitespace`: control characters take control items of their own, so
  /// the line breaker needs no special case.
  fn append_preserve_whitespace(&mut self, string: &str, layout_object: LayoutObject<'c>) {
    let mut start = 0;

    // The soft wrap opportunity after leading preserved spaces, made explicit.
    // https://drafts.csswg.org/css-text-3/#white-space-phase-1
    self.insert_break_opportunity_after_leading_preserved_spaces(string, layout_object, &mut start);

    let length = string.len();
    let find_control = |from: usize| {
      string[from..]
        .find(is_control_item_character)
        .map_or(length, |offset| from + offset)
    };
    let mut control = find_control(start);

    while start < length {
      if control != start {
        self.append_text_item(InlineItemType::Text, &string[start..control], layout_object);

        if control >= length {
          break;
        }

        start = control;
      }

      let Some(c) = string[start..].chars().next() else {
        break;
      };

      match c {
        LINE_FEED => {
          self.append_forced_break(layout_object);
          start += 1;
          // Spaces after a forced break lead a line.
          self.insert_break_opportunity_after_leading_preserved_spaces(
            string,
            layout_object,
            &mut start,
          );
        }
        TAB => {
          let tab_end = string[start + 1..]
            .find(|c| c != TAB)
            .map_or(length, |offset| start + 1 + offset);
          let index = self.append_text_item(
            InlineItemType::Control,
            &string[start..tab_end],
            layout_object,
          );

          self.items[index].text_type = TextItemType::FlowControl;
          start = tab_end;
          self.is_score_line_break_disabled = true;
        }
        ZERO_WIDTH_NON_JOINER => {
          // It splits the item but stays text.
          control = find_control(start + c.len_utf8());

          continue;
        }
        _ => {
          debug_assert!(is_control_item_character(c));

          let index = self.append(InlineItemType::Control, c, layout_object);

          self.items[index].text_type = TextItemType::FlowControl;
          start += c.len_utf8();
        }
      }

      if start >= length {
        break;
      }

      control = find_control(start);
    }
  }

  /// Blink's `AppendPreserveNewline`: newlines force breaks, and the text between them collapses.
  fn append_preserve_newline(&mut self, string: &str, layout_object: LayoutObject<'c>) {
    let mut start = 0;

    while start < string.len() {
      if string.as_bytes()[start] == b'\n' {
        self.append_forced_break_collapse_whitespace(layout_object);
        start += 1;

        continue;
      }

      let end = string[start..]
        .find(LINE_FEED)
        .map_or(string.len(), |offset| start + offset);

      self.append_collapse_whitespace(&string[start..end], layout_object);
      start = end;
    }
  }

  /// Blink's `AppendForcedBreak`.
  fn append_forced_break(&mut self, layout_object: LayoutObject<'c>) {
    let index = self.append(InlineItemType::Control, LINE_FEED, layout_object);
    let item = &mut self.items[index];

    item.text_type = TextItemType::ForcedLineBreak;
    // Spaces after a forced break lead a line and collapse, as after a collapsible space.
    item.end_collapse_type = CollapseType::Collapsible;
    item.is_end_collapsible_newline = false;
    self.did_append_forced_break();
  }

  /// Blink's `AppendForcedBreakCollapseWhitespace`: collapsible spaces before a preserved
  /// newline go.
  fn append_forced_break_collapse_whitespace(&mut self, layout_object: LayoutObject<'c>) {
    self.remove_trailing_collapsible_space_if_exists();
    self.append_forced_break(layout_object);
  }

  /// Blink's `AppendBreakOpportunity`: a zero width space.
  fn append_break_opportunity(&mut self, layout_object: LayoutObject<'c>) -> usize {
    let index = self.append_opaque_char(InlineItemType::Control, ZERO_WIDTH_SPACE, layout_object);

    self.items[index].text_type = TextItemType::FlowControl;

    index
  }

  /// Blink's `Append` of one character.
  fn append(
    &mut self,
    item_type: InlineItemType,
    character: char,
    layout_object: LayoutObject<'c>,
  ) -> usize {
    debug_assert_ne!(character, ' ');

    self.text.push(character);

    let end_offset = self.text.len();
    let start = end_offset - character.len_utf8();
    let item = self.append_item(item_type, start, end_offset, layout_object);

    self.is_block_level &= item.is_block_level;

    self.items.len() - 1
  }

  /// Blink's `AppendAtomicInline`: an object replacement character for the bidi algorithm.
  pub(crate) fn append_atomic_inline(&mut self, layout_object: LayoutObject<'c>) {
    self.restore_trailing_collapsible_space_if_removed();
    self.append(
      InlineItemType::AtomicInline,
      OBJECT_REPLACEMENT_CHARACTER,
      layout_object,
    );

    // Its inline box's height may differ from it, so that box needs a fragment.
    if let Some(current_box) = self.boxes.last_mut()
      && !current_box.should_create_box_fragment
    {
      current_box.should_create_box_fragment = true;
      self.items[current_box.item_index].should_create_box_fragment = true;
    }
  }

  /// Blink's `AppendFloating`: out of flow, so opaque to text processing.
  /// <https://drafts.csswg.org/css-text-3/#text-encoding>
  pub(crate) fn append_floating(&mut self, layout_object: LayoutObject<'c>) {
    self.append_opaque(InlineItemType::Floating, layout_object);
    self.has_floats = true;
    self.is_bisect_line_break_disabled = true;
  }

  /// Blink's `AppendOutOfFlowPositioned`.
  pub(crate) fn append_out_of_flow_positioned(&mut self, layout_object: LayoutObject<'c>) {
    self.append_opaque(InlineItemType::OutOfFlowPositioned, layout_object);
    self.has_out_of_flow_positioned = true;
  }

  /// Blink's `AppendOpaque` of one character.
  fn append_opaque_char(
    &mut self,
    item_type: InlineItemType,
    character: char,
    layout_object: LayoutObject<'c>,
  ) -> usize {
    let index = self.append(item_type, character, layout_object);

    self.items[index].end_collapse_type = CollapseType::OpaqueToCollapsing;

    index
  }

  /// Blink's `AppendOpaque` without a character.
  pub(crate) fn append_opaque(
    &mut self,
    item_type: InlineItemType,
    layout_object: LayoutObject<'c>,
  ) {
    let end_offset = self.text.len();
    let item = self.append_item(item_type, end_offset, end_offset, layout_object);

    item.end_collapse_type = CollapseType::OpaqueToCollapsing;
    self.is_block_level &= item.is_block_level;
  }

  /// Blink's `RemoveTrailingCollapsibleSpaceIfExists`.
  fn remove_trailing_collapsible_space_if_exists(&mut self) {
    if let Some(index) = self.last_item_to_collapse_with()
      && self.items[index].end_collapse_type == CollapseType::Collapsible
    {
      self.remove_trailing_collapsible_space(index);
    }
  }

  /// Blink's `RemoveTrailingCollapsibleSpace`: removes the space ending item `index`, keeping the
  /// item even when empty.
  fn remove_trailing_collapsible_space(&mut self, index: usize) {
    let item = &self.items[index];

    debug_assert_eq!(item.end_collapse_type, CollapseType::Collapsible);
    debug_assert!(item.length() > 0);

    // A forced break poses as a collapsible space, and stays.
    if item.item_type != InlineItemType::Text {
      debug_assert!(matches!(
        item.item_type,
        InlineItemType::Control | InlineItemType::BlockInInline
      ));

      return;
    }

    let space_offset = item.end_offset() - 1;

    debug_assert_eq!(self.text.as_bytes()[space_offset], b' ');

    self.text.remove(space_offset);

    let item = &mut self.items[index];

    item.set_end_offset(item.end_offset() - 1);
    item.end_collapse_type = CollapseType::Collapsed;

    // Items after it, which hold no characters, move back.
    for item in &mut self.items[index + 1..] {
      item.set_offset(item.start_offset() - 1, item.end_offset() - 1);
    }
  }

  /// Blink's `RestoreTrailingCollapsibleSpaceIfRemoved`.
  fn restore_trailing_collapsible_space_if_removed(&mut self) {
    if let Some(index) = self.last_item_to_collapse_with()
      && self.items[index].end_collapse_type == CollapseType::Collapsed
    {
      self.restore_trailing_collapsible_space(index);
    }
  }

  /// Blink's `RestoreTrailingCollapsibleSpace`.
  fn restore_trailing_collapsible_space(&mut self, index: usize) {
    let item = &mut self.items[index];

    debug_assert_eq!(item.end_collapse_type, CollapseType::Collapsed);

    self.text.insert(item.end_offset(), ' ');
    item.set_end_offset(item.end_offset() + 1);
    item.end_collapse_type = CollapseType::Collapsible;

    for item in &mut self.items[index + 1..] {
      item.set_offset(item.start_offset() + 1, item.end_offset() + 1);
    }
  }

  /// Blink's `EnterBlock`: an RTL block needs bidi.
  pub(crate) fn enter_block(&mut self, style: &ComputedStyle) {
    if style.inherited_data.direction == TextDirection::Rtl {
      self.has_bidi_controls = true;
    }
  }

  /// Blink's `EnterInline`.
  pub(crate) fn enter_inline(&mut self, node: &'c RenderNode) {
    self.append_opaque(InlineItemType::OpenTag, LayoutObject::Inline(node));

    let current_box = BoxInfo::new(self.items.len() - 1, node);

    self.items[current_box.item_index].should_create_box_fragment =
      current_box.should_create_box_fragment;

    // The parent needs a fragment when this box changes its size or position.
    if let Some(parent_box) = self.boxes.last_mut()
      && !parent_box.should_create_box_fragment
      && parent_box.should_create_box_fragment_for_child(&current_box, node)
    {
      parent_box.should_create_box_fragment = true;
      self.items[parent_box.item_index].should_create_box_fragment = true;
    }

    self.boxes.push(current_box);
  }

  /// Blink's `ExitBlock`: trailing newlines and collapsible spaces go.
  pub(crate) fn exit_block(&mut self) {
    self.remove_trailing_collapsible_space_if_exists();
  }

  /// Blink's `ExitInline`.
  pub(crate) fn exit_inline(&mut self, node: &'c RenderNode) {
    if let Some(current_box) = self.boxes.last() {
      let open_item_index = current_box.item_index;

      // An empty inline box needs a fragment for its position and size.
      if !current_box.should_create_box_fragment {
        for i in (open_item_index..self.items.len()).rev() {
          let item = &self.items[i];

          if i == open_item_index {
            self.items[i].should_create_box_fragment = true;

            break;
          }

          // Floats, out-of-flow boxes, collapsed spaces, and trailing collapsible spaces leave it
          // empty.
          if item.is_empty_item || item.is_collapsible_space_only() {
            continue;
          }

          break;
        }
      }

      self.boxes.pop();

      let should_create_box_fragment = self.items[open_item_index].should_create_box_fragment;

      self.append_opaque(InlineItemType::CloseTag, LayoutObject::Inline(node));

      let last = self.items.len() - 1;

      self.items[last].should_create_box_fragment = should_create_box_fragment;
    }
  }

  /// Blink's `SetIsSymbolMarker`.
  pub(crate) fn set_is_symbol_marker(&mut self) {
    if let Some(item) = self.items.last_mut() {
      item.set_is_symbol_marker();
    }
  }

  /// Blink's `DidFinishCollectInlines`. Bidi is on for any text that may hold right-to-left
  /// characters; `SegmentText` turns it off when none resolve so.
  pub(crate) fn finish(self) -> InlineNodeData<'c> {
    let text_content = self.text;
    let has_non_orc_16bit = text_content
      .chars()
      .any(|c| u32::from(c) >= 0x100 && c != OBJECT_REPLACEMENT_CHARACTER);
    let is_bidi_enabled =
      self.has_bidi_controls || (has_non_orc_16bit && maybe_bidi_rtl(&text_content));

    InlineNodeData {
      text_content,
      items: self.items,
      has_non_orc_16bit,
      is_bidi_enabled,
      has_floats: self.has_floats,
      has_out_of_flow_positioned: self.has_out_of_flow_positioned,
      is_block_level: self.is_block_level,
      is_bisect_line_break_disabled: self.is_bisect_line_break_disabled,
      is_score_line_break_disabled: self.is_score_line_break_disabled,
      ..InlineNodeData::default()
    }
  }
}
