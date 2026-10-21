//! A table's borders: the border each cell lays out with, and in the collapsing model every edge
//! of the grid resolved to the one border that wins it. Follows Blink's
//! [`table_borders.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_borders.cc)
//! under the notice in LICENSE-CHROMIUM.

use crate::{
  context::RenderContext,
  layout::{
    table::{
      columns::{ColumnConstraints, ColumnVisitor, visit_columns},
      parts::{MAX_ROWSPAN, TableGroupedChildren},
      rows::ColspanCellTabulator,
    },
    tree::RenderNode,
  },
  layout_unit::{BoxStrut, LayoutUnit},
  style::{BorderCollapse, BorderStyle, Color, Direction},
};

/// A physical side of the box a border came from, Blink's `TableBorders::EdgeSide`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EdgeSide {
  Top,
  Right,
  Bottom,
  Left,
  /// An edge inside a spanning cell, which no border may fill.
  DoNotFill,
}

impl EdgeSide {
  fn index(self) -> Option<usize> {
    match self {
      Self::Top => Some(0),
      Self::Right => Some(1),
      Self::Bottom => Some(2),
      Self::Left => Some(3),
      Self::DoNotFill => None,
    }
  }
}

/// Which box a border came from, Blink's `TableBorders::EdgeSource`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeSource {
  Cell,
  Row,
  Section,
  Column,
  Table,
}

/// One box's borders, resolved: each side's used width, the style it collapses as, and its colour.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BorderSource {
  pub(crate) widths: [LayoutUnit; 4],
  pub(crate) styles: [BorderStyle; 4],
  pub(crate) colors: [Color; 4],
}

impl BorderSource {
  /// The borders of the box `context` styles, clockwise from the top.
  pub(crate) fn of(context: &RenderContext) -> Self {
    let style = &context.style;
    let box_data = &style.box_data;
    let surround = &style.surround_data;
    let styles = [
      box_data.border_top_style,
      box_data.border_right_style,
      box_data.border_bottom_style,
      box_data.border_left_style,
    ];
    let widths = [
      box_data.border_top_width,
      box_data.border_right_width,
      box_data.border_bottom_width,
      box_data.border_left_width,
    ];

    Self {
      widths: [0, 1, 2, 3].map(|side| {
        if styles[side].is_rendered() {
          LayoutUnit::from_float(widths[side].to_used_px(&context.sizing))
        } else {
          LayoutUnit::ZERO
        }
      }),
      styles: styles.map(collapsed_style),
      colors: [
        surround.border_top_color,
        surround.border_right_color,
        surround.border_bottom_color,
        surround.border_left_color,
      ]
      .map(|color| color.resolve(context.current_color)),
    }
  }

  fn has_border(&self) -> bool {
    self.styles.iter().any(|style| *style != BorderStyle::None)
  }

  /// The borders as a strut, Blink's `ComputeBorders` of a box that is not a table part.
  pub(crate) fn strut(&self) -> BoxStrut {
    BoxStrut {
      top: self.widths[0],
      right: self.widths[1],
      bottom: self.widths[2],
      left: self.widths[3],
    }
  }
}

/// Blink's `ComputedStyle::CollapsedBorderStyle`: an inset border collapses as the ridge it draws,
/// and an outset one as a groove.
fn collapsed_style(style: BorderStyle) -> BorderStyle {
  match style {
    BorderStyle::Inset => BorderStyle::Ridge,
    BorderStyle::Outset => BorderStyle::Groove,
    style => style,
  }
}

/// How strongly a style wins a conflict, in Blink's `EBorderStyle` order.
/// <https://www.w3.org/TR/css-tables-3/#border-specificity>
pub(crate) fn style_rank(style: BorderStyle) -> u8 {
  match style {
    BorderStyle::None => 0,
    BorderStyle::Hidden => 1,
    BorderStyle::Inset => 2,
    BorderStyle::Groove => 3,
    BorderStyle::Outset => 4,
    BorderStyle::Ridge => 5,
    BorderStyle::Dotted => 6,
    BorderStyle::Dashed => 7,
    BorderStyle::Solid => 8,
    BorderStyle::Double => 9,
  }
}

