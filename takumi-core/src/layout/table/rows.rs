//! Table rows and sections: their block sizes, the extra block size a table, section, or
//! rowspanned cell hands its rows, and the baselines rows align their cells to. Follows Blink's
//! [`table_layout_utils.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_layout_utils.cc)
//! under the notice in LICENSE-CHROMIUM.

use std::cmp::Ordering;

use crate::{layout::content_alignment::BlockContentAlignment, layout_unit::LayoutUnit};

/// The rows, sections, and cells of one table layout, Blink's `TableTypes::Rows`, `Sections`, and
/// `CellBlockConstraints`, with each cell's measure.
#[derive(Default)]
pub(super) struct Rows {
  pub(super) rows: Vec<Row>,
  pub(super) sections: Vec<Section>,
  pub(super) cells: Vec<CellBlockConstraint>,
  pub(super) measures: Vec<CellMeasure>,
}

/// One cell's measure-pass size and baseline.
#[derive(Clone, Copy)]
pub(super) struct CellMeasure {
  pub(super) block_size: LayoutUnit,
  pub(super) first_baseline: Option<LayoutUnit>,
}

/// What one cell asks of its rows, Blink's `TableTypes::CellBlockConstraint`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CellBlockConstraint {
  pub(crate) min_block_size: LayoutUnit,
  pub(crate) effective_rowspan: usize,
  /// Whether the cell has a fixed `height`.
  pub(crate) is_constrained: bool,
}

/// A cell spanning several rows, Blink's `TableTypes::RowspanCell`.
#[derive(Clone, Copy, Debug)]
pub(super) struct RowspanCell {
  pub(super) start_row: usize,
  pub(super) effective_rowspan: usize,
  pub(super) min_block_size: LayoutUnit,
}

impl RowspanCell {
  /// Blink's `RowspanCell::operator<`: a span another encloses distributes first, and of two
  /// equal spans the taller one.
  pub(super) fn distribution_order(&self, other: &Self) -> Ordering {
    let is_enclosed = |inner: &Self, outer: &Self| {
      inner.start_row >= outer.start_row
        && inner.start_row + inner.effective_rowspan <= outer.start_row + outer.effective_rowspan
    };

    if self.start_row == other.start_row && self.effective_rowspan == other.effective_rowspan {
      other.min_block_size.cmp(&self.min_block_size)
    } else if is_enclosed(self, other) {
      Ordering::Less
    } else if is_enclosed(other, self) {
      Ordering::Greater
    } else {
      self.start_row.cmp(&other.start_row)
    }
  }
}

/// One row, Blink's `TableTypes::Row`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Row {
  pub(crate) block_size: LayoutUnit,
  pub(crate) start_cell_index: usize,
  pub(crate) baseline: Option<LayoutUnit>,
  /// 100% stored as 100.
  pub(crate) percent: Option<f32>,
  pub(crate) is_constrained: bool,
  /// Whether a cell spanning several rows starts here.
  pub(crate) has_rowspan_start: bool,
  pub(crate) is_collapsed: bool,
}

/// One row group, Blink's `TableTypes::Section`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Section {
  pub(crate) start_row: usize,
  pub(crate) row_count: usize,
  pub(crate) block_size: LayoutUnit,
  pub(crate) percent: Option<f32>,
  pub(crate) is_constrained: bool,
  pub(crate) is_tbody: bool,
  pub(super) needs_redistribution: bool,
}

/// Blink's `ComputeCellBlockSize`: the block size the rows a cell spans give it, and whether that
/// size is indefinite for percentages inside it.
pub(super) fn cell_block_size(
  constraint: &CellBlockConstraint,
  rows: &[Row],
  row_index: usize,
  block_border_spacing: LayoutUnit,
  is_table_block_size_specified: bool,
) -> (LayoutUnit, bool) {
  let mut block_size = LayoutUnit::ZERO;

  // A cell spanning from a collapsed row has no block size, however far it spans.
  if !rows[row_index].is_collapsed {
    for (offset, row) in rows[row_index..]
      .iter()
      .take(constraint.effective_rowspan)
      .enumerate()
    {
      if row.is_collapsed {
        continue;
      }
      block_size += row.block_size;
      if offset != 0 {
        block_size += block_border_spacing;
      }
    }
  }

  let has_grown = block_size > constraint.min_block_size;
  let is_initial_block_size_definite =
    constraint.is_constrained || (has_grown && is_table_block_size_specified);

  (block_size, !is_initial_block_size_definite)
}

