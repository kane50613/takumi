//! Table columns: what each cell and `<col>` asks of the columns it spans, the table's inline size
//! shared out between them, and where each column lands. Follows Blink's
//! [`table_layout_algorithm_types.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_layout_algorithm_types.cc)
//! and `table_layout_utils.cc` under the notice in LICENSE-CHROMIUM.

use std::iter::repeat_n;

use crate::{
  layout::{table::parts::TableGroupedChildren, tree::RenderNode},
  layout_unit::LayoutUnit,
  style::{BoxSizing, ComputedStyle, Display, Length, SizingContext, Visibility},
};

/// Blink's `TableTypes::kTableMaxInlineSize`.
pub(super) const TABLE_MAX_INLINE_SIZE: LayoutUnit = LayoutUnit::from_int(1_000_000);

/// A sizing property as Blink's `Length` kinds tell it apart.
#[derive(Clone, Copy)]
pub(super) enum TableLength {
  /// `IsFixed`: an absolute length.
  Fixed(LayoutUnit),
  /// `IsPercent`, 100% stored as 100.
  Percent(f32),
  /// `auto`, a keyword, or a `calc()` mixing lengths and percentages.
  Other,
}

impl TableLength {
  pub(super) fn of(length: Option<Length>, sizing: &SizingContext) -> Self {
    match length {
      None | Some(Length::Auto) => Self::Other,
      Some(Length::Percentage(percent)) => Self::Percent(percent),
      Some(Length::Calc(terms)) => {
        let linear = terms.resolve(sizing);

        match (linear.px, linear.percent) {
          (_, 0.0) => Self::Fixed(LayoutUnit::from_float(linear.px)),
          (0.0, percent) => Self::Percent(percent * 100.0),
          _ => Self::Other,
        }
      }
      Some(length) => Self::Fixed(LayoutUnit::from_float(length.to_px(sizing, 0.0))),
    }
  }

  pub(super) fn fixed(self) -> Option<LayoutUnit> {
    match self {
      Self::Fixed(size) => Some(size),
      _ => None,
    }
  }

  pub(super) fn percent(self) -> Option<f32> {
    match self {
      Self::Percent(percent) => Some(percent),
      _ => None,
    }
  }
}

/// A box's `width`, `min-width`, and `max-width` as Blink's `InlineSizesFromStyle` gathers them
/// for a column or cell, each fixed one grown to hold `border_padding`.
struct InlineSizes {
  inline_size: Option<LayoutUnit>,
  min_inline_size: Option<LayoutUnit>,
  max_inline_size: Option<LayoutUnit>,
  percent: Option<f32>,
}

impl InlineSizes {
  fn of(style: &ComputedStyle, sizing: &SizingContext, border_padding: LayoutUnit) -> Self {
    let box_data = &style.box_data;
    let content_box = box_data.box_sizing == BoxSizing::ContentBox;
    let with_border_padding = |size: LayoutUnit| {
      if content_box {
        size + border_padding
      } else {
        size.max(border_padding)
      }
    };
    let width = TableLength::of(box_data.width.as_length(), sizing);
    let max_length = TableLength::of(box_data.max_width.as_length(), sizing);
    let min_inline_size = TableLength::of(Some(box_data.min_width), sizing)
      .fixed()
      .map(with_border_padding);
    let max_inline_size = max_length
      .fixed()
      .map(with_border_padding)
      .map(|max| min_inline_size.map_or(max, |min| min.max(max)));
    let percent = width
      .percent()
      .map(|percent| max_length.percent().map_or(percent, |max| percent.min(max)));

    Self {
      inline_size: width.fixed().map(with_border_padding),
      min_inline_size,
      max_inline_size,
      percent,
    }
  }
}

/// What one cell asks of the columns it spans, Blink's `TableTypes::CellInlineConstraint`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CellInlineConstraint {
  pub(super) min_inline_size: LayoutUnit,
  pub(super) max_inline_size: LayoutUnit,
  /// 100% stored as 100.
  pub(super) percent: Option<f32>,
  /// The border and padding a percentage counts, in a fixed table.
  pub(super) percent_border_padding: LayoutUnit,
  /// Whether the cell has a fixed `width`.
  pub(super) is_constrained: bool,
}

impl CellInlineConstraint {
  /// Blink's `CreateCellInlineConstraint` for a cell of `style` with `border_padding` across, its
  /// content `min_max` measured only when a table that is not fixed needs it.
  pub(super) fn of(
    style: &ComputedStyle,
    sizing: &SizingContext,
    is_fixed_layout: bool,
    border_padding: LayoutUnit,
    min_max: impl FnOnce() -> (LayoutUnit, LayoutUnit),
  ) -> Self {
    let sizes = InlineSizes::of(style, sizing, border_padding);
    // Measured lazily: a fixed table with a fixed `width` never needs the content.
    let mut measure = Some(min_max);
    let mut cached = None;
    let mut min_max =
      || *cached.get_or_insert_with(|| measure.take().map(|measure| measure()).unwrap_or_default());
    let mut min = if is_fixed_layout {
      LayoutUnit::ZERO
    } else {
      min_max().0.max(sizes.min_inline_size.unwrap_or_default())
    };
    let mut content_max = match sizes.inline_size {
      Some(size) => size,
      None => min_max().1,
    };

    if let Some(max) = sizes.max_inline_size {
      content_max = content_max.min(max);
      min = min.min(max);
    }

    let percent_border_padding = if is_fixed_layout
      && sizes.percent.is_some()
      && style.box_data.box_sizing == BoxSizing::ContentBox
    {
      border_padding
    } else {
      LayoutUnit::ZERO
    };

    Self {
      min_inline_size: min,
      max_inline_size: min.max(content_max),
      percent: sizes.percent,
      percent_border_padding,
      is_constrained: sizes.inline_size.is_some(),
    }
  }