/// One edge of the grid, Blink's `TableBorders::Edge`: the side of the source box whose border
/// fills it, and the order that box was visited in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Edge {
  pub(crate) source: Option<u32>,
  pub(crate) side: EdgeSide,
  pub(crate) box_order: u32,
}

impl Edge {
  const EMPTY: Self = Self {
    source: None,
    side: EdgeSide::Top,
    box_order: 0,
  };
}

/// A table's borders, Blink's `TableBorders`. In the collapsing model each row of the grid holds
/// two edges per column and one more column's worth, alternating the edge along the column's
/// inline-start and the edge along its block-start.
#[derive(Clone, Debug)]
pub(crate) struct TableBorders {
  pub(crate) edges: Vec<Edge>,
  pub(crate) sources: Vec<BorderSource>,
  /// Each section's first row and row count.
  sections: Vec<(usize, usize)>,
  pub(crate) edges_per_row: usize,
  table_border: BoxStrut,
  last_column_index: usize,
  pub(crate) is_collapsed: bool,
  /// Whether the table runs right to left, so its columns' inline-start edges are their right ones.
  rtl: bool,
}

impl TableBorders {
  /// Blink's `TableBorders::ComputeTableBorders`.
  pub(crate) fn of(table: &RenderNode, grouped: &TableGroupedChildren) -> Self {
    let table_source = BorderSource::of(&table.context);
    let is_collapsed =
      table.context.style.inherited_data.border_collapse == BorderCollapse::Collapse;
    let mut borders = Self {
      edges: Vec::new(),
      sources: Vec::new(),
      sections: Vec::new(),
      edges_per_row: 0,
      table_border: table_source.strut(),
      last_column_index: usize::MAX,
      is_collapsed,
      rtl: table.context.style.inherited_data.direction == Direction::Rtl,
    };

    if !is_collapsed {
      return borders;
    }

    let children = table.children.as_deref().unwrap_or_default();
    let is_fixed_layout = table.is_fixed_table_layout();
    let mut box_order = 0;
    let mut table_column_count =
      ColumnConstraints::of_column_elements(table, grouped, is_fixed_layout)
        .maximum_non_mergeable_count();
    let mut row_index = 0;
    let mut found_multispan_cells = false;

    // Marks cell borders, once the grid's size is known, unless a span needs the sections' row
    // counts first.
    for (section, _) in grouped.sections() {
      let section_start_row = row_index;
      let mut tabulator = ColspanCellTabulator::default();

      for row in children[section].children.iter().flatten() {
        tabulator.start_row();
        for cell in row.children.iter().flatten() {
          tabulator.find_next_free_column();
          let colspan = cell.table_cell_colspan();
          let rowspan = cell.table_cell_rowspan();

          found_multispan_cells |= rowspan > 1 || colspan > 1;
          table_column_count = table_column_count.max(if is_fixed_layout {
            tabulator.current_column() + colspan
          } else {
            tabulator.current_column() + 1
          });
          if !found_multispan_cells {
            box_order += 1;
            borders.merge(
              row_index,
              tabulator.current_column(),
              rowspan,
              colspan,
              &cell.context,
              EdgeSource::Cell,
              box_order,
              None,
            );
          }
          tabulator.process_cell(colspan, rowspan);
        }
        tabulator.end_row();
        row_index += 1;
      }
      borders
        .sections
        .push((section_start_row, row_index - section_start_row));
    }

    borders.last_column_index = table_column_count;
    let table_row_count = row_index;

    // Marks cell borders again with each rowspan held to its section.
    if found_multispan_cells {
      let mut row_index = 0;

      for (section_index, (section, _)) in grouped.sections().enumerate() {
        let mut tabulator = ColspanCellTabulator::default();

        for row in children[section].children.iter().flatten() {
          tabulator.start_row();
          for cell in row.children.iter().flatten() {
            tabulator.find_next_free_column();
            box_order += 1;
            borders.merge(
              row_index,
              tabulator.current_column(),
              cell.table_cell_rowspan(),
              cell.table_cell_colspan(),
              &cell.context,
              EdgeSource::Cell,
              box_order,
              Some(section_index),
            );
            tabulator.process_cell(cell.table_cell_colspan(), cell.table_cell_rowspan());
          }
          tabulator.end_row();
          row_index += 1;
        }
      }
    }

    // Marks row borders.
    let mut row_index = 0;

    for (section, _) in grouped.sections() {
      for row in children[section].children.iter().flatten() {
        box_order += 1;
        borders.merge(
          row_index,
          0,
          1,
          table_column_count,
          &row.context,
          EdgeSource::Row,
          box_order,
          None,
        );
        row_index += 1;
      }
    }

    // Marks section borders, apart from rows, which would get their precedence wrong.
    for (section_index, (section, _)) in grouped.sections().enumerate() {
      let (start_row, row_count) = borders.sections[section_index];

      box_order += 1;
      borders.merge(
        start_row,
        0,
        row_count,
        table_column_count,
        &children[section].context,
        EdgeSource::Section,
        box_order,
        None,
      );
    }

    // Marks column borders, which take precedence over column group borders.
    box_order += 1;
    let mut columns = ColumnBorders {
      borders: &mut borders,
      table_row_count,
      box_order,
      groups: false,
    };
    visit_columns(table, grouped, table_column_count, &mut columns);
    box_order += 1;
    let mut groups = ColumnBorders {
      borders: &mut borders,
      table_row_count,
      box_order,
      groups: true,
    };
    visit_columns(table, grouped, table_column_count, &mut groups);

    // Marks table borders.
    box_order += 1;
    borders.merge(
      0,
      0,
      table_row_count,
      table_column_count,
      &table.context,
      EdgeSource::Table,
      box_order,
      None,
    );
    borders.update_table_border(table_row_count, table_column_count);

    borders
  }

