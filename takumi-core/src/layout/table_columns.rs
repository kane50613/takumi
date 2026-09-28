//! Table column widths, after Blink's
//! [`table_layout_utils.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_layout_utils.cc)
//! and `table_layout_algorithm_types.cc`: each column's constraints from its cells, then the
//! table's width shared out between them. Follows Blink under the notice in LICENSE-CHROMIUM.
//!
//! Naive next to Blink: sizes are floats instead of `LayoutUnit`s, and `<col>` elements and
//! captions constrain nothing.

/// What one cell asks of the columns it spans, Blink's `CellInlineConstraint`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CellConstraint {
  pub(crate) min: f32,
  pub(crate) max: f32,
  pub(crate) percent: Option<f32>,
  /// Whether the cell has a fixed `width`.
  pub(crate) constrained: bool,
}

/// A cell that spans several columns.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ColspanCell {
  pub(crate) constraint: CellConstraint,
  pub(crate) start: usize,
  pub(crate) span: usize,
}

impl CellConstraint {
  /// Merges another cell of the same column in, as Blink's `CellInlineConstraint::Encompass`.
  pub(crate) fn encompass(&mut self, other: Self) {
    self.min = self.min.max(other.min);
    self.max = match (self.constrained, other.constrained) {
      (true, false) => self.max.max(other.min),
      (false, true) => self.min.max(other.max),
      _ => self.max.max(other.max),
    };
    self.constrained |= other.constrained;
    if other.percent > self.percent {
      self.percent = other.percent;
    }
  }
}

/// One column's constraints, Blink's `TableTypes::Column`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Column {
  min: Option<f32>,
  max: Option<f32>,
  percent: Option<f32>,
  constrained: bool,
  /// Whether no cell starts here, so the column takes no space of its own.
  mergeable: bool,
  table_fixed: bool,
}

impl Column {
  /// A column no cell has reached yet in a table laid out `fixed` or not.
  fn new(table_fixed: bool) -> Self {
    Self {
      min: None,
      max: None,
      percent: None,
      constrained: false,
      mergeable: !table_fixed,
      table_fixed,
    }
  }

  /// Merges the cells that span only this column in, as Blink's `Column::Encompass`.
  fn encompass(&mut self, cell: CellConstraint) {
    if self.constrained && self.table_fixed {
      return;
    }
    if !self.table_fixed {
      self.mergeable = false;
    }

    let (min, max) = match (self.min, self.max) {
      (Some(min), max) => {
        let max = max.unwrap_or_default();
        let max = match (self.constrained, cell.constrained) {
          (true, true) => max.max(cell.max),
          (true, false) => max.max(cell.min),
          (false, _) => max.max(cell.max),
        };

        (min.max(cell.min), max)
      }
      (None, _) => (cell.min, cell.max),
    };

    self.min = Some(min);
    self.max = Some(max.max(min));
    if cell.percent > self.percent {
      self.percent = cell.percent;
    }
    self.constrained |= cell.constrained;
  }

  fn min(&self) -> f32 {
    self.min.unwrap_or_default()
  }

  fn max(&self) -> f32 {
    self.max.unwrap_or_default()
  }

  /// Blink's `Column::ResolvePercentInlineSize`.
  fn percent_size(&self, basis: f32) -> f32 {
    self
      .min()
      .max(self.percent.unwrap_or_default() * basis / 100.0)
  }

  /// Blink's `Column::IsFixed`.
  fn is_fixed(&self) -> bool {
    self.constrained && self.percent.is_none() && self.max.is_some()
  }
}

/// A table's columns, ready to share out its width.
#[derive(Clone, Debug)]
pub(crate) struct TableColumns {
  columns: Vec<Column>,
  fixed: bool,
  /// The horizontal `border-spacing`.
  spacing: f32,
}