/// Blink's `DistributeExcessBlockSizeToRows`: `desired_block_size` across `row_count` rows from
/// `start_row`, grown first in percentage rows, then rows starting a rowspan (only when
/// `is_rowspan_distribution`), then unconstrained rows with content, then empty rows, then all.
fn distribute_excess_block_size_to_rows(
  start_row: usize,
  row_count: usize,
  desired_block_size: LayoutUnit,
  is_rowspan_distribution: bool,
  block_border_spacing: LayoutUnit,
  percentage_resolution_block_size: Option<LayoutUnit>,
  rows: &mut [Row],
) {
  if row_count == 0 {
    return;
  }
  let end_row = start_row + row_count;
  let deficit = |row: &Row, basis: LayoutUnit| {
    (LayoutUnit::from_float(row.percent.unwrap_or_default() * basis.to_float() / 100.0)
      - row.block_size)
      .clamp_negative_to_zero()
  };
  let mut rows_with_originating_rowspan = Vec::new();
  let mut percent_rows_with_deficit = Vec::new();
  let mut unconstrained_non_empty_rows = Vec::new();
  let mut empty_rows = Vec::new();
  let mut non_empty_rows = Vec::new();
  let mut unconstrained_empty_rows = Vec::new();
  let mut constrained_non_empty_row_count = 0;
  let mut total_block_size = LayoutUnit::ZERO;
  let mut percent_block_size_deficit = LayoutUnit::ZERO;
  let mut unconstrained_non_empty_row_block_size = LayoutUnit::ZERO;

  for (offset, row) in rows[start_row..end_row].iter().enumerate() {
    let index = start_row + offset;

    total_block_size += row.block_size;
    if is_rowspan_distribution && index != start_row && row.has_rowspan_start {
      rows_with_originating_rowspan.push(index);
    }

    let mut is_row_empty = row.block_size == LayoutUnit::ZERO;

    if let (Some(percent), Some(basis)) = (row.percent, percentage_resolution_block_size)
      && percent != 0.0
    {
      let row_deficit = deficit(row, basis);

      if row_deficit != LayoutUnit::ZERO {
        percent_rows_with_deficit.push(index);
        percent_block_size_deficit += row_deficit;
        is_row_empty = false;
      }
    }

    // Only percent rows that resolve count as constrained.
    let is_row_constrained =
      row.is_constrained && (row.percent.is_none() || percentage_resolution_block_size.is_some());

    if is_row_empty {
      empty_rows.push(index);
      if !is_row_constrained {
        unconstrained_empty_rows.push(index);
      }
    } else {
      non_empty_rows.push(index);
      if is_row_constrained {
        constrained_non_empty_row_count += 1;
      } else {
        unconstrained_non_empty_rows.push(index);
        unconstrained_non_empty_row_block_size += row.block_size;
      }
    }
  }

  let mut distributable =
    (desired_block_size - block_border_spacing * (row_count as i32 - 1)) - total_block_size;

  if distributable <= LayoutUnit::ZERO {
    return;
  }

  // Step 1: percentage rows grow to no more than their percentage size.
  if let (Some(&last), Some(basis)) = (
    percent_rows_with_deficit.last(),
    percentage_resolution_block_size,
  ) {
    let percent_distributable = percent_block_size_deficit.min(distributable);
    let mut remaining_deficit = percent_distributable;

    for &index in &percent_rows_with_deficit {
      let delta =
        percent_distributable.mul_div(deficit(&rows[index], basis), percent_block_size_deficit);

      rows[index].block_size += delta;
      distributable -= delta;
      remaining_deficit -= delta;
    }
    rows[last].block_size += remaining_deficit;
    distributable -= remaining_deficit;

    // Rounding may distribute more than there was.
    if distributable <= LayoutUnit::ZERO {
      return;
    }
  }

  // Step 2: rows a rowspan starts in.
  if let Some(&last) = rows_with_originating_rowspan.last() {
    let mut remaining_deficit = distributable;

    for &index in &rows_with_originating_rowspan {
      let delta = distributable / rows_with_originating_rowspan.len() as i32;

      rows[index].block_size += delta;
      remaining_deficit -= delta;
    }
    rows[last].block_size += remaining_deficit;
    rows[last].block_size = rows[last].block_size.max(LayoutUnit::ZERO);
    return;
  }

  // Step 3: unconstrained rows with content, in proportion to their block size.
  if let Some(&last) = unconstrained_non_empty_rows.last() {
    let mut remaining_deficit = distributable;

    for &index in &unconstrained_non_empty_rows {
      let delta = distributable.mul_div(
        rows[index].block_size,
        unconstrained_non_empty_row_block_size,
      );

      rows[index].block_size += delta;
      remaining_deficit -= delta;
    }
    rows[last].block_size += remaining_deficit;
    return;
  }

  // Step 4: empty rows, once every row is empty or constrained.
  if !empty_rows.is_empty() {
    let has_only_empty_rows = empty_rows.len() == row_count;

    if is_rowspan_distribution {
      // A rowspan over only empty rows hands everything to the last of them.
      if let Some(&last) = empty_rows.last().filter(|_| has_only_empty_rows) {
        rows[last].block_size += distributable;
        return;
      }
    } else if has_only_empty_rows || empty_rows.len() + constrained_non_empty_row_count == row_count
    {
      // Of constrained and unconstrained empty rows, only the unconstrained ones grow.
      let rows_to_grow = if unconstrained_empty_rows.is_empty() {
        &empty_rows
      } else {
        &unconstrained_empty_rows
      };
      let mut remaining_deficit = distributable;

      for &index in rows_to_grow {
        let delta = distributable / rows_to_grow.len() as i32;

        rows[index].block_size = delta;
        remaining_deficit -= delta;
      }
      if let Some(&last) = rows_to_grow.last() {
        rows[last].block_size += remaining_deficit;
      }
      return;
    }
  }

  // Step 5: every row with content, constrained or not, in proportion to its block size.
  if let Some(&last) = non_empty_rows.last() {
    let mut remaining_deficit = distributable;

    for &index in &non_empty_rows {
      let delta = distributable.mul_div(rows[index].block_size, total_block_size);

      rows[index].block_size += delta;
      remaining_deficit -= delta;
    }
    rows[last].block_size += remaining_deficit;
  }
}