  /// The table's own border: the half of each collapsed edge outside its grid, or its border as
  /// styled when it does not collapse.
  pub(crate) fn table_border(&self) -> BoxStrut {
    self.table_border
  }

  pub(crate) fn source(&self, edge: Edge) -> Option<&BorderSource> {
    self.sources.get(edge.source? as usize)
  }

  /// Blink's `TableBorders::BorderWidth` of the edge at `index`.
  pub(crate) fn border_width(&self, index: usize) -> LayoutUnit {
    self.edges.get(index).map_or(LayoutUnit::ZERO, |edge| {
      match (self.source(*edge), edge.side.index()) {
        (Some(source), Some(side)) => source.widths[side],
        _ => LayoutUnit::ZERO,
      }
    })
  }

  /// Blink's `TableBorders::BorderStyle` of the edge at `index`.
  pub(crate) fn border_style(&self, index: usize) -> BorderStyle {
    self.edges.get(index).map_or(BorderStyle::None, |edge| {
      match (self.source(*edge), edge.side.index()) {
        (Some(source), Some(side)) => source.styles[side],
        _ => BorderStyle::None,
      }
    })
  }

  /// Blink's `TableBorders::BorderColor` of the edge at `index`.
  pub(crate) fn border_color(&self, index: usize) -> Color {
    self.edges.get(index).map_or(Color::transparent(), |edge| {
      match (self.source(*edge), edge.side.index()) {
        (Some(source), Some(side)) => source.colors[side],
        _ => Color::transparent(),
      }
    })
  }

  /// Blink's `TableBorders::CanPaint`.
  pub(crate) fn can_paint(&self, index: usize) -> bool {
    self
      .edges
      .get(index)
      .is_some_and(|edge| edge.source.is_some())
      && self.border_style(index).is_rendered()
      && self.border_width(index) != LayoutUnit::ZERO
  }