  /// Merges another cell of the same column in, as Blink's `CellInlineConstraint::Encompass`.
  pub(super) fn encompass(&mut self, other: &Self) {
    self.min_inline_size = self.min_inline_size.max(other.min_inline_size);
    self.max_inline_size = match (self.is_constrained, other.is_constrained) {
      (true, false) => self.max_inline_size.max(other.min_inline_size),
      (false, true) => self.min_inline_size.max(other.max_inline_size),
      _ => self.max_inline_size.max(other.max_inline_size),
    };
    self.is_constrained |= other.is_constrained;
    if other.percent > self.percent {
      self.percent = other.percent;
      self.percent_border_padding = other.percent_border_padding;
    }
  }
}

/// A cell that spans several columns, Blink's `TableTypes::ColspanCell`.
#[derive(Clone, Copy, Debug)]
pub(super) struct ColspanCell {
  pub(super) constraint: CellInlineConstraint,
  pub(super) start_column: usize,
  pub(super) span: usize,
}

/// One column's constraints, Blink's `TableTypes::Column`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Column {
  pub(super) min_inline_size: Option<LayoutUnit>,
  pub(super) max_inline_size: Option<LayoutUnit>,
  pub(super) percent: Option<f32>,
  pub(super) percent_border_padding: LayoutUnit,
  pub(super) is_constrained: bool,
  pub(super) is_collapsed: bool,
  pub(super) is_table_fixed: bool,
  /// Whether no cell starts here, so the column takes no space of its own.
  pub(super) is_mergeable: bool,
}

impl Column {
  /// A column no cell or `<col>` has reached yet, in a table laid out `fixed` or not.
  fn empty(is_table_fixed: bool) -> Self {
    Self {
      is_table_fixed,
      is_mergeable: !is_table_fixed,
      ..Self::default()
    }
  }

  /// Blink's `TableTypes::CreateColumn` for a `<col>` or `<colgroup>` of `style`.
  fn of(
    style: &ComputedStyle,
    sizing: &SizingContext,
    default_inline_size: Option<LayoutUnit>,
    is_table_fixed: bool,
  ) -> Self {
    let sizes = InlineSizes::of(style, sizing, LayoutUnit::ZERO);
    let mut inline_size = sizes.inline_size.or(default_inline_size);

    if let (Some(min), Some(size)) = (sizes.min_inline_size, inline_size) {
      inline_size = Some(size.max(min));
    }

    let percent = sizes.percent.filter(|percent| *percent != 0.0);
    let is_mergeable = !is_table_fixed
      && inline_size.unwrap_or_default() == LayoutUnit::ZERO
      && percent.unwrap_or_default() == 0.0;

    Self {
      min_inline_size: Some(sizes.min_inline_size.unwrap_or_default()),
      max_inline_size: inline_size,
      percent,
      percent_border_padding: LayoutUnit::ZERO,
      is_constrained: inline_size.is_some(),
      is_collapsed: style.inherited_data.visibility == Visibility::Collapse,
      is_table_fixed,
      is_mergeable,
    }
  }

  /// Merges the cells that span only this column in, as Blink's `Column::Encompass`.
  fn encompass(&mut self, cell: Option<&CellInlineConstraint>) {
    let Some(cell) = cell else {
      return;
    };

    // Constrained columns in fixed tables take precedence over cells.
    if self.is_constrained && self.is_table_fixed {
      return;
    }
    if !self.is_table_fixed {
      self.is_mergeable = false;
    }

    match self.min_inline_size {
      Some(min) => {
        self.min_inline_size = Some(min.max(cell.min_inline_size));
        self.max_inline_size = Some(match (self.is_constrained, cell.is_constrained) {
          (true, true) => self.max().max(cell.max_inline_size),
          (true, false) => self.max().max(cell.min_inline_size),
          (false, _) => self.max().max(cell.max_inline_size),
        });
      }
      None => {
        self.min_inline_size = Some(cell.min_inline_size);
        self.max_inline_size = Some(cell.max_inline_size);
      }
    }
    if let (Some(min), Some(max)) = (self.min_inline_size, self.max_inline_size) {
      self.max_inline_size = Some(min.max(max));
    }
    if cell.percent > self.percent {
      self.percent = cell.percent;
      self.percent_border_padding = cell.percent_border_padding;
    }
    self.is_constrained |= cell.is_constrained;
  }

  fn min(&self) -> LayoutUnit {
    self.min_inline_size.unwrap_or_default()
  }

  fn max(&self) -> LayoutUnit {
    self.max_inline_size.unwrap_or_default()
  }

  /// Blink's `Column::ResolvePercentInlineSize`.
  fn resolve_percent_inline_size(&self, basis: LayoutUnit) -> LayoutUnit {
    self.min().max(
      LayoutUnit::from_float(self.percent.unwrap_or_default() * basis.to_float() / 100.0)
        + self.percent_border_padding,
    )
  }