/// Blink's `DistributeRowspanCellToRows`.
pub(super) fn distribute_rowspan_cell_to_rows(
  cell: &RowspanCell,
  block_border_spacing: LayoutUnit,
  rows: &mut [Row],
) {
  distribute_excess_block_size_to_rows(
    cell.start_row,
    cell.effective_rowspan,
    cell.min_block_size,
    true,
    block_border_spacing,
    None,
    rows,
  );
}

/// Blink's `DistributeSectionFixedBlockSizeToRows`.
pub(super) fn distribute_section_fixed_block_size_to_rows(
  start_row: usize,
  row_count: usize,
  section_fixed_block_size: LayoutUnit,
  block_border_spacing: LayoutUnit,
  rows: &mut [Row],
) {
  distribute_excess_block_size_to_rows(
    start_row,
    row_count,
    section_fixed_block_size,
    false,
    block_border_spacing,
    Some(section_fixed_block_size),
    rows,
  );
}

/// Blink's `DistributeTableBlockSizeToSections`: grows percent sections to their percentage, then
/// the body sections (or else every section) to fill `table_block_size`, and hands each grown
/// section's extra block size to its rows.
pub(super) fn distribute_table_block_size_to_sections(
  block_border_spacing: LayoutUnit,
  table_block_size: LayoutUnit,
  sections: &mut [Section],
  rows: &mut [Row],
) {
  if sections.is_empty() {
    return;
  }

  let undistributable = block_border_spacing * (sections.len() as i32 + 1);
  let distributable_table_block_size =
    (table_block_size - undistributable).clamp_negative_to_zero();
  let percentage_size = |section: &Section| {
    section.block_size.max(LayoutUnit::from_float(
      section.percent.unwrap_or_default() * distributable_table_block_size.to_float() / 100.0,
    ))
  };
  let mut minimum_size_guess = LayoutUnit::ZERO;
  let mut percent_size_guess = LayoutUnit::ZERO;
  let mut has_tbody = false;
  let (mut auto, mut fixed, mut percent) = (Vec::new(), Vec::new(), Vec::new());
  let (mut tbody_auto, mut tbody_fixed, mut tbody_percent) = (Vec::new(), Vec::new(), Vec::new());
  let mut auto_size = LayoutUnit::ZERO;
  let mut fixed_size = LayoutUnit::ZERO;
  let mut percent_size = LayoutUnit::ZERO;
  let mut tbody_auto_size = LayoutUnit::ZERO;
  let mut tbody_fixed_size = LayoutUnit::ZERO;
  let mut tbody_percent_size = LayoutUnit::ZERO;

  for (index, section) in sections.iter().enumerate() {
    minimum_size_guess += section.block_size;
    percent_size_guess += if section.percent.is_some() {
      percentage_size(section)
    } else {
      section.block_size
    };
    has_tbody |= section.is_tbody;

    if section.percent.is_some() {
      percent.push(index);
      if section.is_tbody {
        tbody_percent.push(index);
      }
    } else if section.is_constrained {
      fixed.push(index);
      fixed_size += section.block_size;
      if section.is_tbody {
        tbody_fixed.push(index);
        tbody_fixed_size += section.block_size;
      }
    } else {
      auto.push(index);
      auto_size += section.block_size;
      if section.is_tbody {
        tbody_auto.push(index);
        tbody_auto_size += section.block_size;
      }
    }
  }

  // No free space past the sections' minimums.
  if distributable_table_block_size <= minimum_size_guess {
    return;
  }

  // Percent sections grow to their percentage, in proportion to how far that is past their
  // minimum.
  if let Some(&last) = percent.last()
    && percent_size_guess > minimum_size_guess
  {
    let distributable = percent_size_guess.min(distributable_table_block_size) - minimum_size_guess;
    let percent_minimum_difference = percent_size_guess - minimum_size_guess;
    let mut remaining_deficit = distributable;

    for &index in &percent {
      let section = &mut sections[index];
      let delta = distributable.mul_div(
        percentage_size(section) - section.block_size,
        percent_minimum_difference,
      );

      section.block_size += delta;
      section.needs_redistribution = true;
      remaining_deficit -= delta;
      minimum_size_guess += delta;
      percent_size += section.block_size;
      if section.is_tbody {
        tbody_percent_size += section.block_size;
      }
    }
    sections[last].block_size += remaining_deficit;
    percent_size += remaining_deficit;
    minimum_size_guess += remaining_deficit;
    if sections[last].is_tbody {
      tbody_percent_size += remaining_deficit;
    }
  }

  // Bodies grow before headers and footers; of either, auto sections before fixed ones, fixed
  // before percent.
  let (sections_to_grow, sections_size) = if has_tbody {
    if !tbody_auto.is_empty() {
      (tbody_auto, tbody_auto_size)
    } else if !tbody_fixed.is_empty() {
      (tbody_fixed, tbody_fixed_size)
    } else {
      (tbody_percent, tbody_percent_size)
    }
  } else if !auto.is_empty() {
    (auto, auto_size)
  } else if !fixed.is_empty() {
    (fixed, fixed_size)
  } else {
    (percent, percent_size)
  };

  let distributable = distributable_table_block_size - minimum_size_guess;

  if distributable > LayoutUnit::ZERO
    && let Some(&last) = sections_to_grow.last()
  {
    let mut remaining_deficit = distributable;

    for &index in &sections_to_grow {
      let section = &mut sections[index];
      let delta = if sections_size > LayoutUnit::ZERO {
        distributable.mul_div(section.block_size, sections_size)
      } else {
        distributable / sections_to_grow.len() as i32
      };

      section.block_size += delta;
      section.needs_redistribution = true;
      remaining_deficit -= delta;
    }
    sections[last].block_size += remaining_deficit;
  }

  for section in sections
    .iter()
    .filter(|section| section.needs_redistribution)
  {
    distribute_excess_block_size_to_rows(
      section.start_row,
      section.row_count,
      section.block_size,
      false,
      block_border_spacing,
      Some(section.block_size),
      rows,
    );
  }
}