impl TableColumns {
  /// The columns `cells` and `colspan_cells` constrain, with `spacing` between them, as Blink's
  /// `ApplyCellConstraintsToColumnConstraints` builds them.
  pub(crate) fn new(
    cells: &[Option<CellConstraint>],
    mut colspan_cells: Vec<ColspanCell>,
    spacing: f32,
    fixed: bool,
  ) -> Self {
    let mut columns = vec![Column::new(fixed); cells.len()];

    for colspan in &colspan_cells {
      // At least one column a spanning cell covers takes space, or the span would have none.
      if let Some(column) = columns.get_mut(colspan.start) {
        column.mergeable = false;
      }
    }
    for (column, cell) in columns.iter_mut().zip(cells) {
      if let Some(cell) = cell {
        column.encompass(*cell);
      }
    }

    colspan_cells.sort_by_key(|cell| (cell.span, cell.start));

    let mut table = Self {
      columns,
      fixed,
      spacing,
    };

    for colspan in &colspan_cells {
      if fixed {
        table.distribute_colspan_fixed(colspan);
      } else {
        table.distribute_colspan_auto(colspan);
      }
    }

    let mut total_percent = 0.0;

    for column in &mut table.columns {
      if let Some(percent) = column.percent.as_mut() {
        // An auto table leaves each column no more than what earlier columns left of 100%.
        if !fixed && *percent + total_percent > 100.0 {
          *percent = 100.0 - total_percent;
        }
        total_percent += *percent;
      }
      column.min = Some(column.min());
      column.max = Some(column.max());
    }
    if fixed && total_percent > 100.0 {
      for percent in table
        .columns
        .iter_mut()
        .filter_map(|column| column.percent.as_mut())
      {
        *percent = *percent * 100.0 / total_percent;
      }
    }

    table
  }

  /// The border spacing inside `columns`, between the ones that take space.
  fn inner_spacing(&self, columns: &[Column]) -> f32 {
    let count = columns.iter().filter(|column| !column.mergeable).count();

    self.spacing * count.saturating_sub(1) as f32
  }

  /// Blink's `DistributeColspanCellToColumnsAuto`.
  fn distribute_colspan_auto(&mut self, colspan: &ColspanCell) {
    let end = (colspan.start + colspan.span).min(self.columns.len());

    if colspan.start >= end {
      return;
    }

    let inner = self.inner_spacing(&self.columns[colspan.start..end]);
    let span = &mut self.columns[colspan.start..end];
    let min = (colspan.constraint.min - inner).max(0.0);
    let max = (colspan.constraint.max - inner).max(0.0);

    for column in span.iter_mut() {
      column.min.get_or_insert(0.0);
      column.max.get_or_insert(0.0);
    }

    if let Some(cell_percent) = colspan.constraint.percent {
      let takes_space = || span.iter().filter(|column| !column.mergeable);
      let columns_percent: f32 = takes_space().filter_map(|column| column.percent).sum();
      let others = takes_space().filter(|column| column.percent.is_none());
      let other_count = others.clone().count();
      let other_max: f32 = others.map(Column::max).sum();
      let surplus = cell_percent - columns_percent;

      if surplus > 0.0 && other_count > 0 {
        for column in span
          .iter_mut()
          .filter(|column| column.percent.is_none() && !column.mergeable)
        {
          column.percent = Some(if other_max != 0.0 {
            surplus * column.max() / other_max
          } else {
            surplus / other_count as f32
          });
        }
      }
    }

    let sizes = distribute_auto(min, span, true);

    for (column, size) in span.iter_mut().zip(sizes) {
      column.min = Some(column.min().max(size));
    }

    let sizes = distribute_auto(max, span, colspan.constraint.constrained);

    for (column, size) in span.iter_mut().zip(sizes) {
      column.max = Some(column.min().max(column.max()).max(size));
    }
  }