  /// Blink's `Column::IsFixed`.
  fn is_fixed(&self) -> bool {
    self.is_constrained && self.percent.is_none() && self.max_inline_size.is_some()
  }
}

/// Where a column lands, Blink's `TableColumnLocation`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ColumnLocation {
  pub(crate) offset: LayoutUnit,
  pub(crate) size: LayoutUnit,
  pub(crate) is_collapsed: bool,
}

/// The `<col>` and `<colgroup>` boxes of a table, walked as Blink's `VisitLayoutTableColumn` walks
/// them: a group spans its columns, or its own `span` when it has none, and no span reaches past
/// `table_column_count`.
pub(super) trait ColumnVisitor {
  /// A `<col>` at `path` from the table, starting at `start_column` and spanning `span`.
  fn visit_column(
    &mut self,
    column: &RenderNode,
    path: [usize; 2],
    start_column: usize,
    span: usize,
  );

  fn enter_group(&mut self, _group: &RenderNode, _start_column: usize) {}

  /// The `<colgroup>` at `index` among the table's children, done.
  fn leave_group(
    &mut self,
    group: &RenderNode,
    index: usize,
    start_column: usize,
    span: usize,
    has_children: bool,
  );
}

pub(super) fn visit_columns<V: ColumnVisitor>(
  table: &RenderNode,
  grouped: &TableGroupedChildren,
  table_column_count: usize,
  visitor: &mut V,
) {
  let children = table.children.as_deref().unwrap_or_default();
  let mut current = 0;

  for &index in &grouped.columns {
    if current >= table_column_count {
      break;
    }
    let Some(column) = children.get(index) else {
      continue;
    };
    let visit_column =
      |visitor: &mut V, column: &RenderNode, path: [usize; 2], current: &mut usize| {
        let span = column
          .table_column_span()
          .min(table_column_count - *current);

        visitor.visit_column(column, path, *current, span);
        *current += span;
        span
      };

    if column.context.style.base_data.display == Display::TableColumn {
      visit_column(visitor, column, [index, usize::MAX], &mut current);
      continue;
    }

    visitor.enter_group(column, current);
    let start = current;
    let cols = column.children.as_deref().unwrap_or_default();
    let mut span = 0;

    for (child_index, col) in cols.iter().enumerate() {
      span += visit_column(visitor, col, [index, child_index], &mut current);
      if current >= table_column_count {
        break;
      }
    }
    if cols.is_empty() {
      span = column.table_column_span().min(table_column_count - current);
      current += span;
    }
    visitor.leave_group(column, index, start, span, !cols.is_empty());
  }
}

/// Blink's `ColumnConstraintsBuilder`: the constraints `<col>` and `<colgroup>` widths set.
struct ColumnElementConstraints<'s> {
  columns: Vec<Column>,
  sizing: &'s SizingContext,
  is_fixed_layout: bool,
  group: Option<Column>,
}

impl ColumnVisitor for ColumnElementConstraints<'_> {
  fn visit_column(&mut self, column: &RenderNode, _path: [usize; 2], _start: usize, span: usize) {
    let default_inline_size = self
      .group
      .filter(|_| !self.is_fixed_layout)
      .and_then(|group| group.max_inline_size);
    let constraint = Column::of(
      &column.context.style,
      self.sizing,
      default_inline_size,
      self.is_fixed_layout,
    );

    self.columns.extend(repeat_n(constraint, span));
  }

  fn enter_group(&mut self, group: &RenderNode, _start: usize) {
    self.group = Some(Column::of(
      &group.context.style,
      self.sizing,
      None,
      self.is_fixed_layout,
    ));
  }

  fn leave_group(
    &mut self,
    _group: &RenderNode,
    _index: usize,
    _start: usize,
    span: usize,
    has_children: bool,
  ) {
    if let Some(group) = self.group.take()
      && !has_children
    {
      self.columns.extend(repeat_n(group, span));
    }
  }
}

/// Every column's constraints, Blink's `TableTypes::Columns`.
#[derive(Clone, Debug, Default)]
pub(crate) struct ColumnConstraints {
  pub(super) columns: Vec<Column>,
}

impl ColumnConstraints {
  /// The constraints the table's `<col>` and `<colgroup>` boxes set, Blink's
  /// `ComputeColumnElementConstraints`.
  pub(super) fn of_column_elements(
    table: &RenderNode,
    grouped: &TableGroupedChildren,
    is_fixed_layout: bool,
  ) -> Self {
    let mut builder = ColumnElementConstraints {
      columns: Vec::new(),
      sizing: &table.context.sizing,
      is_fixed_layout,
      group: None,
    };

    visit_columns(table, grouped, usize::MAX, &mut builder);
    Self {
      columns: builder.columns,
    }
  }

  /// Blink's `ComputeMaximumNonMergeableColumnCount`: the columns up to the last one the
  /// `<col>` widths do not merge away.
  pub(super) fn maximum_non_mergeable_count(&self) -> usize {
    self
      .columns
      .iter()
      .rposition(|column| !column.is_mergeable)
      .map_or(0, |last| last + 1)
  }

