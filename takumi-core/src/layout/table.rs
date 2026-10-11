//! Lowering `display: table` onto the grid layout algorithm.
//!
//! `gap` does not apply to a table box, so `border-spacing` drives the grid
//! gaps whether or not one was authored.
//!
//! taffy has no table algorithm. Grid gives every row a shared column track,
//! which flex cannot. Rows and row groups are dropped, so a row's background
//! and borders are copied onto its cells.
//!
//! Cell alignment mirrors Blink's `ComputeContentAlignment` in
//! `block_layout_algorithm_utils.cc`. `baseline` is naive: it uses block-start
//! borders and padding instead of a measured first baseline.
//!
//! Columns take the widths `table_columns` shares out once layout knows the
//! table's width, as fixed tracks.
//!
//! A table taller than [`MAX_GRID_ROWS`] moves its body rows into grids of
//! their own, cut where no rowspan crosses, which take the table's column
//! tracks. Rows overlap past taffy's 10,000-row `MAX_GRID_TRACKS` when a
//! rowspan crosses an edge of the body, or chains more rows than that.

use std::{mem::take, ops::Range};

use taffy::{Style, style_helpers::length};

use crate::{
  context::RenderContext,
  geometry::{AvailableSpace, Size},
  layout::{
    node::NodeKind,
    table_borders::CollapsedBorders,
    table_columns::{CellConstraint, ColspanCell, TableColumns},
    tree::{LayoutResults, NodeOrigin, RenderNode, TablePart},
  },
  sort_key::sort_by_key,
  style::{
    BorderCollapse, BorderStyle, BoxSizing, CaptionSide, ColorInput, ComputedStyle, Display,
    FlexDirection, FromCssStr, Gap, GridPlacement, GridPlacementSpan, GridTemplateComponents,
    JustifyContent, Length, LineWidth, Size as StyleSize, SizingContext, SpacePair, TableLayout,
    VerticalAlign, VerticalAlignKeyword,
  },
};

/// Blink's `kMaxColSpan` (`core/html/table_constants.h`).
const MAX_COLSPAN: u16 = 1000;

/// Blink's `kMaxRowSpan`.
const MAX_ROWSPAN: u16 = 65534;

/// The most rows a table lays out as one grid. taffy 0.14 scans every track of
/// an axis for each item's automatic minimum size, so a grid's cost grows with
/// the square of its rows. Upstream scans only the spanned tracks since
/// <https://github.com/DioxusLabs/taffy/pull/1228>, not yet released.
pub(super) const MAX_GRID_ROWS: usize = 2_000;

/// Blink table-cell content alignment from `block_layout_algorithm_utils.cc`.
#[derive(Clone, Copy, PartialEq)]
enum CellAlignment {
  Start,
  Center,
  End,
  Baseline,
}

impl CellAlignment {
  fn of(style: &ComputedStyle) -> Self {
    match style.align_content {
      JustifyContent::Normal => Self::of_vertical_align(style.vertical_align),
      JustifyContent::SpaceAround
      | JustifyContent::SpaceEvenly
      | JustifyContent::Center
      | JustifyContent::SafeCenter => Self::Center,
      JustifyContent::End
      | JustifyContent::FlexEnd
      | JustifyContent::SafeEnd
      | JustifyContent::SafeFlexEnd => Self::End,
      _ => Self::Start,
    }
  }

  fn of_vertical_align(vertical_align: VerticalAlign) -> Self {
    match vertical_align {
      VerticalAlign::Keyword(VerticalAlignKeyword::Top) => Self::Start,
      VerticalAlign::Keyword(VerticalAlignKeyword::Middle) => Self::Center,
      VerticalAlign::Keyword(VerticalAlignKeyword::Bottom) => Self::End,
      _ => Self::Baseline,
    }
  }

  fn justify_content(self) -> Option<JustifyContent> {
    match self {
      Self::Center => Some(JustifyContent::SafeCenter),
      Self::End => Some(JustifyContent::SafeFlexEnd),
      Self::Start | Self::Baseline => None,
    }
  }
}

/// The table's children sorted into their roles, rows flattened in render order: header rows first,
/// then body, then footer.
struct TableSlots {
  captions: Vec<RenderNode>,
  rows: Vec<RenderNode>,
  header_rows: usize,
  footer_rows: usize,
  strays: Vec<RenderNode>,
}

impl TableSlots {
  fn group_order(display: Display) -> u8 {
    match display {
      Display::TableHeaderGroup => 0,
      Display::TableFooterGroup => 2,
      _ => 1,
    }
  }

  /// Extracts rows without CSS anonymous table-box fixup.
  fn collect(table: &mut RenderNode) -> Self {
    let mut captions = Vec::new();
    let mut groups: Vec<(u8, usize, Vec<RenderNode>)> = Vec::new();
    let mut strays = Vec::new();

    let children = table.children.take().map_or_else(Vec::new, Vec::from);

    for (index, mut child) in children.into_iter().enumerate() {
      match child.context.style.display {
        Display::TableCaption => captions.push(child),
        Display::TableRow => groups.push((1, index, vec![child])),
        Display::TableHeaderGroup | Display::TableRowGroup | Display::TableFooterGroup => {
          let order = Self::group_order(child.context.style.display);
          let rows = child.children.take().map_or_else(Vec::new, Vec::from);

          groups.push((
            order,
            index,
            rows
              .into_iter()
              .filter(|row| row.context.style.display == Display::TableRow)
              .collect(),
          ));
        }
        _ => strays.push(child),
      }
    }

    sort_by_key(&mut groups, |(order, index, _)| (*order, *index));

    let count = |wanted: u8| {
      groups
        .iter()
        .filter(|(order, ..)| *order == wanted)
        .map(|(.., rows)| rows.len())
        .sum()
    };
    let header_rows = count(0);
    let footer_rows = count(2);
    let rows = groups.into_iter().flat_map(|(.., rows)| rows).collect();

    Self {
      captions,
      rows,
      header_rows,
      footer_rows,
      strays,
    }
  }
}

/// Every cell's column track and span, row by row, and the track count they reach.
struct TableGrid {
  placements: Vec<Vec<(usize, u16)>>,
  columns: u16,
  /// Per row, the row the furthest-reaching rowspan from it or above ends before, capped at the
  /// row count.
  reach: Vec<usize>,
}

impl TableGrid {
  /// Places every cell on its column track, honouring the spans above it.
  fn resolve(rows: &[RenderNode]) -> Self {
    let mut covered: Vec<u16> = Vec::new();
    let mut placements = Vec::with_capacity(rows.len());
    let mut reach = Vec::with_capacity(rows.len());
    let mut furthest = 0usize;

    for (index, row) in rows.iter().enumerate() {
      let mut column = 0usize;
      let mut cells = Vec::new();

      for cell in row.cells() {
        while covered.get(column).is_some_and(|rows_left| *rows_left > 0) {
          column += 1;
        }

        let colspan = cell.colspan();
        let rowspan = cell.rowspan();
        let end = column + usize::from(colspan);

        if covered.len() < end {
          covered.resize(end, 0);
        }

        for track in &mut covered[column..end] {
          *track = rowspan;
        }

        cells.push((column, colspan));
        furthest = furthest.max(index + usize::from(rowspan));
        column = end;
      }

      placements.push(cells);
      reach.push(furthest.max(index + 1).min(rows.len()));

      for track in &mut covered {
        *track = track.saturating_sub(1);
      }
    }

    let columns = placements
      .iter()
      .flatten()
      .map(|(column, colspan)| *column as u32 + u32::from(*colspan))
      .max()
      .unwrap_or(1)
      .clamp(1, u32::from(MAX_COLSPAN)) as u16;

    Self {
      placements,
      columns,
      reach,
    }
  }