  /// Blink's `DistributeColspanCellToColumnsFixed`.
  fn distribute_colspan_fixed(&mut self, colspan: &ColspanCell) {
    let end = (colspan.start + colspan.span).min(self.columns.len());

    if colspan.start >= end {
      return;
    }

    let inner = self.inner_spacing(&self.columns[colspan.start..end]);
    let span = &mut self.columns[colspan.start..end];
    let count = span
      .iter()
      .filter(|column| !column.mergeable)
      .count()
      .max(1) as f32;
    let constraint = colspan.constraint;
    let min = if constraint.constrained {
      (constraint.min - inner).max(0.0)
    } else {
      0.0
    };
    let max = (constraint.max - inner).max(0.0);
    let (share_min, share_max) = (min / count, max / count);
    let share_percent = constraint.percent.map(|percent| percent / count);
    let mut last = None;

    for (index, column) in span.iter_mut().enumerate() {
      if column.mergeable {
        continue;
      }
      last = Some(index);
      if column.min.is_none() {
        column.constrained |= constraint.constrained;
        column.min = Some(share_min);
      }
      if column.max.is_none() {
        column.constrained |= constraint.constrained;
        column.max = Some(share_max);
      }
      if column.percent.is_none() && !column.constrained {
        column.percent = share_percent;
      }
    }

    if let Some(last) = last.and_then(|last| span.get_mut(last)) {
      last.min = Some(last.min() + min - share_min * count);
      last.max = Some(last.max() + max - share_max * count);
    }
  }

  /// The space the columns never share: `edges`, the table's border and padding with the border
  /// spacing at its edges, and the spacing between columns, as Blink's
  /// `ComputeUndistributableTableSpace` counts it.
  pub(crate) fn undistributable(&self, edges: f32) -> f32 {
    let count = self
      .columns
      .iter()
      .filter(|column| !column.mergeable)
      .count();

    edges + self.spacing * count.saturating_sub(1) as f32
  }

  /// The border spacing at the table's two edges.
  pub(crate) fn edge_spacing(&self) -> f32 {
    2.0 * self.spacing
  }

  /// The table's min- and max-content widths around `undistributable` space, Blink's
  /// `ComputeGridInlineMinMax`.
  pub(crate) fn min_max(&self, undistributable: f32) -> (f32, f32) {
    let mut min = 0.0;
    let mut max = 0.0;
    let mut percent_estimate = 0.0_f32;
    let mut non_percent_max = 0.0;
    let mut percent_sum = 0.0;

    for column in &self.columns {
      min += if self.fixed && column.is_fixed() {
        column.max()
      } else {
        column.min()
      };
      match column.percent.filter(|percent| *percent > 0.0) {
        Some(percent) if column.max() > 0.0 => {
          percent_estimate = percent_estimate.max(100.0 / percent * column.max());
        }
        Some(_) => {}
        None => non_percent_max += column.max(),
      }
      max += column.max();
      percent_sum += column.percent.unwrap_or_default();
    }

    let percent_sum = percent_sum.min(100.0);

    if percent_sum > 0.0 {
      let from_percent = match non_percent_max {
        0.0 => 0.0,
        _ if percent_sum == 100.0 => TABLE_MAX_INLINE_SIZE,
        _ => 100.0 / (100.0 - percent_sum) * non_percent_max,
      };

      max = max.max(from_percent).max(percent_estimate);
    }

    (min + undistributable, min.max(max) + undistributable)
  }

  /// How many grid tracks the columns take: a column no cell starts in merges away, as Blink gives
  /// it neither size nor border spacing.
  pub(crate) fn tracks(&self) -> usize {
    self.track(self.columns.len()).max(1)
  }

  /// The grid track `column` starts on, past the columns that merged away before it.
  pub(crate) fn track(&self, column: usize) -> usize {
    self.columns[..column.min(self.columns.len())]
      .iter()
      .filter(|column| !column.mergeable)
      .count()
  }

  /// How many grid tracks the `span` columns from `start` cover.
  pub(crate) fn track_span(&self, start: usize, span: usize) -> usize {
    (self.track(start + span) - self.track(start)).max(1)
  }

  /// Each grid track's width once `assignable` is shared out.
  pub(crate) fn track_widths(&self, assignable: f32) -> Vec<f32> {
    self
      .widths(assignable)
      .into_iter()
      .zip(&self.columns)
      .filter(|(_, column)| !column.mergeable)
      .map(|(width, _)| width)
      .collect()
  }