  /// Blink's `ApplyCellConstraintsToColumnConstraints`.
  pub(super) fn apply_cell_constraints(
    &mut self,
    cells: &[Option<CellInlineConstraint>],
    inline_border_spacing: LayoutUnit,
    is_fixed_layout: bool,
    mut colspan_cells: Vec<ColspanCell>,
  ) {
    if self.columns.len() < cells.len() {
      self
        .columns
        .resize(cells.len(), Column::empty(is_fixed_layout));
    } else if self.columns.len() > cells.len() {
      // Trims mergeable columns off the end, as far as none when no cell holds any.
      let mut len = self.columns.len();

      while len > cells.len() && self.columns[len - 1].is_mergeable {
        len -= 1;
      }
      self.columns.truncate(len);
    }

    for colspan in &colspan_cells {
      self.ensure_distributable_column_exists(colspan.start_column, colspan.span);
    }
    for (column, cell) in self.columns.iter_mut().zip(cells) {
      column.encompass(cell.as_ref());
    }

    colspan_cells.sort_by_key(|cell| (cell.span, cell.start_column));
    for colspan in &colspan_cells {
      if is_fixed_layout {
        self.distribute_colspan_fixed(colspan, inline_border_spacing);
      } else {
        self.distribute_colspan_auto(colspan, inline_border_spacing);
      }
    }

    // Auto tables leave each column no more than what earlier columns left of 100%, and fixed
    // tables scale every percentage down to fit 100%.
    let mut total_percent = 0.0;

    for column in &mut self.columns {
      if let Some(percent) = column.percent.as_mut() {
        if !is_fixed_layout && *percent + total_percent > 100.0 {
          *percent = 100.0 - total_percent;
        }
        total_percent += *percent;
      }
      column.min_inline_size = Some(column.min());
      column.max_inline_size = Some(column.max());
    }
    if is_fixed_layout && total_percent > 100.0 {
      for percent in self
        .columns
        .iter_mut()
        .filter_map(|column| column.percent.as_mut())
      {
        *percent = *percent * 100.0 / total_percent;
      }
    }
  }

  /// Blink's `EnsureDistributableColumnExists`: marks the first column a span covers that is not
  /// collapsed, or else its first, as taking space.
  fn ensure_distributable_column_exists(&mut self, start: usize, span: usize) {
    let end = (start + span).min(self.columns.len());
    let Some(columns) = self.columns.get_mut(start..end) else {
      return;
    };

    match columns.iter_mut().find(|column| !column.is_collapsed) {
      Some(column) => column.is_mergeable = false,
      None => {
        if let Some(first) = columns.first_mut() {
          first.is_mergeable = false;
        }
      }
    }
  }

  /// The border spacing between the columns of `span` that take space.
  fn inner_border_spacing(span: &[Column], spacing: LayoutUnit) -> LayoutUnit {
    let count = span.iter().filter(|column| !column.is_mergeable).count();

    spacing * count.saturating_sub(1) as i32
  }

  /// Blink's `DistributeColspanCellToColumnsFixed`.
  fn distribute_colspan_fixed(&mut self, colspan: &ColspanCell, spacing: LayoutUnit) {
    let end = (colspan.start_column + colspan.span).min(self.columns.len());
    let Some(span) = self.columns.get_mut(colspan.start_column..end) else {
      return;
    };
    let total_inner_spacing = Self::inner_border_spacing(span, spacing);
    let effective_span = span.iter().filter(|column| !column.is_mergeable).count() as f32;
    let constraint = colspan.constraint;
    let min = if constraint.is_constrained {
      (constraint.min_inline_size - total_inner_spacing).clamp_negative_to_zero()
    } else {
      LayoutUnit::ZERO
    };
    let max = (constraint.max_inline_size - total_inner_spacing).clamp_negative_to_zero();
    let mut min_rounding_error = min;
    let mut max_rounding_error = max;
    let new_min = LayoutUnit::from_float(min.to_float() / effective_span);
    let new_max = LayoutUnit::from_float(max.to_float() / effective_span);
    let new_percent = constraint.percent.map(|percent| percent / effective_span);
    let mut last = None;

    for (index, column) in span.iter_mut().enumerate() {
      if column.is_mergeable {
        continue;
      }
      last = Some(index);
      min_rounding_error -= new_min;
      max_rounding_error -= new_max;
      if column.min_inline_size.is_none() {
        column.is_constrained |= constraint.is_constrained;
        column.min_inline_size = Some(new_min);
      }
      if column.max_inline_size.is_none() {
        column.is_constrained |= constraint.is_constrained;
        column.max_inline_size = Some(new_max);
      }
      // Percentages only get distributed over auto columns.
      if column.percent.is_none() && !column.is_constrained && new_percent.is_some() {
        column.percent = new_percent;
      }
    }

    if let Some(last) = last.and_then(|last| span.get_mut(last)) {
      last.min_inline_size = Some(last.min() + min_rounding_error);
      last.max_inline_size = Some(last.max() + max_rounding_error);
    }
  }

