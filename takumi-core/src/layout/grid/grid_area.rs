//! Blink's [`GridSpan` and `GridArea`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/style/grid_area.h).
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use crate::layout::grid::grid_style::{GRID_MAX_TRACKS, GridTrackSizingDirection};

/// Blink's `GridSpan::GridSpanType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GridSpanType {
  Indefinite,
  TranslatedDefinite,
  UntranslatedDefinite,
}

/// Blink's `GridSpan`: the lines an item spans, or how many before it is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GridSpan {
  start_line: i32,
  end_line: i32,
  kind: GridSpanType,
}

const MAX: i32 = GRID_MAX_TRACKS as i32;

impl GridSpan {
  fn new(start_line: i32, end_line: i32, kind: GridSpanType) -> Self {
    if kind == GridSpanType::Indefinite {
      return Self {
        start_line: 0,
        end_line: end_line.clamp(1, MAX),
        kind,
      };
    }

    let start_line = start_line.clamp(-MAX, MAX - 1);

    Self {
      start_line,
      end_line: end_line.clamp(start_line + 1, MAX),
      kind,
    }
  }

  pub(crate) fn untranslated_definite(start_line: i32, end_line: i32) -> Self {
    Self::new(start_line, end_line, GridSpanType::UntranslatedDefinite)
  }

  pub(crate) fn translated_definite(start_line: usize, end_line: usize) -> Self {
    Self::new(
      start_line as i32,
      end_line as i32,
      GridSpanType::TranslatedDefinite,
    )
  }

  pub(crate) fn indefinite(span_size: usize) -> Self {
    Self::new(0, span_size as i32, GridSpanType::Indefinite)
  }

  pub(crate) fn integer_span(&self) -> usize {
    self.span_size()
  }

  pub(crate) fn indefinite_span_size(&self) -> usize {
    self.end_line as usize
  }

  pub(crate) fn span_size(&self) -> usize {
    (self.end_line - self.start_line) as usize
  }

  pub(crate) fn untranslated_start_line(&self) -> i32 {
    self.start_line
  }

  pub(crate) fn untranslated_end_line(&self) -> i32 {
    self.end_line
  }

  pub(crate) fn start_line(&self) -> usize {
    self.start_line as usize
  }

  pub(crate) fn end_line(&self) -> usize {
    self.end_line as usize
  }

  pub(crate) fn is_untranslated_definite(&self) -> bool {
    self.kind == GridSpanType::UntranslatedDefinite
  }

  pub(crate) fn is_indefinite(&self) -> bool {
    self.kind == GridSpanType::Indefinite
  }

  pub(crate) fn translate(&mut self, offset: usize) {
    *self = Self::new(
      self.start_line + offset as i32,
      self.end_line + offset as i32,
      GridSpanType::TranslatedDefinite,
    );
  }
}

/// Blink's `GridArea`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GridArea {
  pub(crate) columns: GridSpan,
  pub(crate) rows: GridSpan,
}

impl Default for GridArea {
  fn default() -> Self {
    Self {
      columns: GridSpan::indefinite(1),
      rows: GridSpan::indefinite(1),
    }
  }
}

impl GridArea {
  pub(crate) fn span(&self, direction: GridTrackSizingDirection) -> &GridSpan {
    match direction {
      GridTrackSizingDirection::ForColumns => &self.columns,
      GridTrackSizingDirection::ForRows => &self.rows,
    }
  }

  pub(crate) fn span_mut(&mut self, direction: GridTrackSizingDirection) -> &mut GridSpan {
    match direction {
      GridTrackSizingDirection::ForColumns => &mut self.columns,
      GridTrackSizingDirection::ForRows => &mut self.rows,
    }
  }

  pub(crate) fn set_span(&mut self, span: GridSpan, direction: GridTrackSizingDirection) {
    *self.span_mut(direction) = span;
  }

  pub(crate) fn start_line(&self, direction: GridTrackSizingDirection) -> usize {
    self.span(direction).start_line()
  }

  pub(crate) fn end_line(&self, direction: GridTrackSizingDirection) -> usize {
    self.span(direction).end_line()
  }

  pub(crate) fn span_size(&self, direction: GridTrackSizingDirection) -> usize {
    self.span(direction).integer_span()
  }
}