  /// Each column's width once `assignable` is shared out, Blink's
  /// `SynchronizeAssignableTableInlineSizeAndColumns`.
  pub(crate) fn widths(&self, assignable: f32) -> Vec<f32> {
    if self.fixed {
      self.widths_fixed(assignable)
    } else {
      distribute_auto(assignable, &self.columns, true)
    }
  }

  /// Blink's `SynchronizeAssignableTableInlineSizeAndColumnsFixed`.
  fn widths_fixed(&self, target: f32) -> Vec<f32> {
    // Columns of width 0 are treated as auto by all browsers.
    let treat_as_fixed = |column: &Column| column.is_fixed() && column.max() != 0.0;
    let zero_constrained = |column: &Column| column.constrained && column.max() == 0.0;
    let mut sizes = vec![0.0; self.columns.len()];
    let mut percent_count = 0;
    let mut auto_count = 0;
    let mut fixed_count = 0;
    let mut zero_count = 0;
    let mut total_percent = 0.0;
    let mut total_fixed = 0.0;
    let mut assigned = 0.0;
    let mut last = None;

    for column in &self.columns {
      if column.percent.is_some() {
        percent_count += 1;
        total_percent += column.percent_size(target);
      } else if treat_as_fixed(column) {
        fixed_count += 1;
        total_fixed += column.max();
      } else if zero_constrained(column) {
        zero_count += 1;
      } else {
        auto_count += 1;
      }
    }

    if fixed_count > 0 {
      let fixed_target = (target - total_percent).max(0.0);
      let scales = (total_fixed < fixed_target && auto_count == 0) || total_fixed > target;
      let scale = if scales && total_fixed != 0.0 {
        Some(fixed_target / total_fixed)
      } else if scales {
        None
      } else {
        Some(1.0)
      };

      for (index, column) in self.columns.iter().enumerate() {
        if !treat_as_fixed(column) {
          continue;
        }
        last = Some(index);
        sizes[index] = scale.map_or(target / fixed_count as f32, |scale| scale * column.max());
        assigned += sizes[index];
      }
    }
    if assigned >= target {
      return sizes;
    }
    if percent_count > 0 {
      let room = target - assigned;
      let scales = (total_percent < room && auto_count == 0) || total_percent > room;
      let scale = if scales && total_percent != 0.0 {
        Some(room / total_percent)
      } else if scales {
        None
      } else {
        Some(1.0)
      };

      for (index, column) in self.columns.iter().enumerate() {
        if column.percent.is_none() {
          continue;
        }
        last = Some(index);
        sizes[index] = scale.map_or(room / percent_count as f32, |scale| {
          scale * column.percent_size(target)
        });
        assigned += sizes[index];
      }
    }

    let distributing = target - assigned;
    let zero_only = zero_count == self.columns.len();

    for (index, column) in self.columns.iter().enumerate() {
      if column.percent.is_some() || treat_as_fixed(column) {
        continue;
      }
      if zero_constrained(column) && !zero_only {
        continue;
      }
      last = Some(index);
      sizes[index] = distributing / if zero_only { zero_count } else { auto_count } as f32;
      assigned += sizes[index];
    }

    if let Some(last) = last {
      sizes[last] += target - assigned;
    }
    sizes
  }
}

/// Blink's `TableTypes::kTableMaxInlineSize`.
const TABLE_MAX_INLINE_SIZE: f32 = 1_000_000.0;

/// The guesses of css-tables-3's width distribution, in the order they grow.
/// <https://www.w3.org/TR/css-tables-3/#width-distribution-algorithm>
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Guess {
  Min,
  Percentage,
  Specified,
  Max,
  AboveMax,
}