  /// Blink's `DistributeColspanCellToColumnsAuto`.
  fn distribute_colspan_auto(&mut self, colspan: &ColspanCell, spacing: LayoutUnit) {
    if self.columns.is_empty() {
      return;
    }
    let end = (colspan.start_column + colspan.span).min(self.columns.len());
    let Some(span) = self.columns.get_mut(colspan.start_column..end) else {
      return;
    };
    let total_inner_spacing = Self::inner_border_spacing(span, spacing);
    let constraint = colspan.constraint;
    let min = (constraint.min_inline_size - total_inner_spacing).clamp_negative_to_zero();
    let max = (constraint.max_inline_size - total_inner_spacing).clamp_negative_to_zero();

    if let Some(cell_percent) = constraint.percent {
      let mut columns_percent = 0.0;
      let mut all_count = 0;
      let mut percent_count = 0;
      let mut nonpercent_count = 0;
      let mut nonpercent_max = LayoutUnit::ZERO;

      for column in span.iter_mut() {
        column.max_inline_size.get_or_insert(LayoutUnit::ZERO);
        column.min_inline_size.get_or_insert(LayoutUnit::ZERO);
        if column.is_mergeable {
          continue;
        }
        all_count += 1;
        match column.percent {
          Some(percent) => {
            percent_count += 1;
            columns_percent += percent;
          }
          None => {
            nonpercent_count += 1;
            nonpercent_max += column.max();
          }
        }
      }

      let surplus = cell_percent - columns_percent;

      if surplus > 0.0 && all_count > percent_count {
        for column in span
          .iter_mut()
          .filter(|column| column.percent.is_none() && !column.is_mergeable)
        {
          column.percent = Some(if nonpercent_max != LayoutUnit::ZERO {
            surplus * column.max().to_float() / nonpercent_max.to_float()
          } else {
            surplus / nonpercent_count as f32
          });
        }
      }
    }

    for column in span.iter_mut() {
      column.min_inline_size.get_or_insert(LayoutUnit::ZERO);
      column.max_inline_size.get_or_insert(LayoutUnit::ZERO);
    }

    let sizes = distribute_inline_size_auto(min, span, true);

    for (column, size) in span.iter_mut().zip(sizes) {
      column.min_inline_size = Some(column.min().max(size));
    }

    let sizes = distribute_inline_size_auto(max, span, constraint.is_constrained);

    for (column, size) in span.iter_mut().zip(sizes) {
      column.max_inline_size = Some(column.min().max(column.max()).max(size));
    }
  }

  /// Blink's `ComputeUndistributableTableSpace`: the table's inline border and padding, and the
  /// border spacing at its edges and between the columns that take space.
  pub(super) fn undistributable_space(
    &self,
    inline_border_padding: LayoutUnit,
    inline_border_spacing: LayoutUnit,
  ) -> LayoutUnit {
    let non_mergeable = self
      .columns
      .iter()
      .filter(|column| !column.is_mergeable)
      .count();
    let spaces = 2 + non_mergeable.saturating_sub(1);

    inline_border_padding + inline_border_spacing * spaces as i32
  }

  /// Blink's `ComputeGridInlineMinMax`: the table's min- and max-content widths around
  /// `undistributable_space`, a percentage column widening the maximum when
  /// `allow_column_percentages`.
  pub(super) fn grid_inline_min_max(
    &self,
    undistributable_space: LayoutUnit,
    is_fixed_layout: bool,
    allow_column_percentages: bool,
  ) -> (LayoutUnit, LayoutUnit) {
    let mut min = LayoutUnit::ZERO;
    let mut max = LayoutUnit::ZERO;
    let mut percent_max_size_estimate = LayoutUnit::ZERO;
    let mut non_percent_max_size_sum = LayoutUnit::ZERO;
    let mut percent_sum = 0.0_f32;

    for column in &self.columns {
      if let Some(column_min) = column.min_inline_size {
        // In fixed layout, constrained cells minimum inline size is their maximum.
        min += if is_fixed_layout && column.is_fixed() {
          column.max()
        } else {
          column_min
        };
        match column.percent.filter(|percent| *percent > 0.0) {
          Some(percent) => {
            if column.max() > LayoutUnit::ZERO {
              let estimate = LayoutUnit::from_float(
                100.0 / percent * (column.max() - column.percent_border_padding).to_float(),
              );

              percent_max_size_estimate = percent_max_size_estimate.max(estimate);
            }
          }
          None => non_percent_max_size_sum += column.max(),
        }
      }
      if let Some(column_max) = column.max_inline_size {
        max += column_max;
      }
      if let Some(percent) = column.percent {
        percent_sum += percent;
      }
    }

    let percent_sum = percent_sum.min(100.0);

    if percent_sum > 0.0 && allow_column_percentages {
      let size_from_percent_and_fixed = if non_percent_max_size_sum == LayoutUnit::ZERO {
        LayoutUnit::ZERO
      } else if percent_sum == 100.0 {
        TABLE_MAX_INLINE_SIZE
      } else {
        LayoutUnit::from_float(100.0 / (100.0 - percent_sum) * non_percent_max_size_sum.to_float())
      };

      max = max
        .max(size_from_percent_and_fixed)
        .max(percent_max_size_estimate);
    }

    let max = min.max(max);

    (min + undistributable_space, max + undistributable_space)
  }

  /// Each column's inline size once `assignable` is shared out, Blink's
  /// `SynchronizeAssignableTableInlineSizeAndColumns`.
  pub(super) fn sizes(&self, assignable: LayoutUnit, is_fixed_layout: bool) -> Vec<LayoutUnit> {
    if self.columns.is_empty() {
      Vec::new()
    } else if is_fixed_layout {
      self.sizes_fixed(assignable)
    } else {
      distribute_inline_size_auto(assignable, &self.columns, true)
    }
  }