  /// The `body` rows cut into runs of `rows_per_run`, each run stretched until no rowspan crosses
  /// its end. A rowspan across either end of the body leaves no runs.
  fn body_runs(&self, body: Range<usize>, rows_per_run: usize) -> Vec<Range<usize>> {
    let clear = |boundary: usize| boundary == 0 || self.reach[boundary - 1] <= boundary;

    if body.is_empty() || !clear(body.start) || !clear(body.end) {
      return Vec::new();
    }

    let mut runs = Vec::new();
    let mut start = body.start;

    while start < body.end {
      let mut end = (start + rows_per_run.max(1)).min(body.end);

      while !clear(end) {
        end += 1;
      }

      runs.push(start..end);
      start = end;
    }

    runs
  }

  /// The cells of the first `rows_taken` rows with their placements.
  fn placed_cells<'a>(
    &'a self,
    rows: &'a [RenderNode],
    rows_taken: usize,
  ) -> impl Iterator<Item = (&'a RenderNode, &'a (usize, u16))> {
    rows
      .iter()
      .take(rows_taken)
      .zip(&self.placements)
      .flat_map(|(row, cells)| row.cells().zip(cells))
  }

  /// The columns the rows constrain, or in a `fixed` table its first row only, with `spacing`
  /// between them, as Blink's `ComputeColumnConstraints` gathers them.
  fn column_constraints(&self, rows: &[RenderNode], fixed: bool, spacing: f32) -> TableColumns {
    let mut cells: Vec<Option<CellConstraint>> = vec![None; usize::from(self.columns)];
    let mut colspan_cells = Vec::new();
    let measured = if fixed { 1 } else { rows.len() };

    for (cell, (column, colspan)) in self.placed_cells(rows, measured) {
      let constraint = cell.inline_constraint(fixed);

      if *colspan > 1 {
        colspan_cells.push(ColspanCell {
          constraint,
          start: *column,
          span: usize::from(*colspan),
        });
      } else if let Some(slot) = cells.get_mut(*column) {
        match slot {
          Some(merged) => merged.encompass(constraint),
          None => *slot = Some(constraint),
        }
      }
    }

    TableColumns::new(&cells, colspan_cells, spacing, fixed)
  }
}

impl RenderNode {
  /// Lowers every table in the subtree onto grids of at most `max_rows` rows each.
  pub(super) fn lower_tables(&mut self, max_rows: usize) {
    if let Some(children) = self.children.as_mut() {
      for child in children {
        child.lower_tables(max_rows);
      }
    }

    if self.context.style.display == Display::Table {
      self.lower_table(max_rows);
    }
  }

  /// The `colspan` attribute within Blink's limit.
  pub(crate) fn colspan(&self) -> u16 {
    self.span_attribute("colspan", MAX_COLSPAN)
  }

  /// The `rowspan` attribute within Blink's limit.
  pub(crate) fn rowspan(&self) -> u16 {
    self.span_attribute("rowspan", MAX_ROWSPAN)
  }

  fn span_attribute(&self, name: &str, max: u16) -> u16 {
    self
      .node
      .as_ref()
      .and_then(|node| node.attribute(name))
      .and_then(|value| value.trim().parse::<u32>().ok())
      .map_or(1, |value| value.clamp(1, u32::from(max)) as u16)
  }

  /// The cells of this row, skipping children that are not cells.
  pub(crate) fn cells(&self) -> impl Iterator<Item = &RenderNode> {
    self
      .children
      .as_deref()
      .unwrap_or_default()
      .iter()
      .filter(|cell| cell.is_cell())
  }

  /// Wraps each run of this row's children that are not table cells in an anonymous cell, as
  /// CSS 2.2 §17.2.1 fixes a table up.
  fn wrap_anonymous_cells(&mut self) {
    let Some(children) = self.children.take() else {
      return;
    };
    let mut wrapped = Vec::with_capacity(children.len());
    let mut run = Vec::new();

    for child in children {
      if child.context.style.display != Display::TableCell && child.is_cell() {
        run.push(child);
        continue;
      }
      if !run.is_empty() {
        wrapped.push(self.anonymous_box(Display::TableCell, take(&mut run)));
      }
      wrapped.push(child);
    }
    if !run.is_empty() {
      wrapped.push(self.anonymous_box(Display::TableCell, run));
    }

    self.children = Some(wrapped.into_boxed_slice());
  }

  /// A `display` box this one generates around `children`, styled only by what it inherits.
  fn anonymous_box(&self, display: Display, children: Vec<RenderNode>) -> RenderNode {
    let mut style = ComputedStyle::from_parent(&self.context.style);

    style.display = display;
    style.make_computed(&self.context.sizing);

    RenderNode::new(
      RenderContext::from_parent(
        &self.context,
        style,
        self.context.sizing.clone(),
        self.context.current_color,
      ),
      NodeOrigin::Anonymous,
      None,
      Some(children.into_boxed_slice()),
    )
  }

  /// Recognizes authored cells without CSS anonymous table-box fixup.
  fn is_cell(&self) -> bool {
    let display = self.context.style.display;

    if display == Display::TableCell {
      return true;
    }

    display != Display::None
      && matches!(self.origin, NodeOrigin::Authored { .. })
      && self
        .node
        .as_ref()
        .is_some_and(|node| !matches!(node.kind, NodeKind::Text(_)))
  }

  fn lower_table(&mut self, max_rows: usize) {
    let TableSlots {
      captions,
      mut rows,
      header_rows,
      footer_rows,
      strays,
    } = TableSlots::collect(self);

    for row in &mut rows {
      row.wrap_anonymous_cells();
    }

    let grid = TableGrid::resolve(&rows);
    let columns = grid.columns;
    let collapse = self.context.style.border_collapse == BorderCollapse::Collapse;
    let spacing = self.context.style.border_spacing.0;
    let sizing = self.context.sizing.clone();
    let fixed =
      self.context.style.table_layout == TableLayout::Fixed && !self.context.style.width.is_auto();
    let collapsed = collapse.then(|| {
      CollapsedBorders::resolve(
        &self.context.style,
        &rows,
        &grid.placements,
        usize::from(columns),
      )
    });

    if let Some(collapsed) = collapsed.as_ref() {
      for (index, row) in rows.iter_mut().enumerate() {
        for (cell_index, cell) in row
          .children
          .as_deref_mut()
          .unwrap_or_default()
          .iter_mut()
          .filter(|cell| cell.is_cell())
          .enumerate()
        {
          collapsed.apply(index, cell_index, &mut cell.context.style);
        }
      }
    }

    let spacing_px = if collapse {
      0.0
    } else {
      spacing.x.to_px(&sizing, 0.0)
    };
    let table_columns = grid.column_constraints(&rows, fixed, spacing_px);
    let footer_start = rows.len() - footer_rows;
    let lines = captions.len() + rows.len() + strays.len();
    let mut body_runs = if lines > max_rows {
      grid.body_runs(header_rows..footer_start, max_rows / 2)
    } else {
      Vec::new()
    }
    .into_iter()
    .peekable();
    let placements = grid.placements;
    let mut items = Vec::new();
    let mut line: i16 = 1;
    let (top_captions, bottom_captions): (Vec<_>, Vec<_>) = captions
      .into_iter()
      .partition(|caption| caption.context.style.caption_side == CaptionSide::Top);

    let tracks = table_columns.tracks() as u16;
    let gaps = if collapse {
      SpacePair::from_single(Length::zero())
    } else {
      spacing
    };

    for mut caption in top_captions {
      caption.lower_full_width(line, tracks);
      caption.table_part = Some(TablePart::Caption);
      items.push(caption);
      line = line.saturating_add(1);
    }

    if header_rows > 0 && header_rows < rows.len() {
      let start = line;

      self.table_header_lines = Some((start, start.saturating_add(header_rows as i16)));
    }

    let mut run: Option<(Range<usize>, Vec<RenderNode>)> = None;

    for (index, (row, positions)) in rows.into_iter().zip(placements).enumerate() {
      let part = if index < header_rows {
        TablePart::HeaderCell
      } else if index >= footer_start {
        TablePart::FooterCell
      } else {
        TablePart::BodyCell
      };

      if body_runs.peek().is_some_and(|next| next.start == index) {
        run = body_runs.next().map(|range| (range, Vec::new()));
      }

      let Some((range, cells)) = run.as_mut() else {
        row.lower_row(line, part, positions, &table_columns, collapse, &mut items);
        line = line.saturating_add(1);
        continue;
      };

      row.lower_row(
        (index - range.start + 1) as i16,
        part,
        positions,
        &table_columns,
        collapse,
        cells,
      );

      if index + 1 == range.end
        && let Some((_, cells)) = run.take()
      {
        items.push(self.body_rows(cells, line, tracks, gaps));
        line = line.saturating_add(1);
      }
    }

    for mut stray in strays {
      stray.lower_full_width(line, tracks);
      items.push(stray);
      line = line.saturating_add(1);
    }

    for mut caption in bottom_captions {
      caption.lower_full_width(line, tracks);
      caption.table_part = Some(TablePart::Caption);
      items.push(caption);
      line = line.saturating_add(1);
    }

    self.table_part = Some(TablePart::Table);

    let style = &mut self.context.style;

    style.set_table_grid(tracks, gaps);

    if collapse {
      style.clear_border();
    } else {
      style.inset_table_edges(spacing, &sizing);
    }

    self.children = Some(items.into_boxed_slice());
    self.table_columns = Some(Box::new(table_columns));
  }