/// `target` shared out over `columns`, Blink's `DistributeInlineSizeToComputedInlineSizeAuto`.
/// A `constrained` target may grow fixed columns past their widths.
fn distribute_auto(target: f32, columns: &[Column], constrained: bool) -> Vec<f32> {
  let mut guesses = [0.0_f32; 4];
  let mut increases = [0.0_f32; 4];
  let mut counts = (0, 0, 0);
  let mut total_percent = 0.0;
  let mut total_auto_max = 0.0;
  let mut total_fixed_max = 0.0;

  for column in columns.iter().filter(|column| !column.mergeable) {
    let (min, max) = (column.min(), column.max());

    if column.percent.is_some() {
      let percent = column.percent_size(target);

      counts.0 += 1;
      total_percent += column.percent.unwrap_or_default();
      guesses = [
        guesses[0] + min,
        guesses[1] + percent,
        guesses[2] + percent,
        guesses[3] + percent,
      ];
      increases[1] += percent - min;
    } else if column.constrained {
      counts.1 += 1;
      total_fixed_max += max;
      guesses = [
        guesses[0] + min,
        guesses[1] + min,
        guesses[2] + max,
        guesses[3] + max,
      ];
      increases[2] += max - min;
    } else {
      counts.2 += 1;
      total_auto_max += max;
      guesses = [
        guesses[0] + min,
        guesses[1] + min,
        guesses[2] + min,
        guesses[3] + max,
      ];
      increases[3] += max - min;
    }
  }

  let (percent_count, fixed_count, auto_count) = counts;
  // Distributing never takes a column below its minimum.
  let target = target.max(guesses[0]);
  let guess = [Guess::Min, Guess::Percentage, Guess::Specified, Guess::Max]
    .into_iter()
    .zip(guesses)
    .find(|(_, size)| *size >= target)
    .map_or(Guess::AboveMax, |(guess, _)| guess);
  let mut sizes = vec![0.0; columns.len()];
  let mut deficit = 0.0;
  let mut last = None;
  // Each column the pass grows takes `delta` of `distributable` by its share of `increase`, or an
  // even share of `count` without one.
  let share = |distributable: f32, weight: f32, total: f32, count: usize| {
    if total > 0.0 {
      distributable * weight / total
    } else {
      distributable / count as f32
    }
  };

  match guess {
    Guess::Min => {
      for (size, column) in sizes.iter_mut().zip(columns) {
        if !column.mergeable {
          *size = column.min();
        }
      }
    }
    Guess::Percentage => {
      let distributable = target - guesses[0];

      deficit = distributable;
      for (index, column) in columns.iter().enumerate() {
        if column.mergeable {
          continue;
        }
        sizes[index] = if column.percent.is_some() {
          let delta = share(
            distributable,
            column.percent_size(target) - column.min(),
            increases[1],
            percent_count,
          );

          last = Some(index);
          deficit -= delta;
          column.min() + delta
        } else {
          column.min()
        };
      }
    }
    Guess::Specified => {
      let distributable = target - guesses[1];

      deficit = distributable;
      for (index, column) in columns.iter().enumerate() {
        if column.mergeable {
          continue;
        }
        sizes[index] = if column.percent.is_some() {
          column.percent_size(target)
        } else if column.constrained {
          let delta = share(
            distributable,
            column.max() - column.min(),
            increases[2],
            fixed_count,
          );

          last = Some(index);
          deficit -= delta;
          column.min() + delta
        } else {
          column.min()
        };
      }
    }
    Guess::Max => {
      let distributable = target - guesses[2];
      // An exact match usually means an auto table sized to fit its content without wrapping.
      let exact = target == guesses[3];

      deficit = if exact { 0.0 } else { distributable };
      for (index, column) in columns.iter().enumerate() {
        if column.mergeable {
          continue;
        }
        sizes[index] = if column.percent.is_some() {
          column.percent_size(target)
        } else if column.constrained || exact {
          column.max()
        } else {
          let delta = share(
            distributable,
            column.max() - column.min(),
            increases[3],
            auto_count,
          );

          last = Some(index);
          deficit -= delta;
          column.min() + delta
        };
      }
    }
    Guess::AboveMax => {
      let distributable = target - guesses[3];

      deficit = distributable;
      if auto_count > 0 {
        for (index, column) in columns.iter().enumerate() {
          if column.mergeable {
            continue;
          }
          sizes[index] = if column.percent.is_some() {
            column.percent_size(target)
          } else if column.constrained {
            column.max()
          } else {
            let delta = share(distributable, column.max(), total_auto_max, auto_count);

            last = Some(index);
            deficit -= delta;
            column.max() + delta
          };
        }
      } else if fixed_count > 0 && constrained {
        for (index, column) in columns.iter().enumerate() {
          if column.mergeable {
            continue;
          }
          sizes[index] = if column.percent.is_some() {
            column.percent_size(target)
          } else {
            let delta = share(distributable, column.max(), total_fixed_max, fixed_count);

            last = Some(index);
            deficit -= delta;
            column.max() + delta
          };
        }
      } else if percent_count > 0 {
        for (index, column) in columns.iter().enumerate() {
          let Some(percent) = column.percent.filter(|_| !column.mergeable) else {
            continue;
          };
          let delta = share(distributable, percent, total_percent, percent_count);

          last = Some(index);
          deficit -= delta;
          sizes[index] = column.percent_size(target) + delta;
        }
      } else {
        deficit = 0.0;
      }
    }
  }

  if let Some(last) = last {
    sizes[last] += deficit;
  }
  sizes
}