/// Blink's `ColspanCellTabulator`: the column each cell of a row group starts in, past the cells
/// rows above it still span.
#[derive(Default)]
pub(super) struct ColspanCellTabulator {
  current_column: usize,
  /// Each spanning cell's first column, span, and the rows it has left.
  spanning_cells: Vec<(usize, usize, usize)>,
}

impl ColspanCellTabulator {
  pub(super) fn current_column(&self) -> usize {
    self.current_column
  }

  pub(super) fn start_row(&mut self) {
    self.current_column = 0;
  }

  /// Drops the cells that span no more rows.
  pub(super) fn end_row(&mut self) {
    self.spanning_cells.retain_mut(|(.., remaining_rows)| {
      *remaining_rows -= 1;
      *remaining_rows != 0
    });
    self.spanning_cells.sort_by_key(|(start, ..)| *start);
  }

  /// Moves past the columns a cell from an earlier row still covers.
  pub(super) fn find_next_free_column(&mut self) {
    for &(start, span, _) in &self.spanning_cells {
      if start <= self.current_column && start + span > self.current_column {
        self.current_column = start + span;
      }
    }
  }

  pub(super) fn process_cell(&mut self, colspan: usize, rowspan: usize) {
    if rowspan > 1 {
      self
        .spanning_cells
        .push((self.current_column, colspan, rowspan));
    }
    self.current_column += colspan;
  }
}