  /// Lowers this row's cells onto grid `line`, appending them to `items`.
  fn lower_row(
    mut self,
    line: i16,
    part: TablePart,
    positions: Vec<(usize, u16)>,
    columns: &TableColumns,
    collapse: bool,
    items: &mut Vec<RenderNode>,
  ) {
    let mut cells = self.children.take().map_or_else(Vec::new, Vec::from);
    let mut positions = positions.into_iter();

    cells.retain(RenderNode::is_cell);

    wrap_row_baselines(&mut cells);

    for mut cell in cells {
      let Some((column, colspan)) = positions.next() else {
        break;
      };

      cell.inherit_row_background(&self);
      cell.lower_cell(
        line,
        columns.track(column),
        columns.track_span(column, usize::from(colspan)) as u16,
        collapse,
      );
      cell.table_part = Some(part);
      items.push(cell);
    }
  }

  /// A grid of its own on table `line` for a run of body rows' `cells`, which layout gives the
  /// table's column tracks.
  fn body_rows(
    &self,
    cells: Vec<RenderNode>,
    line: i16,
    tracks: u16,
    gaps: SpacePair<Length>,
  ) -> RenderNode {
    let mut rows = self.anonymous_box(Display::Grid, cells);

    rows.context.style.set_table_grid(tracks, gaps);
    rows.lower_full_width(line, tracks);
    rows.table_part = Some(TablePart::BodyRows);
    rows
  }

  /// Places the self explicitly: taffy's cursor does not return to the row start
  /// on a row a `rowspan` reaches into.
  fn lower_cell(&mut self, line: i16, column: usize, colspan: u16, collapse: bool) {
    let rowspan = self.rowspan();
    let style = &mut self.context.style;

    // A cell fills its columns; its widths only constrained them.
    style.width = Default::default();
    style.min_width = Default::default();
    style.max_width = Default::default();
    self.context.collapsed_borders = collapse;

    if self.context.style.display == Display::TableCell {
      self.align_cell_content();

      if self.context.style.display == Display::TableCell {
        self.context.style.display = Display::Block;
      }
    }

    self.context.style.grid_row_start = GridPlacement::Line(line);
    self.context.style.grid_row_end = GridPlacement::Span(GridPlacementSpan::Span(rowspan));
    self.context.style.grid_column_start = GridPlacement::Line(column as i16 + 1);
    self.context.style.grid_column_end = GridPlacement::Span(GridPlacementSpan::Span(colspan));
  }

  fn lower_full_width(&mut self, line: i16, columns: u16) {
    // A stray that is itself a lowered table keeps its grid; blocking it would
    // drop the placement its own cells already carry.
    if self.context.style.display != Display::Grid {
      self.context.style.display = Display::Block;
    }

    self.context.style.grid_row_start = GridPlacement::Line(line);
    self.context.style.grid_row_end = GridPlacement::Span(GridPlacementSpan::Span(1));
    self.context.style.grid_column_start = GridPlacement::Line(1);
    self.context.style.grid_column_end = GridPlacement::Span(GridPlacementSpan::Span(columns));
  }

  /// Approximation: row backgrounds leave `border-spacing` gaps unpainted.
  fn inherit_row_background(&mut self, row: &RenderNode) {
    let row_style = &row.context.style;

    if row_style.background_color == ColorInput::transparent()
      && row_style.background_image.is_none()
    {
      return;
    }

    let cell_style = &mut self.context.style;

    if cell_style.background_color != ColorInput::transparent()
      || cell_style.background_image.is_some()
    {
      return;
    }

    cell_style.background_color = row_style.background_color;
    cell_style.background_image = row_style.background_image.clone();
    cell_style.background_position = row_style.background_position.clone();
    cell_style.background_size = row_style.background_size.clone();
    cell_style.background_repeat = row_style.background_repeat.clone();
    cell_style.background_clip = row_style.background_clip;
    cell_style.background_origin = row_style.background_origin;
  }

  /// Wraps content to preserve its formatting context during alignment.
  fn wrap_cell_content(&mut self) -> Option<&mut RenderNode> {
    let children = match self.children.take() {
      Some(children) => children.into_vec(),
      // A cell holding only text folded it into itself; it goes back into a child to align.
      None => vec![RenderNode::generated_sibling_text(
        &self.context,
        self.node.as_mut()?.take_text()?,
      )],
    };
    let content = RenderNode::anonymous_block_container(&self.context, children);

    self.children = Some(Box::new([content]));
    self.children.as_deref_mut()?.first_mut()
  }

  fn align_cell_content(&mut self) {
    let Some(justify) = CellAlignment::of(&self.context.style).justify_content() else {
      return;
    };

    if self.wrap_cell_content().is_none() {
      return;
    }

    let style = &mut self.context.style;

    style.display = Display::Flex;
    style.flex_direction = FlexDirection::Column;
    style.justify_content = justify;
  }

  /// Whether this lowered cell aligns its content to its row's baseline: a baseline-aligned cell
  /// whose content a row of several such cells wrapped for layout to move.
  pub(super) fn aligns_to_row_baseline(&self) -> bool {
    self.table_part.is_some_and(TablePart::is_cell)
      && CellAlignment::of(&self.context.style) == CellAlignment::Baseline
      && matches!(
        self.children.as_deref(),
        Some([content]) if content.origin == NodeOrigin::Anonymous
      )
  }

