//! Blink's [`LineFlexer`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/flex/line_flexer.cc):
//! the flexible lengths of one line.
//! <https://drafts.csswg.org/css-flexbox/#resolve-flexible-lengths>

use crate::{
  layout::flex::flex_item::{FlexItem, FlexerState},
  layout_unit::LayoutUnit,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlexerMode {
  Grow,
  Shrink,
}

/// Blink's `LineFlexer`: resolves the main-axis content sizes of a line's items from their
/// `flex-grow` and `flex-shrink`.
pub(crate) struct LineFlexer<'a> {
  line_items: &'a mut [FlexItem],
  main_axis_inner_size: LayoutUnit,
  gap_between_items: LayoutUnit,
  mode: FlexerMode,
  total_flex_factor: f64,
  initial_free_space: LayoutUnit,
  free_space: LayoutUnit,
}

impl<'a> LineFlexer<'a> {
  pub(crate) fn new(
    line_items: &'a mut [FlexItem],
    main_axis_inner_size: LayoutUnit,
    sum_hypothetical_main_size: LayoutUnit,
    gap_between_items: LayoutUnit,
  ) -> Self {
    let mode = if sum_hypothetical_main_size < main_axis_inner_size {
      FlexerMode::Grow
    } else {
      FlexerMode::Shrink
    };
    let mut flexer = Self {
      line_items,
      main_axis_inner_size,
      gap_between_items,
      mode,
      total_flex_factor: 0.0,
      initial_free_space: LayoutUnit::ZERO,
      free_space: LayoutUnit::ZERO,
    };

    // Freeze any item with a flex factor of zero, or a min or max size violation.
    flexer.freeze_items(|item| {
      let flex_factor = match mode {
        FlexerMode::Grow => item.flex_grow,
        FlexerMode::Shrink => item.flex_shrink,
      };

      if flex_factor == 0.0 {
        return true;
      }

      match mode {
        FlexerMode::Grow => item.base_content_size > item.hypothetical_content_size,
        FlexerMode::Shrink => item.base_content_size < item.hypothetical_content_size,
      }
    });
    flexer.initial_free_space = flexer.free_space;
    flexer
  }

  pub(crate) fn run(mut self) {
    while self.resolve_flexible_lengths() {}
  }

  /// Blink's `FreezeItems`: freezes the items `should_freeze` picks, and sums the flex factors
  /// and free space again. The sums restart rather than subtract, as floating point does not
  /// associate.
  fn freeze_items(&mut self, should_freeze: impl Fn(&FlexItem) -> bool) {
    let mut total_weighted_flex_shrink = 0.0;

    self.total_flex_factor = 0.0;
    self.free_space = self.main_axis_inner_size;

    for item in self.line_items.iter_mut() {
      item.state = if item.state == FlexerState::Frozen || should_freeze(item) {
        FlexerState::Frozen
      } else {
        FlexerState::None
      };

      if item.state == FlexerState::Frozen {
        self.free_space -= item.flexed_margin_box_size() + self.gap_between_items;
        continue;
      }

      item.flexed_content_size = item.hypothetical_content_size;

      let flex_factor = f64::from(match self.mode {
        FlexerMode::Grow => item.flex_grow,
        FlexerMode::Shrink => item.flex_shrink,
      });

      self.total_flex_factor += flex_factor;
      item.free_space_fraction = match self.mode {
        FlexerMode::Grow => flex_factor / self.total_flex_factor,
        FlexerMode::Shrink => {
          let weighted_flex_shrink = flex_factor * item.base_content_size.to_double();

          total_weighted_flex_shrink += weighted_flex_shrink;
          weighted_flex_shrink / total_weighted_flex_shrink
        }
      };
      self.free_space -= item.flex_base_margin_box_size() + self.gap_between_items;
    }

    self.free_space += self.gap_between_items;
  }

  /// Blink's `ResolveFlexibleLengths`: distributes the free space once, and whether a violation
  /// froze items to distribute again.
  fn resolve_flexible_lengths(&mut self) -> bool {
    // Flex factors summing below one distribute only that fraction of the free space.
    if self.total_flex_factor > 0.0 && self.total_flex_factor < 1.0 {
      let fractional =
        LayoutUnit::from_double(self.initial_free_space.to_double() * self.total_flex_factor);

      if fractional.abs() < self.free_space.abs() {
        self.free_space = fractional;
      }
    }

    match self.mode {
      FlexerMode::Grow if self.free_space <= LayoutUnit::ZERO => return false,
      FlexerMode::Shrink if self.free_space >= LayoutUnit::ZERO => return false,
      _ => {}
    }

    // Each item takes its fraction of what the items after it leave, which converges without
    // rounding error.
    let mut total_violation = LayoutUnit::ZERO;

    for item in self.line_items.iter_mut().rev() {
      if item.state == FlexerState::Frozen {
        continue;
      }

      let extra_size = if item.free_space_fraction == 1.0 {
        self.free_space
      } else {
        let extra = self.free_space.to_double() * item.free_space_fraction;

        if extra.is_finite() {
          LayoutUnit::from_double_round(extra)
        } else {
          LayoutUnit::ZERO
        }
      };

      self.free_space -= extra_size;

      let item_size = item.base_content_size + extra_size;
      let adjusted_item_size = item
        .main_axis_min_max_sizes
        .clamp_size_to_min_and_max(item_size);

      item.flexed_content_size = adjusted_item_size;

      let violation = adjusted_item_size - item_size;

      if violation != LayoutUnit::ZERO {
        item.state = if violation < LayoutUnit::ZERO {
          FlexerState::MaxViolation
        } else {
          FlexerState::MinViolation
        };
      }
      total_violation += violation;
    }

    if total_violation == LayoutUnit::ZERO {
      return false;
    }

    let state = if total_violation < LayoutUnit::ZERO {
      FlexerState::MaxViolation
    } else {
      FlexerState::MinViolation
    };

    self.freeze_items(|item| item.state == state);
    true
  }
}