  /// Blink's `SynchronizeAssignableTableInlineSizeAndColumnsFixed`.
  fn sizes_fixed(&self, target: LayoutUnit) -> Vec<LayoutUnit> {
    // Columns of width 0 are treated as auto by all browsers.
    let treat_as_fixed = |column: &Column| column.is_fixed() && column.max() != LayoutUnit::ZERO;
    let zero_constrained =
      |column: &Column| column.is_constrained && column.max_inline_size == Some(LayoutUnit::ZERO);
    let mut sizes = vec![LayoutUnit::ZERO; self.columns.len()];
    let (mut percent_count, mut auto_count, mut fixed_count, mut zero_count) = (0, 0, 0, 0);
    let mut total_percent_size = LayoutUnit::ZERO;
    let mut total_fixed_size = LayoutUnit::ZERO;
    let mut assigned = LayoutUnit::ZERO;
    let mut last = None;

    for column in &self.columns {
      if column.percent.is_some() {
        percent_count += 1;
        total_percent_size += column.resolve_percent_inline_size(target);
      } else if treat_as_fixed(column) {
        fixed_count += 1;
        total_fixed_size += column.max();
      } else if zero_constrained(column) {
        zero_count += 1;
      } else {
        auto_count += 1;
      }
    }

    if fixed_count > 0 {
      let target_fixed_size = (target - total_percent_size).clamp_negative_to_zero();
      let scale_up = total_fixed_size < target_fixed_size && auto_count == 0;
      let scale_down = total_fixed_size > target;
      let scale = if !(scale_up || scale_down) {
        Some(1.0)
      } else if total_fixed_size != LayoutUnit::ZERO {
        Some(target_fixed_size.to_float() / total_fixed_size.to_float())
      } else {
        None
      };

      for (index, column) in self.columns.iter().enumerate() {
        if !treat_as_fixed(column) {
          continue;
        }
        last = Some(index);
        sizes[index] = match scale {
          Some(scale) => LayoutUnit::from_float(scale * column.max().to_float()),
          None => LayoutUnit::from_float(target.to_float() / fixed_count as f32),
        };
        assigned += sizes[index];
      }
    }
    if assigned >= target {
      return sizes;
    }
    if percent_count > 0 {
      let room = target - assigned;
      let scale_up = total_percent_size < room && auto_count == 0;
      let scale_down = total_percent_size > room;
      let scale = if !(scale_up || scale_down) {
        Some(1.0)
      } else if total_percent_size != LayoutUnit::ZERO {
        Some(room.to_float() / total_percent_size.to_float())
      } else {
        None
      };

      for (index, column) in self.columns.iter().enumerate() {
        if column.percent.is_none() {
          continue;
        }
        last = Some(index);
        sizes[index] = match scale {
          Some(scale) => {
            LayoutUnit::from_float(scale * column.resolve_percent_inline_size(target).to_float())
          }
          None => LayoutUnit::from_float(room.to_float() / percent_count as f32),
        };
        assigned += sizes[index];
      }
    }

    // Distributes to auto columns, and zero-width ones only if every column is.
    let distributing = target - assigned;
    let distribute_zero = zero_count == self.columns.len();

    for (index, column) in self.columns.iter().enumerate() {
      if column.percent.is_some() || treat_as_fixed(column) {
        continue;
      }
      if zero_constrained(column) && !distribute_zero {
        continue;
      }
      last = Some(index);
      let count = if distribute_zero {
        zero_count
      } else {
        auto_count
      };

      sizes[index] = LayoutUnit::from_float(distributing.to_float() / count as f32);
      assigned += sizes[index];
    }

    if let Some(last) = last {
      sizes[last] += target - assigned;
    }
    sizes
  }

  /// Where each column lands, `inline_border_spacing` apart, as Blink's
  /// `ComputeLocationsFromColumns` places them: an empty mergeable column, or a collapsed one when
  /// `shrink_collapsed`, takes no room. Also says whether any column is collapsed.
  pub(super) fn locations(
    &self,
    sizes: &[LayoutUnit],
    inline_border_spacing: LayoutUnit,
    shrink_collapsed: bool,
  ) -> (Vec<ColumnLocation>, bool) {
    let mut has_collapsed_columns = false;
    let mut is_first = true;
    let mut offset = inline_border_spacing;
    let locations = self
      .columns
      .iter()
      .zip(sizes)
      .map(|(column, &size)| {
        has_collapsed_columns |= column.is_collapsed;
        if (column.is_mergeable && size == LayoutUnit::ZERO)
          || (shrink_collapsed && column.is_collapsed)
        {
          return ColumnLocation {
            offset,
            size: LayoutUnit::ZERO,
            is_collapsed: true,
          };
        }
        if is_first {
          is_first = false;
        } else {
          offset += inline_border_spacing;
        }
        let location = ColumnLocation {
          offset,
          size,
          is_collapsed: false,
        };

        offset += size;
        location
      })
      .collect();

    (locations, has_collapsed_columns)
  }
}