  /// The border a cell lays out with: in the collapsing model half of each edge it shares, the
  /// widest along each side, as Blink's `TableBorders::CellBorder` gives it.
  pub(crate) fn cell_border(
    &self,
    cell: &RenderNode,
    row: usize,
    column: usize,
    section: usize,
  ) -> BoxStrut {
    if !self.is_collapsed {
      return BorderSource::of(&cell.context).strut();
    }

    self.cell_borders(
      row,
      column,
      self.clamp_rowspan(section, row, cell.table_cell_rowspan()),
      self.clamp_colspan(column, cell.table_cell_colspan()),
    )
  }

  fn clamp_colspan(&self, column: usize, colspan: usize) -> usize {
    colspan.min(self.last_column_index.saturating_sub(column))
  }

  fn clamp_rowspan(&self, section: usize, row: usize, rowspan: usize) -> usize {
    if rowspan <= 1 {
      return rowspan;
    }

    let Some(&(start_row, row_count)) = self.sections.get(section) else {
      return rowspan;
    };

    rowspan.min(row_count - (row - start_row))
  }

  /// Blink's `TableBorders::GetCellBorders`.
  fn cell_borders(&self, row: usize, column: usize, rowspan: usize, colspan: usize) -> BoxStrut {
    let mut strut = BoxStrut::default();

    if self.edges_per_row == 0
      || column * 2 >= self.edges_per_row
      || row >= self.edges.len() / self.edges_per_row
    {
      return strut;
    }

    let paint_width = |index: usize| {
      if self.can_paint(index) {
        self.border_width(index)
      } else {
        LayoutUnit::ZERO
      }
    };
    let first_inline_start_edge = row * self.edges_per_row + column * 2;
    let first_inline_end_edge = first_inline_start_edge + colspan * 2;

    for offset in 0..rowspan {
      let start = first_inline_start_edge + offset * self.edges_per_row;

      strut.left = strut.left.max(paint_width(start));
      if start >= self.edges.len() {
        break;
      }
      let end = first_inline_end_edge + offset * self.edges_per_row;

      strut.right = strut.right.max(paint_width(end));
    }

    let start_column_edge = column * 2 + 1;

    for offset in 0..colspan {
      let column_edge = start_column_edge + offset * 2;

      if column_edge >= self.edges_per_row {
        break;
      }
      let start = row * self.edges_per_row + column_edge;

      strut.top = strut.top.max(paint_width(start));
      strut.bottom = strut
        .bottom
        .max(paint_width(start + rowspan * self.edges_per_row));
    }

    // Two halves of an odd border lose 1/64px, which pixel snapping hides.
    let (inline_start, inline_end) = (strut.left / 2, strut.right / 2);

    BoxStrut {
      top: strut.top / 2,
      right: if self.rtl { inline_start } else { inline_end },
      bottom: strut.bottom / 2,
      left: if self.rtl { inline_end } else { inline_start },
    }
  }

  /// The physical sides a column's inline-start and inline-end edges take their borders from.
  fn inline_sides(&self) -> (EdgeSide, EdgeSide) {
    if self.rtl {
      (EdgeSide::Right, EdgeSide::Left)
    } else {
      (EdgeSide::Left, EdgeSide::Right)
    }
  }

  /// Blink's `TableBorders::UpdateTableBorder`.
  fn update_table_border(&mut self, table_row_count: usize, table_column_count: usize) {
    self.table_border = if self.edges_per_row == 0 {
      BoxStrut::default()
    } else {
      self.cell_borders(0, 0, table_row_count, table_column_count)
    };
  }