/// Blink's `RowBaselineTabulator`: a row's block size and baseline from its cells' baselines.
#[derive(Default)]
pub(super) struct RowBaselineTabulator {
  max_cell_ascent: Option<LayoutUnit>,
  max_cell_descent: Option<LayoutUnit>,
  fallback_cell_descent: Option<LayoutUnit>,
}

impl RowBaselineTabulator {
  /// A cell `block_size` tall with its `first_baseline`, aligned as `alignment`; its block-end
  /// border and padding stand in for a baseline it lacks.
  pub(super) fn process_cell(
    &mut self,
    block_size: LayoutUnit,
    first_baseline: Option<LayoutUnit>,
    block_end_border_padding: LayoutUnit,
    alignment: BlockContentAlignment,
    has_descendants: bool,
    is_rowspanned: bool,
  ) {
    if alignment == BlockContentAlignment::Baseline
      && has_descendants
      && let Some(baseline) = first_baseline
    {
      self.max_cell_ascent = Some(
        self
          .max_cell_ascent
          .unwrap_or(LayoutUnit::MIN)
          .max(baseline),
      );
      self.max_cell_descent = Some(if is_rowspanned {
        self.max_cell_descent.unwrap_or_default()
      } else {
        self
          .max_cell_descent
          .unwrap_or(LayoutUnit::MIN)
          .max(block_size - baseline)
      });
    }

    // css-tables-3 §row-layout: without a line box, the baseline is the bottom of the cell's
    // content edge.
    if self.max_cell_ascent.is_none() {
      self.fallback_cell_descent = Some(
        self
          .fallback_cell_descent
          .unwrap_or(LayoutUnit::MAX)
          .min(block_end_border_padding),
      );
    }
  }

  pub(super) fn row_block_size(&self, max_cell_block_size: LayoutUnit) -> LayoutUnit {
    match (self.max_cell_ascent, self.max_cell_descent) {
      (Some(ascent), Some(descent)) => max_cell_block_size.max(ascent + descent),
      _ => max_cell_block_size,
    }
  }

  pub(super) fn baseline(&self, row_block_size: LayoutUnit) -> LayoutUnit {
    match (self.max_cell_ascent, self.fallback_cell_descent) {
      (Some(ascent), _) => ascent,
      (None, Some(descent)) => (row_block_size - descent).clamp_negative_to_zero(),
      // An empty row's baseline is its top.
      (None, None) => LayoutUnit::ZERO,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn px(value: i32) -> LayoutUnit {
    LayoutUnit::from_int(value)
  }

  fn row(block_size: i32) -> Row {
    Row {
      block_size: px(block_size),
      start_cell_index: 0,
      baseline: None,
      percent: None,
      is_constrained: false,
      has_rowspan_start: false,
      is_collapsed: false,
    }
  }

  #[test]
  fn a_rowspan_grows_the_rows_it_covers_in_proportion() {
    let mut rows = [row(10), row(30)];

    distribute_rowspan_cell_to_rows(
      &RowspanCell {
        start_row: 0,
        effective_rowspan: 2,
        min_block_size: px(82),
      },
      px(2),
      &mut rows,
    );

    // 40 to share, past 40 of rows and 2 of spacing.
    assert_eq!([rows[0].block_size, rows[1].block_size], [px(20), px(60)]);
  }

  #[test]
  fn a_table_height_grows_body_rows_before_headers() {
    let mut rows = [row(10), row(10)];
    let mut sections = [
      Section {
        start_row: 0,
        row_count: 1,
        block_size: px(10),
        percent: None,
        is_constrained: false,
        is_tbody: false,
        needs_redistribution: false,
      },
      Section {
        start_row: 1,
        row_count: 1,
        block_size: px(10),
        percent: None,
        is_constrained: false,
        is_tbody: true,
        needs_redistribution: false,
      },
    ];

    distribute_table_block_size_to_sections(px(0), px(100), &mut sections, &mut rows);

    assert_eq!([rows[0].block_size, rows[1].block_size], [px(10), px(90)]);
  }

  #[test]
  fn a_spanning_cell_pushes_the_next_rows_cells_past_its_columns() {
    let mut tabulator = ColspanCellTabulator::default();

    tabulator.start_row();
    tabulator.find_next_free_column();
    tabulator.process_cell(2, 2);
    tabulator.end_row();
    tabulator.start_row();
    tabulator.find_next_free_column();

    assert_eq!(tabulator.current_column(), 2);
  }
}