/// The guesses of css-tables-3's width distribution, in the order they grow.
/// <https://www.w3.org/TR/css-tables-3/#width-distribution-algorithm>
#[derive(Clone, Copy, PartialEq, Eq)]
enum Guess {
  Min,
  Percentage,
  Specified,
  Max,
  AboveMax,
}

/// `target` shared out over `columns`, Blink's `DistributeInlineSizeToComputedInlineSizeAuto`. A
/// `constrained` target may grow fixed columns past their widths.
fn distribute_inline_size_auto(
  target: LayoutUnit,
  columns: &[Column],
  constrained: bool,
) -> Vec<LayoutUnit> {
  let mut guess_sizes = [LayoutUnit::ZERO; 4];
  let mut guess_increases = [LayoutUnit::ZERO; 4];
  let (mut percent_count, mut fixed_count, mut auto_count) = (0, 0, 0);
  let mut total_percent = 0.0_f32;
  let mut total_auto_max = LayoutUnit::ZERO;
  let mut total_fixed_max = LayoutUnit::ZERO;

  for column in columns.iter().filter(|column| !column.is_mergeable) {
    let (min, max) = (column.min(), column.max());

    if let Some(percent) = column.percent {
      let percent_size = column.resolve_percent_inline_size(target);

      percent_count += 1;
      total_percent += percent;
      guess_sizes[0] += min;
      guess_sizes[1] += percent_size;
      guess_sizes[2] += percent_size;
      guess_sizes[3] += percent_size;
      guess_increases[1] += percent_size - min;
    } else if column.is_constrained {
      fixed_count += 1;
      total_fixed_max += max;
      guess_sizes[0] += min;
      guess_sizes[1] += min;
      guess_sizes[2] += max;
      guess_sizes[3] += max;
      guess_increases[2] += max - min;
    } else {
      auto_count += 1;
      total_auto_max += max;
      guess_sizes[0] += min;
      guess_sizes[1] += min;
      guess_sizes[2] += min;
      guess_sizes[3] += max;
      guess_increases[3] += max - min;
    }
  }

  // Distributing never takes a column below its minimum.
  let target = target.max(guess_sizes[0]);
  let guess = [Guess::Min, Guess::Percentage, Guess::Specified, Guess::Max]
    .into_iter()
    .zip(guess_sizes)
    .find(|(_, size)| *size >= target)
    .map_or(Guess::AboveMax, |(guess, _)| guess);
  let mut sizes = vec![LayoutUnit::ZERO; columns.len()];
  let mut remaining_deficit = LayoutUnit::ZERO;
  let mut last = None;
  // The share of `distributable` a column growing by `increase` of `total` takes, or an even
  // share of `count` when nothing grows.
  let share = |distributable: LayoutUnit, increase: LayoutUnit, total: LayoutUnit, count: i32| {
    if total > LayoutUnit::ZERO {
      distributable.mul_div(increase, total)
    } else {
      distributable / count
    }
  };

  match guess {
    Guess::Min => {
      for (size, column) in sizes.iter_mut().zip(columns) {
        if !column.is_mergeable {
          *size = column.min();
        }
      }
    }
    Guess::Percentage => {
      let distributable = target - guess_sizes[0];

      remaining_deficit = distributable;
      for (index, column) in columns.iter().enumerate() {
        if column.is_mergeable {
          continue;
        }
        sizes[index] = if column.percent.is_some() {
          let increase = column.resolve_percent_inline_size(target) - column.min();
          let delta = share(distributable, increase, guess_increases[1], percent_count);

          last = Some(index);
          remaining_deficit -= delta;
          column.min() + delta
        } else {
          column.min()
        };
      }
    }
    Guess::Specified => {
      let distributable = target - guess_sizes[1];

      remaining_deficit = distributable;
      for (index, column) in columns.iter().enumerate() {
        if column.is_mergeable {
          continue;
        }
        sizes[index] = if column.percent.is_some() {
          column.resolve_percent_inline_size(target)
        } else if column.is_constrained {
          let delta = share(
            distributable,
            column.max() - column.min(),
            guess_increases[2],
            fixed_count,
          );

          last = Some(index);
          remaining_deficit -= delta;
          column.min() + delta
        } else {
          column.min()
        };
      }
    }
    Guess::Max => {
      let distributable = target - guess_sizes[2];
      // An exact match usually means an auto table sized to fit its content without wrapping,
      // where the distributing math could round a column short and wrap a line.
      let is_exact_match = target == guess_sizes[3];

      remaining_deficit = if is_exact_match {
        LayoutUnit::ZERO
      } else {
        distributable
      };
      for (index, column) in columns.iter().enumerate() {
        if column.is_mergeable {
          continue;
        }
        sizes[index] = if column.percent.is_some() {
          column.resolve_percent_inline_size(target)
        } else if column.is_constrained || is_exact_match {
          column.max()
        } else {
          let delta = share(
            distributable,
            column.max() - column.min(),
            guess_increases[3],
            auto_count,
          );

          last = Some(index);
          remaining_deficit -= delta;
          column.min() + delta
        };
      }
    }
    Guess::AboveMax => {
      let distributable = target - guess_sizes[3];

      remaining_deficit = distributable;
      if auto_count > 0 {
        for (index, column) in columns.iter().enumerate() {
          if column.is_mergeable {
            continue;
          }
          sizes[index] = if column.percent.is_some() {
            column.resolve_percent_inline_size(target)
          } else if column.is_constrained {
            column.max()
          } else {
            let delta = share(distributable, column.max(), total_auto_max, auto_count);

            last = Some(index);
            remaining_deficit -= delta;
            column.max() + delta
          };
        }
      } else if fixed_count > 0 && constrained {
        for (index, column) in columns.iter().enumerate() {
          if column.is_mergeable {
            continue;
          }
          sizes[index] = if column.percent.is_some() {
            column.resolve_percent_inline_size(target)
          } else {
            let delta = share(distributable, column.max(), total_fixed_max, fixed_count);

            last = Some(index);
            remaining_deficit -= delta;
            column.max() + delta
          };
        }
      } else if percent_count > 0 {
        for (index, column) in columns.iter().enumerate() {
          let Some(percent) = column.percent.filter(|_| !column.is_mergeable) else {
            continue;
          };
          let delta = if total_percent != 0.0 {
            LayoutUnit::from_float(distributable.to_float() * percent / total_percent)
          } else {
            distributable / percent_count
          };

          last = Some(index);
          remaining_deficit -= delta;
          sizes[index] = column.resolve_percent_inline_size(target) + delta;
        }
      } else {
        remaining_deficit = LayoutUnit::ZERO;
      }
    }
  }

  // Blink's `if (remaining_deficit != LayoutUnit() && last_computed_size)` skips a deficit owed to
  // the first column when every other column is mergeable.
  let skips_first_column =
    guess == Guess::AboveMax && auto_count == 0 && !(fixed_count > 0 && constrained);

  if let Some(last) = last.filter(|&last| !(skips_first_column && last == 0)) {
    sizes[last] += remaining_deficit;
  }
  sizes
}