#[cfg(test)]
mod tests {
  use super::*;

  fn cell(min: f32, max: f32) -> Option<CellConstraint> {
    Some(CellConstraint {
      min,
      max,
      percent: None,
      constrained: false,
    })
  }

  #[test]
  fn a_narrow_table_shares_what_is_past_the_minimums_by_the_room_each_column_wants() {
    let table = TableColumns::new(
      &[cell(10.0, 50.0), cell(20.0, 120.0)],
      Vec::new(),
      0.0,
      false,
    );

    // 30 past the minimums of 30, over wants of 40 and 100.
    assert_eq!(
      table.widths(60.0),
      vec![10.0 + 30.0 * 40.0 / 140.0, 20.0 + 30.0 * 100.0 / 140.0]
    );
  }

  #[test]
  fn a_wide_table_grows_auto_columns_by_their_maximums() {
    let table = TableColumns::new(
      &[cell(10.0, 50.0), cell(20.0, 150.0)],
      Vec::new(),
      0.0,
      false,
    );

    assert_eq!(
      table.widths(400.0),
      vec![50.0 + 200.0 * 0.25, 150.0 + 200.0 * 0.75]
    );
  }

  #[test]
  fn a_percentage_column_takes_its_share_first() {
    let percent = Some(CellConstraint {
      min: 10.0,
      max: 10.0,
      percent: Some(30.0),
      constrained: false,
    });
    let table = TableColumns::new(&[percent, cell(10.0, 40.0)], Vec::new(), 0.0, false);

    assert_eq!(table.widths(200.0), vec![60.0, 140.0]);
  }

  #[test]
  fn the_grid_widths_count_the_border_spacing() {
    let table = TableColumns::new(
      &[cell(10.0, 50.0), cell(20.0, 150.0)],
      Vec::new(),
      2.0,
      false,
    );
    let undistributable = table.undistributable(4.0);

    assert_eq!(undistributable, 6.0);
    assert_eq!(table.min_max(undistributable), (36.0, 206.0));
  }

  #[test]
  fn columns_only_a_span_covers_merge_into_one_track() {
    let span = ColspanCell {
      constraint: CellConstraint {
        min: 30.0,
        max: 90.0,
        percent: None,
        constrained: false,
      },
      start: 0,
      span: 3,
    };
    let table = TableColumns::new(&[None, None, None], vec![span], 2.0, false);

    assert_eq!(table.tracks(), 1);
    assert_eq!(table.track_span(0, 3), 1);
    assert_eq!(table.undistributable(4.0), 4.0);
    assert_eq!(table.track_widths(90.0), [90.0]);
  }

  #[test]
  fn a_fixed_table_shares_the_rest_evenly_after_a_fixed_column() {
    let fixed = Some(CellConstraint {
      min: 0.0,
      max: 120.0,
      percent: None,
      constrained: true,
    });
    let table = TableColumns::new(
      &[fixed, cell(0.0, 30.0), cell(0.0, 900.0)],
      Vec::new(),
      0.0,
      true,
    );

    assert_eq!(table.widths(420.0), vec![120.0, 150.0, 150.0]);
  }
}
