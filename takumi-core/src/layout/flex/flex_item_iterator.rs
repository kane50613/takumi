//! Blink's [`FlexItemIterator`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/flex/flex_item_iterator.cc),
//! under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::layout::{
  block_break_token::BlockBreakToken, break_token::BreakToken, flex::flex_item::FlexLine,
};

/// Blink's `FlexItemIterator::Entry`: an item of a line to lay out, and where it resumes.
pub(crate) struct FlexItemEntry {
  pub(crate) item_index: usize,
  pub(crate) line_index: usize,
  pub(crate) token: Option<Rc<BlockBreakToken>>,
}

/// Blink's `FlexItemIterator`: the items of a container's lines a fragment lays out, first those
/// that resume after a break, then those that start. It reads the lines it walks at each step, as
/// the fragment marks them done.
pub(crate) struct FlexItemIterator {
  /// The container's break token, while its child break tokens remain.
  break_token: Option<Rc<BlockBreakToken>>,
  is_column: bool,
  /// The line and item of the next item that starts in this fragment.
  next_unstarted_item: Option<(usize, usize)>,
  child_token_index: usize,
  line_index: usize,
  item_index: usize,
  /// Where each column resumes, for a column container.
  next_item_index_for_line: Vec<usize>,
}

impl FlexItemIterator {
  pub(crate) fn new(
    flex_lines: &[FlexLine],
    break_token: Option<Rc<BlockBreakToken>>,
    is_column: bool,
  ) -> Self {
    let mut iterator = Self {
      break_token,
      is_column,
      next_unstarted_item: None,
      child_token_index: 0,
      line_index: 0,
      item_index: 0,
      next_item_index_for_line: Vec::new(),
    };

    if !flex_lines.is_empty() {
      iterator.next_unstarted_item = Some((0, 0));
      iterator.item_index = 1;
    }
    if let Some(token) = &iterator.break_token {
      if !token.child_break_tokens.is_empty() || token.has_seen_all_children {
        iterator.next_unstarted_item = None;
        iterator.item_index = 0;
      }
      if token.child_break_tokens.is_empty() {
        iterator.break_token = None;
      }
    }
    iterator
  }

  /// Blink's `NextItem`: the next item to lay out, none once every item laid out.
  pub(crate) fn next_item(
    &mut self,
    flex_lines: &[FlexLine],
    broke_before_row: bool,
  ) -> Option<FlexItemEntry> {
    let Some(break_token) = self.break_token.clone() else {
      let item = self.next_unstarted_item?;

      if self.next_unstarted_item.is_some() {
        self.next_unstarted_item = self.find_next_item(flex_lines, None);
      }
      return Some(FlexItemEntry {
        item_index: item.1,
        line_index: item.0,
        token: None,
      });
    };
    let child_break_tokens = &break_token.child_break_tokens;
    let BreakToken::Block(token) = child_break_tokens.get(self.child_token_index)? else {
      return None;
    };
    let token = Rc::clone(token);

    self.child_token_index += 1;

    let item = self.find_next_item(flex_lines, Some(&token));

    if self.is_column {
      if self.next_item_index_for_line.len() <= self.line_index {
        self.next_item_index_for_line.resize(self.line_index + 1, 0);
      }
      self.next_item_index_for_line[self.line_index] = self.item_index;
    }

    let entry = item.map(|(line_index, item_index)| FlexItemEntry {
      item_index,
      line_index,
      token: Some(Rc::clone(&token)),
    });

    if self.child_token_index == child_break_tokens.len() {
      let item_index = self.item_index.saturating_sub(1);

      if !self.is_column && (item_index != 0 || !token.is_break_before || !broke_before_row) {
        self.break_token = None;
        self.next_line(flex_lines);
      } else if !break_token.has_seen_all_children {
        if self.is_column {
          self.line_index = 0;
          self.item_index = self.next_item_index_for_line[0];
        }
        self.next_unstarted_item = self.find_next_item(flex_lines, None);
        self.break_token = None;
      }
    }
    entry
  }

  /// Blink's `HasMoreBreakTokens`.
  pub(crate) fn has_more_break_tokens(&self) -> bool {
    self.break_token.is_some()
  }

  /// Blink's `HasNextItemInLine`.
  pub(crate) fn has_next_item_in_line(&self, line_index: usize) -> bool {
    line_index == self.line_index && self.next_unstarted_item.is_some()
  }

  /// Blink's `NextLine`: moves past the rest of the current line.
  pub(crate) fn next_line(&mut self, flex_lines: &[FlexLine]) {
    if self.item_index == 0 {
      return;
    }

    self.line_index += 1;
    self.adjust_item_index_for_new_line();
    if self.break_token.is_none() {
      self.next_unstarted_item = self.find_next_item(flex_lines, None);
    }
  }

  /// Blink's `FindNextItem`: the next item, or the one `item_break_token` resumes.
  fn find_next_item(
    &mut self,
    flex_lines: &[FlexLine],
    item_break_token: Option<&BlockBreakToken>,
  ) -> Option<(usize, usize)> {
    while let Some(line) = flex_lines.get(self.line_index) {
      if !line.has_seen_all_children || item_break_token.is_some() {
        while let Some(item) = line.line_items_data.get(self.item_index) {
          let found = (self.line_index, self.item_index);

          self.item_index += 1;
          if item_break_token.is_none_or(|token| item.node == token.node) {
            return Some(found);
          }
        }
      }
      if self.is_column
        && item_break_token.is_none()
        && self.line_index + 1 == self.next_item_index_for_line.len()
      {
        break;
      }

      self.line_index += 1;
      self.adjust_item_index_for_new_line();
    }

    // A column container's break tokens resume its columns in turn, from the first.
    let token = item_break_token?;

    self.line_index = 0;
    self.item_index = self.next_item_index_for_line.first().copied().unwrap_or(0);
    self.find_next_item_in_lines(flex_lines, token)
  }

  /// The item `token` resumes, from the current line on.
  fn find_next_item_in_lines(
    &mut self,
    flex_lines: &[FlexLine],
    token: &BlockBreakToken,
  ) -> Option<(usize, usize)> {
    while let Some(line) = flex_lines.get(self.line_index) {
      while let Some(item) = line.line_items_data.get(self.item_index) {
        let found = (self.line_index, self.item_index);

        self.item_index += 1;
        if item.node == token.node {
          return Some(found);
        }
      }

      self.line_index += 1;
      self.adjust_item_index_for_new_line();
    }
    None
  }

  /// Blink's `AdjustItemIndexForNewLine`.
  fn adjust_item_index_for_new_line(&mut self) {
    self.item_index = self
      .next_item_index_for_line
      .get(self.line_index)
      .copied()
      .unwrap_or(0);
  }
}