  /// Widths a cell subtree takes when nothing constrains it and when everything
  /// does. The cell is lowered first: `table-cell` establishes no formatting
  /// context of its own, so taffy would lay its inline children out as blocks.
  /// Nested tables pay for this once per level, since each level lays the levels
  /// below it out again.
  fn intrinsic_widths(&self) -> (f32, f32) {
    let mut cell = self.clone();
    let style = &mut cell.context.style;

    // Blink's `MinMaxSizes` of the cell's content; its own widths constrain the column apart.
    style.width = Default::default();
    style.min_width = Default::default();
    style.max_width = Default::default();
    cell.lower_cell(1, 0, 1, false);
    cell.context.style.display.blockify();

    let measure = |width| {
      LayoutResults::compute(
        &cell,
        Size {
          width,
          height: AvailableSpace::MaxContent,
        },
      )
      .root_size()
      .width
    };

    (
      measure(AvailableSpace::MinContent),
      measure(AvailableSpace::MaxContent),
    )
  }
}

impl RenderNode {
  /// What this cell asks of its columns, Blink's `CreateCellInlineConstraint`; a `fixed` table
  /// gives it no minimum.
  fn inline_constraint(&self, fixed: bool) -> CellConstraint {
    let style = &self.context.style;
    let sizing = &self.context.sizing;
    let border = |style_rendered: bool, width: LineWidth| {
      if style_rendered {
        Length::from(width).to_px(sizing, 0.0)
      } else {
        0.0
      }
    };
    let padding = |length: Length| match length {
      Length::Percentage(_) | Length::Auto => 0.0,
      length => length.to_px(sizing, 0.0),
    };
    let border_padding = border(
      style.border_left_style.is_rendered(),
      style.border_left_width,
    ) + border(
      style.border_right_style.is_rendered(),
      style.border_right_width,
    ) + padding(style.padding_left)
      + padding(style.padding_right);
    let content_box = style.box_sizing == BoxSizing::ContentBox;
    let fixed_px = |length: Option<Length>| match length? {
      Length::Auto | Length::Percentage(_) => None,
      length if content_box => Some(length.to_px(sizing, 0.0) + border_padding),
      length => Some(length.to_px(sizing, 0.0).max(border_padding)),
    };
    let width = fixed_px(style.width.as_length());
    let min_width = fixed_px(Some(style.min_width));
    let max_width =
      fixed_px(style.max_width.as_length()).map(|max| max.max(min_width.unwrap_or(max)));
    let percent = match style.width.as_length() {
      Some(Length::Percentage(percent)) => Some(match style.max_width.as_length() {
        Some(Length::Percentage(max)) => percent.min(max),
        _ => percent,
      }),
      _ => None,
    };
    // A fixed table sizes its columns from declared widths only, never from content.
    let (content_min, content_max) = if fixed {
      (0.0, 0.0)
    } else {
      self.intrinsic_widths()
    };
    let mut min = if fixed {
      0.0
    } else {
      content_min.max(min_width.unwrap_or_default())
    };
    let mut max = width.unwrap_or(content_max);

    if let Some(max_width) = max_width {
      max = max.min(max_width);
      min = min.min(max_width);
    }

    CellConstraint {
      min,
      max: max.max(min),
      percent,
      constrained: width.is_some(),
    }
  }

  /// Whether this is a lowered table whose `width` is `auto`, which fits its content rather than
  /// its container.
  pub(super) fn shrinks_to_fit_as_table(&self) -> bool {
    self.table_columns.is_some()
      && self
        .context
        .style
        .width
        .as_length()
        .is_none_or(|width| width == Length::Auto)
  }

  /// Sizes a lowered table, and its columns, for `available` width across, as Blink's
  /// `TableLayoutAlgorithm::ComputeTableInlineSize` does. A flex or grid `item` takes the `known`
  /// width its container stretches it to; a block-level table shrinks to fit whatever width its
  /// container would stretch it to.
  pub(super) fn size_table(
    &self,
    style: &mut Style,
    available: AvailableSpace,
    known: Option<f32>,
    item: bool,
    sizing: &SizingContext,
  ) {
    let Some(columns) = self.table_columns.as_deref() else {
      return;
    };
    let table = &self.context.style;
    let basis = available.into_option();
    let px = |length: Length| match length {
      Length::Auto => 0.0,
      length => length.to_px(sizing, basis.unwrap_or_default()),
    };
    let rendered = |style_rendered: bool, width: LineWidth| {
      if style_rendered {
        px(Length::from(width))
      } else {
        0.0
      }
    };
    // The padding already holds the border spacing around the grid's edges.
    let edges = rendered(
      table.border_left_style.is_rendered(),
      table.border_left_width,
    ) + rendered(
      table.border_right_style.is_rendered(),
      table.border_right_width,
    ) + px(table.padding_left)
      + px(table.padding_right);
    let undistributable = columns.undistributable(edges);
    let (grid_min, grid_max) = columns.min_max(undistributable);
    let content_box = table.box_sizing == BoxSizing::ContentBox;
    // A border-box width, as the style's own box sizing counts it.
    let styled = |border_box: f32| {
      if content_box {
        border_box - edges
      } else {
        border_box
      }
    };
    let specified = |length: Option<Length>| match length? {
      Length::Auto => None,
      // A percentage of an indefinite width behaves as `auto`.
      Length::Percentage(_) if basis.is_none() => None,
      length if content_box => Some(px(length) + edges - columns.edge_spacing()),
      length => Some(px(length)),
    };
    let min_width = specified(Some(table.min_width)).map_or(grid_min, |min| min.max(grid_min));
    // `auto` and the sizing keywords size the table from its grid, which the grid's minimum and
    // maximum stand in for as the table's content sizes, as Blink's
    // `ComputeUsedInlineSizeForTableFragment` hands them to the inline-size resolution.
    let sized_by_grid = !matches!(table.width, StyleSize::Length(length) if length != Length::Auto);
    let width = match known {
      Some(known) if item || !sized_by_grid => known,
      _ => {
        let stretch = match available {
          AvailableSpace::Definite(space) => {
            Some(space - px(table.margin_left) - px(table.margin_right))
          }
          AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
        };
        let fit = match available {
          AvailableSpace::MinContent => grid_min,
          AvailableSpace::MaxContent => grid_max,
          AvailableSpace::Definite(_) => {
            stretch.map_or(grid_max, |stretch| stretch.clamp(grid_min, grid_max))
          }
        };
        let width = match table.width {
          StyleSize::MinContent => grid_min,
          StyleSize::MaxContent => grid_max,
          StyleSize::Stretch => stretch.unwrap_or(fit),
          StyleSize::FitContent => fit,
          StyleSize::Length(length) => specified(Some(length)).unwrap_or(fit),
        };
        let width = specified(table.max_width.as_length()).map_or(width, |max| width.min(max));

        width.max(min_width)
      }
    };

    // A block-level table shrinks to fit rather than stretching.
    if sized_by_grid && !item {
      style.size.width = length(styled(width));
    }
    style.min_size.width = length(styled(min_width));
    style.grid_template_columns = columns
      .track_widths((width - undistributable).max(0.0))
      .into_iter()
      .map(length)
      .collect();
  }
}

/// Wraps the content of a row's baseline-aligned cells, when it has several, for layout to move
/// each down to the row's baseline.
fn wrap_row_baselines(cells: &mut [RenderNode]) {
  let baseline =
    |cell: &RenderNode| CellAlignment::of(&cell.context.style) == CellAlignment::Baseline;

  if cells.iter().filter(|cell| baseline(cell)).count() < 2 {
    return;
  }

  for cell in cells.iter_mut().filter(|cell| baseline(cell)) {
    cell.wrap_cell_content();
  }
}