#[cfg(test)]
mod tests {
  use super::*;

  fn px(value: i32) -> LayoutUnit {
    LayoutUnit::from_int(value)
  }

  fn cell(min: i32, max: i32) -> Option<CellInlineConstraint> {
    Some(CellInlineConstraint {
      min_inline_size: px(min),
      max_inline_size: px(max),
      percent: None,
      percent_border_padding: LayoutUnit::ZERO,
      is_constrained: false,
    })
  }

  fn columns(cells: &[Option<CellInlineConstraint>], fixed: bool) -> ColumnConstraints {
    let mut columns = ColumnConstraints::default();

    columns.apply_cell_constraints(cells, LayoutUnit::ZERO, fixed, Vec::new());
    columns
  }

  #[test]
  fn a_narrow_table_shares_what_is_past_the_minimums_by_the_room_each_column_wants() {
    let table = columns(&[cell(10, 50), cell(20, 120)], false);

    // 30 past the minimums of 30, over wants of 40 and 100.
    assert_eq!(
      table.sizes(px(60), false),
      vec![
        px(10) + px(30).mul_div(px(40), px(140)),
        px(50) - px(30).mul_div(px(40), px(140))
      ]
    );
  }

  #[test]
  fn a_wide_table_grows_auto_columns_by_their_maximums() {
    let table = columns(&[cell(10, 50), cell(20, 150)], false);

    assert_eq!(table.sizes(px(400), false), vec![px(100), px(300)]);
  }

  #[test]
  fn a_percentage_column_takes_its_share_first() {
    let percent = Some(CellInlineConstraint {
      percent: Some(30.0),
      ..cell(10, 10).expect("cell")
    });
    let table = columns(&[percent, cell(10, 40)], false);

    assert_eq!(table.sizes(px(200), false), vec![px(60), px(140)]);
  }

  #[test]
  fn the_grid_widths_count_the_border_spacing() {
    let mut table = ColumnConstraints::default();

    table.apply_cell_constraints(&[cell(10, 50), cell(20, 150)], px(2), false, Vec::new());
    let undistributable = table.undistributable_space(px(4) - px(4), px(2));

    assert_eq!(undistributable, px(6));
    assert_eq!(
      table.grid_inline_min_max(undistributable, false, true),
      (px(36), px(206))
    );
  }

  #[test]
  fn a_fixed_table_shares_the_rest_evenly_after_a_fixed_column() {
    let fixed = Some(CellInlineConstraint {
      is_constrained: true,
      ..cell(0, 120).expect("cell")
    });
    let table = columns(&[fixed, cell(0, 30), cell(0, 900)], true);

    assert_eq!(table.sizes(px(420), true), vec![px(120), px(150), px(150)]);
  }

  #[test]
  fn an_empty_mergeable_column_lands_collapsed_between_its_neighbours() {
    let table = ColumnConstraints {
      columns: vec![
        Column {
          min_inline_size: Some(px(10)),
          max_inline_size: Some(px(10)),
          ..Column::default()
        },
        Column {
          is_mergeable: true,
          ..Column::default()
        },
        Column {
          min_inline_size: Some(px(10)),
          max_inline_size: Some(px(10)),
          ..Column::default()
        },
      ],
    };
    let (locations, _) = table.locations(&[px(10), px(0), px(10)], px(2), false);

    assert_eq!(
      locations
        .iter()
        .map(|location| location.offset)
        .collect::<Vec<_>>(),
      vec![px(2), px(12), px(14)]
    );
    assert!(locations[1].is_collapsed);
  }
}