  /// Blink's `TableBorders::MergeBorders`: the borders of the box `context` styles, spanning
  /// `rowspan` rows and `colspan` columns from `start_row` and `start_column`, into every edge
  /// they are more specific than.
  #[allow(clippy::too_many_arguments)]
  fn merge(
    &mut self,
    start_row: usize,
    start_column: usize,
    rowspan: usize,
    colspan: usize,
    context: &RenderContext,
    source: EdgeSource,
    box_order: u32,
    section: Option<usize>,
  ) {
    // Empty table parts span nothing.
    if rowspan == 0 || colspan == 0 {
      return;
    }

    let colspan = self.clamp_colspan(start_column, colspan);
    let rowspan = match (source, section) {
      (EdgeSource::Cell, Some(section)) => self.clamp_rowspan(section, start_row, rowspan),
      (EdgeSource::Cell, None) => rowspan.min(MAX_ROWSPAN as usize),
      _ => rowspan,
    };
    let borders = BorderSource::of(context);
    let mark_inner_borders = source == EdgeSource::Cell && (rowspan > 1 || colspan > 1);
    let (inline_start_side, inline_end_side) = self.inline_sides();
    let has = |side: EdgeSide| {
      side
        .index()
        .is_some_and(|index| borders.styles[index] != BorderStyle::None)
    };
    let (top, bottom) = (has(EdgeSide::Top), has(EdgeSide::Bottom));
    let (inline_start, inline_end) = (has(inline_start_side), has(inline_end_side));

    if mark_inner_borders {
      self.ensure_cell_column_fits(start_column + colspan - 1);
      self.ensure_cell_row_fits(start_row + rowspan - 1);
    } else {
      if !borders.has_border() {
        return;
      }
      // Only the edges a border fills need to exist.
      self.ensure_cell_column_fits(if !inline_end && !top && !bottom {
        start_column
      } else {
        start_column + colspan - 1
      });
      self.ensure_cell_row_fits(if !inline_start && !inline_end && !bottom {
        start_row
      } else {
        start_row + rowspan - 1
      });
    }

    let source_index = self.sources.len() as u32;

    self.sources.push(borders);
    self.merge_row_axis(
      start_row,
      start_column,
      colspan,
      source_index,
      box_order,
      EdgeSide::Top,
    );
    self.merge_row_axis(
      start_row + rowspan,
      start_column,
      colspan,
      source_index,
      box_order,
      EdgeSide::Bottom,
    );
    self.merge_column_axis(
      start_row,
      start_column,
      rowspan,
      source_index,
      box_order,
      inline_start_side,
    );
    self.merge_column_axis(
      start_row,
      start_column + colspan,
      rowspan,
      source_index,
      box_order,
      inline_end_side,
    );
    if mark_inner_borders {
      self.mark_inner_borders_as_do_not_fill(start_row, start_column, rowspan, colspan);
    }
  }

  /// Blink's `IsSourceMoreSpecificThanEdge`.
  fn is_more_specific(&self, style: BorderStyle, width: LayoutUnit, edge: Edge) -> bool {
    if edge.side == EdgeSide::DoNotFill {
      return false;
    }
    let (Some(existing), Some(side)) = (self.source(edge), edge.side.index()) else {
      return true;
    };

    if style == BorderStyle::Hidden {
      return true;
    }
    if existing.styles[side] == BorderStyle::Hidden {
      return false;
    }

    let existing_width = existing.widths[side];

    if width != existing_width {
      return width > existing_width;
    }
    style_rank(style) > style_rank(existing.styles[side])
  }

  /// Blink's `MergeRowAxisBorder`: the border along `side` into the edges along row line `row`.
  fn merge_row_axis(
    &mut self,
    row: usize,
    start_column: usize,
    colspan: usize,
    source: u32,
    box_order: u32,
    side: EdgeSide,
  ) {
    let Some(index) = side.index() else {
      return;
    };
    let borders = self.sources[source as usize];
    let style = borders.styles[index];

    if style == BorderStyle::None {
      return;
    }
    let start = self.edges_per_row * row + start_column * 2 + 1;

    for edge in (start..start + colspan * 2).step_by(2) {
      if self.is_more_specific(style, borders.widths[index], self.edges[edge]) {
        self.edges[edge] = Edge {
          source: Some(source),
          side,
          box_order,
        };
      }
    }
  }