impl ComputedStyle {
  /// A grid of `tracks` `auto` columns, which layout fixes once the table's width is known, spaced
  /// by `gaps`.
  fn set_table_grid(&mut self, tracks: u16, gaps: SpacePair<Length>) {
    self.display = Display::Grid;
    self.grid_template_columns =
      GridTemplateComponents::from_css_str(&vec!["auto"; usize::from(tracks)].join(" ")).ok();
    self.column_gap = Gap::Length(gaps.x);
    self.row_gap = Gap::Length(gaps.y);
  }

  /// CSS 2.2 §17.6.1: separate borders space the outer cells from the table's
  /// edges too, which the grid gap alone does not do. Naive: a percentage or
  /// `auto` padding keeps its own value and takes no inset.
  fn inset_table_edges(&mut self, spacing: SpacePair<Length>, sizing: &SizingContext) {
    let inset = |padding: &mut Length, extra: Length| {
      if matches!(padding, Length::Percentage(_) | Length::Auto) {
        return;
      }

      *padding = Length::Px(padding.to_px(sizing, 0.0) + extra.to_px(sizing, 0.0));
    };

    inset(&mut self.padding_top, spacing.y);
    inset(&mut self.padding_bottom, spacing.y);
    inset(&mut self.padding_left, spacing.x);
    inset(&mut self.padding_right, spacing.x);
  }

  /// The collapsed border lives on the edge cells, so the table box stops painting its own.
  fn clear_border(&mut self) {
    self.border_top_style = BorderStyle::None;
    self.border_right_style = BorderStyle::None;
    self.border_bottom_style = BorderStyle::None;
    self.border_left_style = BorderStyle::None;
    self.border_top_width = LineWidth::Length(Length::zero());
    self.border_right_width = LineWidth::Length(Length::zero());
    self.border_bottom_width = LineWidth::Length(Length::zero());
    self.border_left_width = LineWidth::Length(Length::zero());
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::MAX_GRID_ROWS;
  use crate::{
    context::RenderContext,
    geometry::{AvailableSpace, NodeId, Point, Size},
    layout::{
      node::Node,
      tree::{LayoutResults, NodeOrigin, RenderNode, TablePart},
    },
    resources::font::Fonts,
    style::{
      BorderStyle, Color, ColorInput, Display, FlexDirection, Gap, GridPlacement,
      GridPlacementSpan, JustifyContent, Length, SizingContext, Style, StyleDeclaration,
      StyleSheet, ToCss,
    },
    viewport::Viewport,
  };

  /// Lowers a tree whose displays come from a stylesheet, standing in for the element presets the
  /// HTML and JSX front ends apply.
  fn lower(root: Node) -> RenderNode {
    lower_within(root, MAX_GRID_ROWS)
  }

  /// [`lower`], onto grids of at most `max_rows` rows.
  fn lower_within(root: Node, max_rows: usize) -> RenderNode {
    let stylesheet = StyleSheet::parse(
      r"
        .table { display: table }
        .thead { display: table-header-group }
        .tbody { display: table-row-group }
        .tfoot { display: table-footer-group }
        .tr { display: table-row }
        .td { display: table-cell }
        .caption { display: table-caption }
        .caption-bottom { display: table-caption; caption-side: bottom }
        .middle { display: table-cell; vertical-align: middle }
        .align-content-end { align-content: end }
        .padded { display: table-cell; padding-top: 10px }
        .flex { display: flex; vertical-align: middle }
        .pseudo-row::before { content: 'x'; display: block }
        .collapse { display: table; border-collapse: collapse }
        .bordered { display: table-cell; border: 1px solid rgb(0, 0, 0) }
        .heavy-bottom { display: table-cell; border: 1px solid rgb(0, 0, 0); border-bottom-width: 3px }
        .hidden-right { display: table-cell; border: 1px solid rgb(0, 0, 0); border-right-style: hidden }
        .marked-row { display: table-row; border-top: 2px solid rgb(255, 0, 0) }
        .red-border { display: table-cell; border: 1px solid rgb(255, 0, 0) }
        .blue-border { display: table-cell; border: 1px solid rgb(0, 0, 255) }
        .heavy-row { display: table-row; border-bottom: 3px solid rgb(0, 0, 0) }
        .fixed { display: table; table-layout: fixed; width: 300px }
        .fixed-auto { display: table; table-layout: fixed }
        .spaced { display: table; border-spacing: 4px 8px }
        .tight { display: table; border-spacing: 0 }
        .gapped { display: table; border-spacing: 4px 8px; column-gap: 0; row-gap: 0 }
        .w80 { display: table-cell; width: 80px }
        .plain { display: table }
        .outset-cell { display: table-cell; border: 2px outset rgb(0, 0, 0) }
        .inset-cell { display: table-cell; border: 2px inset rgb(0, 0, 0) }
        .heavy-under { display: table-cell; border: 1px solid rgb(0, 0, 0); border-bottom-width: 4px }
        .short { display: table-cell; height: 10px; padding: 2px }
        .tall { display: table-cell; height: 23px }
      ",
    )
    .expect("stylesheet parses");
    let fonts = Fonts::default();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .stylesheet(Arc::new(stylesheet))
      .build();

    RenderNode::from_node_within(&context, root, max_rows)
  }

  /// The width each column of the lowered `table` takes when `assignable` is shared out.
  fn column_widths(table: &RenderNode, assignable: f32) -> Vec<f32> {
    table
      .table_columns
      .as_deref()
      .expect("the table's columns")
      .widths(assignable)
  }

  fn cell(id: &str) -> Node {
    Node::container([Node::text(id)])
      .with_class_name("td")
      .with_id(id)
  }

  fn row(cells: impl IntoIterator<Item = Node>) -> Node {
    named_row("row", cells)
  }

  /// A row keeps its own box in the grid, so tests name it to tell it apart.
  fn named_row(id: &str, cells: impl IntoIterator<Item = Node>) -> Node {
    Node::container(cells.into_iter().collect::<Vec<_>>())
      .with_class_name("tr")
      .with_id(id)
  }

  fn bordered_cell(id: &str, class_name: &str) -> Node {
    Node::container([Node::text(id)])
      .with_class_name(class_name)
      .with_id(id)
  }

  /// The resolved border widths of one lowered cell, clockwise from the top.
  fn borders(table: &RenderNode, id: &str) -> [f32; 4] {
    let cell = table
      .children
      .as_deref()
      .unwrap_or_default()
      .iter()
      .find(|child| {
        child
          .node
          .as_ref()
          .and_then(|node| node.metadata.id.as_deref())
          == Some(id)
      })
      .expect("lowered cell");
    let style = &cell.context.style;
    let sizing = &cell.context.sizing;
    let width = |line_width, border_style: BorderStyle| {
      if border_style.is_rendered() {
        Length::from(line_width).to_px(sizing, 0.0)
      } else {
        0.0
      }
    };

    [
      width(style.border_top_width, style.border_top_style),
      width(style.border_right_width, style.border_right_style),
      width(style.border_bottom_width, style.border_bottom_style),
      width(style.border_left_width, style.border_left_style),
    ]
  }

  /// The ids of the grid's items, in the order they will be auto-placed.
  fn ids(node: &RenderNode) -> Vec<String> {
    node
      .children
      .as_deref()
      .unwrap_or_default()
      .iter()
      .map(|child| {
        child
          .node
          .as_ref()
          .and_then(|node| node.metadata.id.as_deref())
          .unwrap_or_default()
          .to_owned()
      })
      .collect()
  }

  #[test]
  fn rows_and_row_groups_flatten_into_one_grid() {
    let tree = lower(
      Node::container([Node::container([row([cell("a"), cell("b")])]).with_class_name("tbody")])
        .with_class_name("table"),
    );

    assert_eq!(tree.context.style.display, Display::Grid);
    assert_eq!(ids(&tree), ["a", "b"]);
  }

  #[test]
  fn header_group_renders_first_and_footer_group_last() {
    let tree = lower(
      Node::container([
        Node::container([named_row("r-foot", [cell("foot")])]).with_class_name("tfoot"),
        Node::container([named_row("r-body", [cell("body")])]).with_class_name("tbody"),
        Node::container([named_row("r-head", [cell("head")])]).with_class_name("thead"),
      ])
      .with_class_name("table"),
    );

    assert_eq!(ids(&tree), ["head", "body", "foot"]);
  }

  #[test]
  fn caption_leads_the_grid_and_spans_every_column() {
    let tree = lower(
      Node::container([
        Node::container([Node::text("cap")])
          .with_class_name("caption")
          .with_id("cap"),
        row([cell("a"), cell("b"), cell("c")]),
      ])
      .with_class_name("table"),
    );

    assert_eq!(ids(&tree), ["cap", "a", "b", "c"]);

    let caption = &tree.children.as_deref().expect("children")[0];

    assert_eq!(
      caption.context.style.grid_column_start,
      GridPlacement::Line(1)
    );
    assert_eq!(
      caption.context.style.grid_column_end,
      GridPlacement::Span(GridPlacementSpan::Span(3))
    );
  }

  #[test]
  fn a_bottom_caption_trails_the_grid() {
    let tree = lower(
      Node::container([
        Node::container([Node::text("cap")])
          .with_class_name("caption-bottom")
          .with_id("cap"),
        row([cell("a"), cell("b")]),
      ])
      .with_class_name("table"),
    );

    assert_eq!(ids(&tree), ["a", "b", "cap"]);
  }

  #[test]
  fn a_middle_cell_centers_its_content_in_a_flex_column() {
    let tree = lower(
      Node::container([row([
        Node::container([Node::text("middle")])
          .with_class_name("middle")
          .with_id("middle"),
        cell("tall"),
      ])])
      .with_class_name("table"),
    );

    let cell = &tree.children.as_deref().expect("children")[0];

    assert_eq!(cell.context.style.display, Display::Flex);
    assert_eq!(cell.context.style.flex_direction, FlexDirection::Column);
    assert_eq!(
      cell.context.style.justify_content,
      JustifyContent::SafeCenter
    );
    assert_eq!(cell.children.as_deref().expect("wrapped content").len(), 1);
  }

  #[test]
  fn align_content_outranks_vertical_align_on_a_cell() {
    let tree = lower(
      Node::container([row([
        Node::container([Node::text("cell")])
          .with_class_name("middle align-content-end")
          .with_id("cell"),
        cell("tall"),
      ])])
      .with_class_name("table"),
    );

    let cell = &tree.children.as_deref().expect("children")[0];

    assert_eq!(
      cell.context.style.justify_content,
      JustifyContent::SafeFlexEnd
    );
  }

  #[test]
  fn a_cell_that_is_not_a_table_cell_keeps_its_display_and_its_track() {
    let tree = lower(
      Node::container([row([
        Node::container([Node::text("flex")])
          .with_class_name("flex")
          .with_id("flex"),
        cell("b"),
      ])])
      .with_class_name("table"),
    );

    // An anonymous cell wraps the flex box, which keeps its display inside it.
    assert_eq!(ids(&tree), ["", "b"]);

    fn find<'n>(node: &'n RenderNode, id: &str) -> Option<&'n RenderNode> {
      if node
        .node
        .as_ref()
        .and_then(|node| node.metadata.id.as_deref())
        == Some(id)
      {
        return Some(node);
      }

      node
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find_map(|child| find(child, id))
    }

    let cell = &tree.children.as_deref().expect("children")[0];
    let flex = find(cell, "flex").expect("the flex box inside the anonymous cell");

    assert_eq!(cell.origin, NodeOrigin::Anonymous);
    assert_eq!(cell.context.style.grid_column_start, GridPlacement::Line(1));
    assert_eq!(flex.context.style.display, Display::Flex);
    assert_eq!(flex.context.style.flex_direction, FlexDirection::Row);
  }

  #[test]
  fn a_rows_generated_box_takes_no_track() {
    let tree = lower(
      Node::container([named_row("row", [cell("a"), cell("b")]).with_class_name("tr pseudo-row")])
        .with_class_name("table"),
    );

    assert_eq!(ids(&tree), ["a", "b"]);

    let cells = tree.children.as_deref().expect("children");

    assert_eq!(
      cells[0].context.style.grid_column_start,
      GridPlacement::Line(1)
    );
    assert_eq!(
      cells[1].context.style.grid_column_start,
      GridPlacement::Line(2)
    );
  }

  fn with_span(node: Node, name: &str, value: &str) -> Node {
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert(name.into(), value.into());

    node.with_attributes(attributes)
  }

  #[test]
  fn jsx_camel_case_span_counts_the_same_as_the_html_attribute() {
    for name in ["colSpan", "colspan"] {
      // A second row starts a cell in each column, so neither merges away.
      let tree = lower(
        Node::container([
          row([with_span(cell("wide"), name, "2"), cell("c")]),
          row([cell("x"), cell("y"), cell("z")]),
        ])
        .with_class_name("table"),
      );
      let wide = &tree.children.as_deref().expect("children")[0];

      assert_eq!(
        wide.context.style.grid_column_end,
        GridPlacement::Span(GridPlacementSpan::Span(2)),
        "{name} should be read as a column span"
      );
    }
  }

  #[test]
  fn oversized_spans_clamp_instead_of_overflowing_the_track_count() {
    let tree = lower(
      Node::container([row([
        with_span(cell("a"), "colspan", "65535"),
        with_span(cell("b"), "colspan", "65535"),
      ])])
      .with_class_name("table"),
    );

    // Blink's kMaxColSpan keeps the sum from wrapping; the columns only the spans cover merge
    // away, leaving each cell one track.
    let cells = tree.children.as_deref().expect("children");

    assert_eq!(
      cells[0].context.style.grid_column_end,
      GridPlacement::Span(GridPlacementSpan::Span(1))
    );
    assert_eq!(
      cells[1].context.style.grid_column_start,
      GridPlacement::Line(2)
    );
  }

  #[test]
  fn a_cells_declared_width_sizes_its_column() {
    let stylesheet_width = Node::container([Node::text("w")])
      .with_class_name("td")
      .with_id("w")
      .with_style(Style::default().with(StyleDeclaration::width(Length::Px(220.0))));
    let tree =
      lower(Node::container([row([stylesheet_width, cell("b")])]).with_class_name("table"));
    let widths = column_widths(&tree, 400.0);

    assert_eq!(widths[0], 220.0);
    assert_eq!(widths[1], 180.0);
  }

  #[test]
  fn a_rows_background_lands_on_its_cells() {
    let striped = row([cell("a"), cell("b")]).with_style(Style::default().with(
      StyleDeclaration::background_color(ColorInput::Value(Color::black())),
    ));
    let tree = lower(Node::container([striped]).with_class_name("table"));
    let cells = tree.children.as_deref().expect("children");

    assert_ne!(
      cells[0].context.style.background_color,
      ColorInput::transparent()
    );
    assert_eq!(
      cells[0].context.style.background_color,
      cells[1].context.style.background_color
    );
  }

  #[test]
  fn cells_after_a_rowspan_land_past_the_covered_track() {
    // Row two's cells sit in tracks 2 and 3; the rowspan holds track 1.
    let tree = lower(
      Node::container([
        row([with_span(cell("a"), "rowspan", "2"), cell("b")]),
        row([cell("c"), cell("d")]),
      ])
      .with_class_name("table"),
    );

    assert_eq!(
      tree
        .context
        .style
        .grid_template_columns
        .as_ref()
        .expect("template")
        .to_css_string(),
      "auto auto auto"
    );
  }

  #[test]
  fn a_width_declared_under_a_rowspan_sizes_the_shifted_column() {
    let shifted = Node::container([Node::text("w")])
      .with_class_name("td")
      .with_id("w")
      .with_style(Style::default().with(StyleDeclaration::width(Length::Px(220.0))));
    let tree = lower(
      Node::container([row([with_span(cell("a"), "rowspan", "2")]), row([shifted])])
        .with_class_name("table"),
    );

    assert_eq!(column_widths(&tree, 400.0)[1], 220.0);
  }

  #[test]
  fn colspan_becomes_a_grid_span() {
    let spanning = with_span(cell("wide"), "colspan", "2");
    let tree = lower(
      Node::container([
        row([spanning, cell("c")]),
        row([cell("a"), cell("b"), cell("c")]),
      ])
      .with_class_name("table"),
    );

    let wide = &tree.children.as_deref().expect("children")[0];

    assert_eq!(
      wide.context.style.grid_column_end,
      GridPlacement::Span(GridPlacementSpan::Span(2))
    );
  }

  #[test]
  fn a_shared_line_carries_one_border_instead_of_two() {
    let table = lower(
      Node::container([
        named_row(
          "top",
          [
            bordered_cell("a", "bordered"),
            bordered_cell("b", "bordered"),
          ],
        ),
        named_row(
          "bottom",
          [
            bordered_cell("c", "bordered"),
            bordered_cell("d", "bordered"),
          ],
        ),
      ])
      .with_class_name("collapse"),
    );

    assert_eq!(borders(&table, "a"), [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(borders(&table, "b"), [1.0, 1.0, 0.0, 1.0]);
    assert_eq!(borders(&table, "c"), [1.0, 0.0, 1.0, 1.0]);
    assert_eq!(borders(&table, "d"), [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(table.context.style.column_gap, Gap::Length(Length::zero()));
  }

  #[test]
  fn the_wider_border_wins_the_shared_line() {
    let table = lower(
      Node::container([
        named_row("top", [bordered_cell("a", "heavy-bottom")]),
        named_row("bottom", [bordered_cell("b", "bordered")]),
      ])
      .with_class_name("collapse"),
    );

    assert_eq!(borders(&table, "b")[0], 3.0);
  }

  #[test]
  fn a_hidden_border_clears_the_shared_line() {
    let table = lower(
      Node::container([named_row(
        "only",
        [
          bordered_cell("a", "hidden-right"),
          bordered_cell("b", "bordered"),
        ],
      )])
      .with_class_name("collapse"),
    );

    assert_eq!(borders(&table, "b")[3], 0.0);
  }

  #[test]
  fn a_row_border_lands_on_its_cells() {
    let table = lower(
      Node::container([
        named_row("top", [bordered_cell("a", "bordered")]),
        Node::container([bordered_cell("b", "bordered")])
          .with_class_name("marked-row")
          .with_id("marked"),
      ])
      .with_class_name("collapse"),
    );

    assert_eq!(borders(&table, "b")[0], 2.0);
  }

  #[test]
  fn an_equal_line_takes_the_colour_of_the_cell_above() {
    let table = lower(
      Node::container([
        named_row("top", [bordered_cell("a", "red-border")]),
        named_row("bottom", [bordered_cell("b", "blue-border")]),
      ])
      .with_class_name("collapse"),
    );
    let cell = table
      .children
      .as_deref()
      .unwrap_or_default()
      .iter()
      .find(|child| {
        child
          .node
          .as_ref()
          .and_then(|node| node.metadata.id.as_deref())
          == Some("b")
      })
      .expect("lowered cell");

    assert_eq!(
      cell.context.style.border_top_color,
      ColorInput::Value(Color([255, 0, 0, 255]))
    );
  }

  #[test]
  fn a_rowspan_reads_the_row_the_line_actually_borders() {
    let table = lower(
      Node::container([
        named_row(
          "first",
          [
            with_span(bordered_cell("tall", "bordered"), "rowspan", "2"),
            bordered_cell("a", "bordered"),
          ],
        ),
        Node::container([bordered_cell("b", "bordered")])
          .with_class_name("heavy-row")
          .with_id("heavy"),
        named_row(
          "third",
          [
            bordered_cell("c", "bordered"),
            bordered_cell("d", "bordered"),
          ],
        ),
      ])
      .with_class_name("collapse"),
    );

    assert_eq!(borders(&table, "c")[0], 3.0);
  }

  #[test]
  fn an_inset_border_resolves_as_the_ridge_it_draws() {
    let table = lower(
      Node::container([
        named_row("top", [bordered_cell("a", "outset-cell")]),
        named_row("bottom", [bordered_cell("b", "inset-cell")]),
      ])
      .with_class_name("collapse"),
    );
    let cell = table
      .children
      .as_deref()
      .unwrap_or_default()
      .iter()
      .find(|child| {
        child
          .node
          .as_ref()
          .and_then(|node| node.metadata.id.as_deref())
          == Some("b")
      })
      .expect("lowered cell");

    assert_eq!(cell.context.style.border_top_style, BorderStyle::Ridge);
  }

  #[test]
  fn only_the_cell_itself_collapses_its_corners() {
    let table = lower(
      Node::container([named_row(
        "only",
        [Node::container([Node::container([Node::text("x")])
          .with_class_name("bordered")
          .with_id("inner")])
        .with_class_name("bordered")
        .with_id("cell")],
      )])
      .with_class_name("collapse"),
    );

    fn find<'a>(node: &'a RenderNode, id: &str) -> Option<&'a RenderNode> {
      if node
        .node
        .as_ref()
        .and_then(|node| node.metadata.id.as_deref())
        == Some(id)
      {
        return Some(node);
      }

      node
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find_map(|child| find(child, id))
    }

    assert!(
      find(&table, "cell")
        .expect("cell")
        .context
        .collapsed_borders
    );
    assert!(
      !find(&table, "inner")
        .expect("inner box")
        .context
        .collapsed_borders
    );
  }

  #[test]
  fn a_fixed_table_shares_its_tracks_evenly() {
    let table =
      lower(Node::container([row([cell("a"), cell("b"), cell("c")])]).with_class_name("fixed"));

    assert_eq!(column_widths(&table, 300.0), [100.0, 100.0, 100.0]);
  }

  #[test]
  fn a_fixed_table_reads_widths_from_its_first_row_only() {
    let table = lower(
      Node::container([
        row([
          Node::container([Node::text("wide")])
            .with_class_name("w80")
            .with_id("wide"),
          cell("b"),
        ]),
        row([cell("c"), bordered_cell("late", "w80")]),
      ])
      .with_class_name("fixed"),
    );

    assert_eq!(column_widths(&table, 300.0), [80.0, 220.0]);
  }

  #[test]
  fn border_spacing_sizes_both_gaps() {
    let spaced = lower(Node::container([row([cell("a"), cell("b")])]).with_class_name("spaced"));

    assert_eq!(
      spaced.context.style.column_gap,
      Gap::Length(Length::Px(4.0))
    );
    assert_eq!(spaced.context.style.row_gap, Gap::Length(Length::Px(8.0)));

    let tight = lower(Node::container([row([cell("a"), cell("b")])]).with_class_name("tight"));

    assert_eq!(tight.context.style.column_gap, Gap::Length(Length::zero()));
  }

  #[test]
  fn a_fixed_span_splits_its_width_across_its_tracks() {
    let table = lower(
      Node::container([row([
        with_span(
          Node::container([Node::text("wide")])
            .with_class_name("w80")
            .with_id("wide"),
          "colspan",
          "2",
        ),
        cell("c"),
      ])])
      .with_class_name("fixed"),
    );

    assert_eq!(column_widths(&table, 300.0), [40.0, 40.0, 220.0]);
  }

  #[test]
  fn border_spacing_insets_the_table_edges() {
    let spaced = lower(Node::container([row([cell("a")])]).with_class_name("spaced"));

    assert_eq!(spaced.context.style.padding_left, Length::Px(4.0));
    assert_eq!(spaced.context.style.padding_top, Length::Px(8.0));

    let plain = lower(Node::container([row([cell("a")])]).with_class_name("plain"));

    assert_eq!(plain.context.style.padding_left, Length::zero());
  }

  #[test]
  fn a_fixed_table_without_a_width_still_follows_its_content() {
    let table = lower(Node::container([row([cell("a"), cell("b")])]).with_class_name("fixed-auto"));

    assert_eq!(
      table
        .context
        .style
        .grid_template_columns
        .as_ref()
        .map(ToCss::to_css_string),
      Some(String::from("auto auto"))
    );
  }

  #[test]
  fn border_spacing_outranks_an_authored_gap() {
    let table = lower(Node::container([row([cell("a"), cell("b")])]).with_class_name("gapped"));

    assert_eq!(table.context.style.column_gap, Gap::Length(Length::Px(4.0)));
    assert_eq!(table.context.style.row_gap, Gap::Length(Length::Px(8.0)));
    assert_eq!(table.context.style.padding_left, Length::Px(4.0));
    assert_eq!(table.context.style.padding_top, Length::Px(8.0));
  }

  /// Every identified box's border box, as `[x, y, width, height]` in the root's space.
  fn boxes(root: &RenderNode) -> Vec<(String, [f32; 4])> {
    fn collect(
      node: &RenderNode,
      node_id: NodeId,
      origin: Point<f32>,
      results: &LayoutResults,
      boxes: &mut Vec<(String, [f32; 4])>,
    ) {
      let layout = results.layout(node_id).expect("layout");
      let x = origin.x + layout.location.x;
      let y = origin.y + layout.location.y;

      if let Some(id) = node
        .node
        .as_ref()
        .and_then(|node| node.metadata.id.as_deref())
      {
        boxes.push((id.to_owned(), [x, y, layout.size.width, layout.size.height]));
      }

      let content = Point {
        x: x + layout.border.left + layout.padding.left,
        y: y + layout.border.top + layout.padding.top,
      };

      for child in results.box_children(node_id).expect("children") {
        if let Some(render) = node
          .children
          .as_deref()
          .unwrap_or_default()
          .get(child.render_index)
          && child.inline_path.is_none()
        {
          collect(render, child.node_id, content, results, boxes);
        }
      }
    }

    let results = LayoutResults::compute(
      root,
      Size {
        width: AvailableSpace::Definite(400.0),
        height: AvailableSpace::MaxContent,
      },
    );
    let mut boxes = Vec::new();

    collect(root, NodeId::ROOT, Point::ZERO, &results, &mut boxes);
    boxes
  }

  /// A table with a header, a footer, captions and rowspans, `body` rows long.
  fn tall_table(class_name: &str, body: usize) -> Node {
    let body_rows = (0..body).map(|index| {
      let mut cells = vec![
        Node::container([Node::text("a")])
          .with_class_name(if index % 3 == 0 { "tall" } else { "short" })
          .with_id(format!("a{index}")),
      ];

      if index % 4 == 1 && index + 2 < body {
        cells.push(with_span(
          Node::container([Node::text("span")])
            .with_class_name("short")
            .with_id(format!("span{index}")),
          "rowspan",
          "2",
        ));
      } else if index % 4 != 2 {
        cells.push(
          Node::container([Node::text("b")])
            .with_class_name("short")
            .with_id(format!("b{index}")),
        );
      }

      Node::container(cells).with_class_name("tr")
    });

    Node::container([
      Node::container([Node::text("cap")])
        .with_class_name("caption")
        .with_id("cap"),
      Node::container([row([
        Node::container([Node::text("h")])
          .with_class_name("w80")
          .with_id("h"),
        Node::container([Node::text("h2")])
          .with_class_name("tall")
          .with_id("h2"),
      ])])
      .with_class_name("thead"),
      Node::container(body_rows.collect::<Vec<_>>()).with_class_name("tbody"),
      Node::container([row([Node::container([Node::text("f")])
        .with_class_name("short")
        .with_id("f")])])
      .with_class_name("tfoot"),
    ])
    .with_class_name(class_name)
  }

  /// The parts of the lowered table's direct children.
  fn parts(table: &RenderNode) -> Vec<Option<TablePart>> {
    table
      .children
      .as_deref()
      .unwrap_or_default()
      .iter()
      .map(|child| child.table_part)
      .collect()
  }

  #[test]
  fn a_table_taller_than_a_grid_moves_its_body_into_grids_of_its_own() {
    let table = lower_within(tall_table("spaced", 10), 8);
    let parts = parts(&table);

    assert_eq!(
      parts
        .iter()
        .filter(|part| **part == Some(TablePart::BodyRows))
        .count(),
      3
    );
    assert_eq!(parts.first(), Some(&Some(TablePart::Caption)));
    assert_eq!(parts.last(), Some(&Some(TablePart::FooterCell)));
  }

  #[test]
  fn a_body_cut_into_grids_lays_out_as_one_grid() {
    for class_name in ["spaced", "collapse"] {
      let cut_table = lower_within(tall_table(class_name, 10), 8);

      assert!(parts(&cut_table).contains(&Some(TablePart::BodyRows)));

      let whole = boxes(&lower(tall_table(class_name, 10)));
      let cut = boxes(&cut_table);

      assert_eq!(whole.len(), cut.len());
      for ((id, expected), (cut_id, actual)) in whole.iter().zip(&cut) {
        assert_eq!(id, cut_id);
        for (expected, actual) in expected.iter().zip(actual) {
          assert!(
            (expected - actual).abs() < 1e-3,
            "{class_name} {id}: {expected:?} != {actual:?}"
          );
        }
      }
    }
  }

  #[test]
  fn a_rowspan_across_the_body_edge_keeps_the_body_in_the_table_grid() {
    let table = lower_within(
      Node::container([
        Node::container([row([with_span(cell("h"), "rowspan", "2")])]).with_class_name("thead"),
        Node::container((0..10).map(|_| row([cell("b")])).collect::<Vec<_>>())
          .with_class_name("tbody"),
      ])
      .with_class_name("table"),
      8,
    );

    assert!(!parts(&table).contains(&Some(TablePart::BodyRows)));
  }

  #[test]
  fn a_cut_waits_for_the_rowspan_above_it_to_end() {
    let rows: Vec<_> = (0..10)
      .map(|index| {
        if index == 2 {
          row([with_span(cell("long"), "rowspan", "4")])
        } else {
          row([cell("b")])
        }
      })
      .collect();
    let table = lower_within(Node::container(rows).with_class_name("table"), 8);
    let runs: Vec<_> = table
      .children
      .as_deref()
      .unwrap_or_default()
      .iter()
      .map(|grid| grid.children.as_deref().map_or(0, <[RenderNode]>::len))
      .collect();

    assert_eq!(runs, [6, 4]);
  }
}