  /// Blink's `MergeColumnAxisBorder`: the border along `side` into the edges along column line
  /// `column`.
  fn merge_column_axis(
    &mut self,
    start_row: usize,
    column: usize,
    rowspan: usize,
    source: u32,
    box_order: u32,
    side: EdgeSide,
  ) {
    let Some(index) = side.index() else {
      return;
    };
    let borders = self.sources[source as usize];
    let style = borders.styles[index];

    if style == BorderStyle::None {
      return;
    }
    let start = self.edges_per_row * start_row + column * 2;

    for edge in (start..start + rowspan * self.edges_per_row).step_by(self.edges_per_row) {
      if self.is_more_specific(style, borders.widths[index], self.edges[edge]) {
        self.edges[edge] = Edge {
          source: Some(source),
          side,
          box_order,
        };
      }
    }
  }

  /// Blink's `MarkInnerBordersAsDoNotFill`: keeps the parts of a table from drawing inside a
  /// spanning cell.
  fn mark_inner_borders_as_do_not_fill(
    &mut self,
    start_row: usize,
    start_column: usize,
    rowspan: usize,
    colspan: usize,
  ) {
    let mark = |edge: &mut Edge| {
      // Two cells that overlap, which is an error, may have filled it already.
      if edge.source.is_none() {
        edge.side = EdgeSide::DoNotFill;
      }
    };
    let start = start_column * 2 + 2;
    let end = start + (colspan - 1) * 2;

    if start != end {
      for row in start_row..start_row + rowspan {
        let row_offset = row * self.edges_per_row;

        for edge in (row_offset + start..row_offset + end).step_by(2) {
          mark(&mut self.edges[edge]);
        }
      }
    }

    let start = start_column * 2 + 1;
    let end = start + colspan * 2;

    for row in start_row + 1..start_row + rowspan {
      let row_offset = row * self.edges_per_row;

      for edge in (row_offset + start..row_offset + end).step_by(2) {
        mark(&mut self.edges[edge]);
      }
    }
  }

  /// Blink's `EnsureCellColumnFits`: widens every row to hold the edges of `column`.
  fn ensure_cell_column_fits(&mut self, column: usize) {
    let desired = (column + 2) * 2;

    if desired <= self.edges_per_row {
      return;
    }

    let row_count = self
      .edges
      .len()
      .checked_div(self.edges_per_row)
      .unwrap_or(1);
    let mut edges = vec![Edge::EMPTY; row_count * desired];

    for row in 0..row_count {
      for edge in 0..self.edges_per_row {
        edges[row * desired + edge] = self.edges[row * self.edges_per_row + edge];
      }
    }
    self.edges = edges;
    self.edges_per_row = desired;
  }

  /// Blink's `EnsureCellRowFits`: adds rows of edges down to the ones below `row`.
  fn ensure_cell_row_fits(&mut self, row: usize) {
    let desired = (row + 2) * self.edges_per_row;

    if self.edges.len() < desired {
      self.edges.resize(desired, Edge::EMPTY);
    }
  }
}

/// Blink's `ColBordersMarker` and `ColgroupBordersMarker`.
struct ColumnBorders<'b> {
  borders: &'b mut TableBorders,
  table_row_count: usize,
  box_order: u32,
  groups: bool,
}

impl ColumnVisitor for ColumnBorders<'_> {
  fn visit_column(&mut self, column: &RenderNode, _path: [usize; 2], start: usize, span: usize) {
    if self.groups {
      return;
    }
    for index in start..start + span {
      self.borders.merge(
        0,
        index,
        self.table_row_count,
        1,
        &column.context,
        EdgeSource::Column,
        self.box_order,
        None,
      );
    }
  }

  fn leave_group(
    &mut self,
    group: &RenderNode,
    _index: usize,
    start: usize,
    span: usize,
    _has_children: bool,
  ) {
    if self.groups {
      self.borders.merge(
        0,
        start,
        self.table_row_count,
        span,
        &group.context,
        EdgeSource::Column,
        self.box_order,
        None,
      );
    }
  }
}
