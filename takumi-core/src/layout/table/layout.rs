//! A table laid out in one pass over its parts: Blink's
//! [`TableLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_layout_algorithm.cc),
//! with the `TableSectionLayoutAlgorithm` and `TableRowLayoutAlgorithm` it runs folded in, under
//! the notice in LICENSE-CHROMIUM.
//!
//! Naive next to Blink: a cell's percentage-height descendants are not tracked, so a row's
//! baseline is always the one measured before the rows grow, and alignment moves a cell's
//! content by growing its padding, so a `content-box` background and the measured runs of the
//! cell's own text start at the moved content.

use std::rc::Rc;

use taffy::{
  AvailableSpace as TaffyAvailableSpace, Baselines, BoxSizing as TaffyBoxSizing, Layout,
  LayoutInput, LayoutOutput, LayoutPartialTree, LengthPercentageAuto, Line, MaybeResolve,
  NodeId as TaffyNodeId, OofCandidates, Point as TaffyPoint, Rect as TaffyRect, RequestedAxis,
  ResolveOrZero, RunMode, Size as TaffySize, SizingMode,
};

use crate::{
  layout::{
    table::{
      borders::TableBorders,
      columns::{
        CellInlineConstraint, ColspanCell, ColumnConstraints, ColumnLocation,
        TABLE_MAX_INLINE_SIZE, TableLength,
      },
      fragment::TableFragment,
      parts::TableGroupedChildren,
      rows::{
        CellAlignment, CellBlockConstraint, ColspanCellTabulator, Row, RowBaselineTabulator,
        RowspanCell, Section, cell_block_size, distribute_rowspan_cell_to_rows,
        distribute_section_fixed_block_size_to_rows, distribute_table_block_size_to_sections,
      },
    },
    tree::{LayoutTree, RenderNode, TableBoxStyle},
  },
  layout_unit::{BoxStrut, LayoutUnit},
  style::{
    BorderCollapse, BoxSizing, CaptionSide, Direction, Display, EmptyCells, Length,
    Size as StyleSize, TableLayout as StyleTableLayout, Visibility,
  },
};

/// One cell, where the table's grid puts it.
pub(crate) struct CellPart {
  pub(crate) node: TaffyNodeId,
  pub(crate) column: usize,
  pub(crate) colspan: usize,
  pub(crate) rowspan: usize,
  /// The border it lays out with.
  pub(crate) borders: BoxStrut,
}

pub(crate) struct RowPart {
  pub(crate) node: TaffyNodeId,
  pub(crate) cells: Vec<CellPart>,
}

pub(crate) struct SectionPart {
  pub(crate) node: TaffyNodeId,
  pub(crate) rows: Vec<RowPart>,
  pub(crate) is_tbody: bool,
}

/// A table's parts as layout reads them, gathered once per layout tree.
pub(crate) struct TableStructure {
  pub(crate) grouped: TableGroupedChildren,
  pub(crate) borders: TableBorders,
  pub(crate) sections: Vec<SectionPart>,
  pub(crate) captions: Vec<TaffyNodeId>,
  pub(crate) columns: Vec<TaffyNodeId>,
  /// Whether a column percentage widens the table's max-content width, which Blink's
  /// `AllowColumnPercentages` denies a table inside a cell, flex item, or grid item.
  pub(crate) allows_intrinsic_column_percentages: bool,
}

/// Blink's `TableTypes::Caption`: the widest a caption is at its narrowest.
#[derive(Clone, Copy, Default)]
struct CaptionConstraint {
  min_size: LayoutUnit,
}

/// A caption laid out at the table's inline size, Blink's `CaptionResult`.
struct CaptionResult {
  node: TaffyNodeId,
  size: TaffySize<f32>,
  margins: BoxStrut,
  output: LayoutOutput,
  is_bottom: bool,
}

/// One cell's measure-pass size and baseline.
#[derive(Clone, Copy)]
struct CellMeasure {
  block_size: LayoutUnit,
  first_baseline: Option<LayoutUnit>,
}

impl<'r> LayoutTree<'r> {
  /// Gathers every table's parts and gives them the layout styles the table algorithm expects:
  /// a table part has no border or padding of its own, a cell lays out with the border the table
  /// resolves for it and takes the size the table gives it, and a column does not lay out.
  pub(in crate::layout) fn prepare_tables(&mut self) {
    let mut inside_cell_flex_or_grid = vec![false; self.node_count()];

    for index in 0..self.node_count() {
      let node = TaffyNodeId::from(index);
      let render_node = self.render_node(node);
      let display = render_node.context.style.base_data.display;
      let denies_percentages = inside_cell_flex_or_grid[index]
        || display == Display::TableCell
        || display.is_flex()
        || matches!(display, Display::Grid | Display::InlineGrid);

      for &child in self.layout_children(node) {
        inside_cell_flex_or_grid[usize::from(child)] = denies_percentages;
      }

      if display.is_table() {
        let structure = self.table_structure_of(node, !inside_cell_flex_or_grid[index]);

        self.set_table_structure(node, structure);
      }
    }
  }

  fn table_structure_of(&mut self, table: TaffyNodeId, allows_percentages: bool) -> TableStructure {
    let render_node = self.render_node(table);
    let grouped = TableGroupedChildren::of(render_node);
    let borders = TableBorders::of(render_node, &grouped);
    let children: Vec<TaffyNodeId> = self.layout_children(table).to_vec();
    let mut sections = Vec::new();
    let mut row_index = 0;

    for (section_index, (index, is_tbody)) in grouped.sections().enumerate() {
      let section = children[index];
      let mut tabulator = ColspanCellTabulator::default();
      let mut rows = Vec::new();

      for &row in self.layout_children(section) {
        let mut cells = Vec::new();

        tabulator.start_row();
        for &cell in self.layout_children(row) {
          let cell_node = self.render_node(cell);
          let colspan = cell_node.table_cell_colspan();
          let rowspan = cell_node.table_cell_rowspan();

          tabulator.find_next_free_column();
          cells.push(CellPart {
            node: cell,
            column: tabulator.current_column(),
            colspan,
            rowspan,
            borders: borders.cell_border(
              cell_node,
              row_index,
              tabulator.current_column(),
              section_index,
            ),
          });
          tabulator.process_cell(colspan, rowspan);
        }
        tabulator.end_row();
        rows.push(RowPart { node: row, cells });
        row_index += 1;
      }

      sections.push(SectionPart {
        node: section,
        rows,
        is_tbody,
      });
    }

    let is_collapsed = borders.is_collapsed;

    self.set_table_box_style(
      table,
      TableBoxStyle {
        border: is_collapsed.then(|| borders.table_border()),
        clears_padding: is_collapsed,
        ..TableBoxStyle::TABLE
      },
    );
    for section in &sections {
      self.set_table_box_style(section.node, TableBoxStyle::PART);
      for row in &section.rows {
        self.set_table_box_style(row.node, TableBoxStyle::PART);
        for cell in &row.cells {
          self.set_table_box_style(
            cell.node,
            TableBoxStyle {
              border: Some(cell.borders),
              ..TableBoxStyle::CELL
            },
          );
        }
      }
    }
    let columns: Vec<TaffyNodeId> = grouped
      .columns
      .iter()
      .map(|&index| children[index])
      .collect();

    for &column in &columns {
      self.set_table_box_style(column, TableBoxStyle::HIDDEN);
      for &col in &self.layout_children(column).to_vec() {
        self.set_table_box_style(col, TableBoxStyle::HIDDEN);
      }
    }

    TableStructure {
      captions: grouped
        .captions
        .iter()
        .map(|&index| children[index])
        .collect(),
      columns,
      grouped,
      borders,
      sections,
      allows_intrinsic_column_percentages: allows_percentages,
    }
  }

  /// Lays out the table `node` for `inputs`.
  pub(in crate::layout) fn compute_table_layout(
    &mut self,
    node: TaffyNodeId,
    inputs: LayoutInput,
  ) -> LayoutOutput {
    let Some(structure) = self.table_structure(node) else {
      return self.compute_hidden(node);
    };

    TableLayout::new(self, node, inputs, structure).run()
  }
}

/// One layout of one table.
struct TableLayout<'t, 'r> {
  tree: &'t mut LayoutTree<'r>,
  node: TaffyNodeId,
  inputs: LayoutInput,
  structure: Rc<TableStructure>,
  border_padding: BoxStrut,
  /// The inline and block `border-spacing`, none when borders collapse.
  spacing: (LayoutUnit, LayoutUnit),
  is_fixed_layout: bool,
  rtl: bool,
}

impl<'t, 'r> TableLayout<'t, 'r> {
  fn new(
    tree: &'t mut LayoutTree<'r>,
    node: TaffyNodeId,
    inputs: LayoutInput,
    structure: Rc<TableStructure>,
  ) -> Self {
    let render_node = tree.render_node(node);
    let style = &render_node.context.style;
    let is_collapsed = style.inherited_data.border_collapse == BorderCollapse::Collapse;
    let spacing = if is_collapsed {
      (LayoutUnit::ZERO, LayoutUnit::ZERO)
    } else {
      let spacing = style.inherited_data.border_spacing.0;
      let sizing = &render_node.context.sizing;

      (
        LayoutUnit::from_float(spacing.x.to_px(sizing, 0.0)),
        LayoutUnit::from_float(spacing.y.to_px(sizing, 0.0)),
      )
    };
    let border_padding = tree.border_padding(node, inputs.parent_size.width);

    Self {
      is_fixed_layout: render_node.is_fixed_table_layout(),
      rtl: style.inherited_data.direction == Direction::Rtl,
      tree,
      node,
      inputs,
      structure,
      border_padding,
      spacing,
    }
  }

  fn render_node(&self) -> &'r RenderNode {
    self.tree.render_node(self.node)
  }

  fn run(mut self) -> LayoutOutput {
    let columns = self.column_constraints();
    let undistributable =
      columns.undistributable_space(self.inline_border_padding(), self.spacing.0);
    let caption_constraint = self.caption_constraint();
    if self.inputs.run_mode == RunMode::ComputeSize
      && self.inputs.axis == RequestedAxis::Horizontal
      && self.inputs.known_dimensions.width.is_none()
    {
      let width = self.intrinsic_inline_size(&columns, undistributable, caption_constraint);

      return LayoutOutput::from_outer_size(TaffySize {
        width: width.to_float(),
        height: 0.0,
      });
    }

    // The start of Blink's `TableLayoutAlgorithm::Layout`.
    let assignable = self.assignable_inline_size(&columns, undistributable, caption_constraint);
    let column_sizes = columns.sizes(assignable, self.is_fixed_layout);
    let (mut column_locations, has_collapsed_columns) =
      columns.locations(&column_sizes, self.spacing.0, false);
    let is_grid_empty = column_locations.is_empty();
    let table_inline_size_before_collapse = if is_grid_empty {
      self.empty_table_inline_size(assignable, undistributable, caption_constraint)
    } else {
      self.size_from_columns(&column_locations)
    };

    // A container stretching an `auto`-width table gives it the width Blink would stretch it to,
    // which still loses its collapsed columns.
    let table_inline_size =
      match self.known_inline_size(&columns, undistributable, caption_constraint) {
        Some(width)
          if !has_collapsed_columns
            || !self.render_node().context.style.box_data.width.is_auto() =>
        {
          width
        }
        _ => self.table_inline_size(&columns, undistributable, caption_constraint),
      };

    let captions = self.lay_out_captions(table_inline_size);
    let captions_block_size = captions
      .iter()
      .map(|caption| {
        LayoutUnit::from_float(caption.size.height) + caption.margins.top + caption.margins.bottom
      })
      .fold(LayoutUnit::ZERO, |total, size| total + size);
    let grid_inline_size = table_inline_size_before_collapse - self.inline_border_padding();
    let mut rows = Rows::default();
    let minimal_grid_block_size = self.compute_rows(
      grid_inline_size,
      &column_locations,
      captions_block_size,
      &mut rows,
    );

    if has_collapsed_columns {
      column_locations = columns.locations(&column_sizes, self.spacing.0, true).0;
    }

    let spacing = if is_grid_empty {
      (LayoutUnit::ZERO, LayoutUnit::ZERO)
    } else {
      self.spacing
    };

    self.generate(
      table_inline_size,
      minimal_grid_block_size,
      &column_locations,
      &rows,
      &captions,
      spacing,
    )
  }

  fn inline_border_padding(&self) -> LayoutUnit {
    self.border_padding.left + self.border_padding.right
  }

  /// Blink's `TableNode::GetColumnConstraints`, measured once per layout tree.
  fn column_constraints(&mut self) -> Rc<ColumnConstraints> {
    if let Some(columns) = self.tree.table_column_constraints(self.node) {
      return columns;
    }

    let columns = Rc::new(self.compute_column_constraints());

    self
      .tree
      .set_table_column_constraints(self.node, Rc::clone(&columns));
    columns
  }

  /// Blink's `ComputeColumnConstraints`.
  fn compute_column_constraints(&mut self) -> ColumnConstraints {
    let structure = Rc::clone(&self.structure);
    let table = self.render_node();
    let mut columns =
      ColumnConstraints::of_column_elements(table, &structure.grouped, self.is_fixed_layout);
    let mut cells: Vec<Option<CellInlineConstraint>> = Vec::new();
    let mut colspan_cells = Vec::new();
    let mut is_first_section = true;

    for section in &structure.sections {
      if self.tree.render_node(section.node).is_empty_table_section() {
        continue;
      }
      for (row_index, row) in section.rows.iter().enumerate() {
        for cell in &row.cells {
          let ignored = self.is_fixed_layout && (!is_first_section || row_index != 0);
          let max_column = if self.is_fixed_layout {
            cell.column + cell.colspan
          } else {
            cell.column + 1
          };

          if max_column > cells.len() {
            cells.resize(max_column, None);
          }
          if ignored {
            continue;
          }

          let constraint = self.cell_inline_constraint(cell);

          if cell.colspan == 1 {
            match &mut cells[cell.column] {
              Some(merged) => merged.encompass(&constraint),
              slot @ None => *slot = Some(constraint),
            }
          } else {
            colspan_cells.push(ColspanCell {
              constraint,
              start_column: cell.column,
              span: cell.colspan,
            });
          }
        }
      }
      is_first_section = false;
    }

    columns.apply_cell_constraints(&cells, self.spacing.0, self.is_fixed_layout, colspan_cells);
    columns
  }

  /// Blink's `CreateCellInlineConstraint` with the cell's padding measured against an indefinite
  /// size, as Blink's `CellPaddingForMeasure` measures it.
  fn cell_inline_constraint(&mut self, cell: &CellPart) -> CellInlineConstraint {
    let render_node = self.tree.render_node(cell.node);
    let padding = self.tree.padding(cell.node, None);
    let border_padding = cell.borders.left + cell.borders.right + padding.left + padding.right;
    let tree = &mut *self.tree;

    CellInlineConstraint::of(
      &render_node.context.style,
      &render_node.context.sizing,
      self.is_fixed_layout,
      border_padding,
      || {
        let mut width = |available| {
          LayoutUnit::from_float_ceil(
            tree
              .compute_child_layout(
                cell.node,
                LayoutInput {
                  run_mode: RunMode::ComputeSize,
                  sizing_mode: SizingMode::ContentSize,
                  axis: RequestedAxis::Horizontal,
                  known_dimensions: TaffySize::NONE,
                  known_dimensions_are_definite: TaffySize {
                    width: true,
                    height: true,
                  },
                  parent_size: TaffySize::NONE,
                  available_space: TaffySize {
                    width: available,
                    height: TaffyAvailableSpace::MaxContent,
                  },
                  vertical_margins_are_collapsible: Line::FALSE,
                },
              )
              .size
              .width,
          )
        };

        (
          width(TaffyAvailableSpace::MinContent),
          width(TaffyAvailableSpace::MaxContent),
        )
      },
    )
  }

  /// Blink's `ComputeCaptionConstraint`: the widest caption's min-content contribution with its
  /// margins.
  fn caption_constraint(&mut self) -> CaptionConstraint {
    let structure = Rc::clone(&self.structure);
    let mut constraint = CaptionConstraint::default();

    for &caption in &structure.captions {
      let width = self.tree.compute_child_layout(
        caption,
        LayoutInput {
          run_mode: RunMode::ComputeSize,
          sizing_mode: SizingMode::InherentSize,
          axis: RequestedAxis::Horizontal,
          known_dimensions: TaffySize::NONE,
          known_dimensions_are_definite: TaffySize {
            width: true,
            height: true,
          },
          parent_size: TaffySize::NONE,
          available_space: TaffySize {
            width: TaffyAvailableSpace::MinContent,
            height: TaffyAvailableSpace::MaxContent,
          },
          vertical_margins_are_collapsible: Line::FALSE,
        },
      );
      let margins = self.tree.margins(caption, None);
      let size = LayoutUnit::from_float_ceil(width.size.width) + margins.left + margins.right;

      constraint.min_size = constraint.min_size.max(size);
    }

    constraint
  }

  /// Blink's `TableLayoutAlgorithm::ComputeMinMaxSizes`, the table's own border and padding
  /// included, for an intrinsic sizing pass.
  fn intrinsic_inline_size(
    &self,
    columns: &ColumnConstraints,
    undistributable: LayoutUnit,
    caption_constraint: CaptionConstraint,
  ) -> LayoutUnit {
    let style = &self.render_node().context.style;
    let allows_percentages = self.structure.allows_intrinsic_column_percentages
      && style.box_data.width != StyleSize::MaxContent;
    let (grid_min, grid_max) =
      columns.grid_inline_min_max(undistributable, self.is_fixed_layout, allows_percentages);
    let min = grid_min.max(caption_constraint.min_size);
    let mut max = grid_max.max(caption_constraint.min_size);

    if self.is_fixed_layout
      && matches!(
        style.box_data.width.as_length(),
        Some(Length::Percentage(_))
      )
    {
      max = TABLE_MAX_INLINE_SIZE;
    }

    let max = min.max(max);

    match self.inputs.available_space.width {
      TaffyAvailableSpace::MinContent => min,
      TaffyAvailableSpace::MaxContent => max,
      TaffyAvailableSpace::Definite(_) => {
        self.table_inline_size(columns, undistributable, caption_constraint)
      }
    }
  }

  /// Blink's `ComputeTableInlineSize`: the width its columns take once the collapsed ones drop
  /// out, at least its widest caption.
  fn table_inline_size(
    &self,
    columns: &ColumnConstraints,
    undistributable: LayoutUnit,
    caption_constraint: CaptionConstraint,
  ) -> LayoutUnit {
    let assignable = self.assignable_inline_size(columns, undistributable, caption_constraint);
    let column_sizes = columns.sizes(assignable, self.is_fixed_layout);
    let (locations, _) = columns.locations(&column_sizes, self.spacing.0, true);

    if locations.is_empty() {
      return self.empty_table_inline_size(assignable, undistributable, caption_constraint);
    }

    self
      .size_from_columns(&locations)
      .max(caption_constraint.min_size)
  }

  /// Blink's `ComputeAssignableTableInlineSize`: the used width of the table, at least its
  /// widest caption, less the space no column takes.
  fn assignable_inline_size(
    &self,
    columns: &ColumnConstraints,
    undistributable: LayoutUnit,
    caption_constraint: CaptionConstraint,
  ) -> LayoutUnit {
    if let Some(width) = self.known_inline_size(columns, undistributable, caption_constraint) {
      return (width - undistributable).clamp_negative_to_zero();
    }

    let (grid_min, grid_max) = self.grid_inline_min_max(columns, undistributable);
    let used = self
      .used_inline_size(grid_min, grid_max)
      .max(caption_constraint.min_size);

    used - undistributable
  }

  /// Blink's `ComputeGridInlineMinMax` for the layout pass.
  fn grid_inline_min_max(
    &self,
    columns: &ColumnConstraints,
    undistributable: LayoutUnit,
  ) -> (LayoutUnit, LayoutUnit) {
    let allows_percentages =
      self.render_node().context.style.box_data.width != StyleSize::MaxContent;

    columns.grid_inline_min_max(undistributable, self.is_fixed_layout, allows_percentages)
  }

  /// The width the table's container gives it, never narrower than its columns and captions
  /// need, as Blink lays out a table no narrower than its min-content width.
  fn known_inline_size(
    &self,
    columns: &ColumnConstraints,
    undistributable: LayoutUnit,
    caption_constraint: CaptionConstraint,
  ) -> Option<LayoutUnit> {
    let width = LayoutUnit::from_float(self.inputs.known_dimensions.width?);
    let (grid_min, _) = self.grid_inline_min_max(columns, undistributable);

    Some(width.max(grid_min).max(caption_constraint.min_size))
  }

  /// Blink's `ComputeUsedInlineSizeForTableFragment`: the table's `width`, fitting its content
  /// when `auto`, held between its `min-width` and `max-width` and never narrower than
  /// `grid_min`.
  fn used_inline_size(&self, grid_min: LayoutUnit, grid_max: LayoutUnit) -> LayoutUnit {
    let render_node = self.render_node();
    let style = &render_node.context.style;
    let sizing = &render_node.context.sizing;
    let available = match self.inputs.available_space.width {
      TaffyAvailableSpace::Definite(width) => Some(LayoutUnit::from_float(width)),
      _ => None,
    };
    let basis = self.inputs.parent_size.width;
    let margins = self.tree.margins(self.node, basis);
    let border_padding = self.inline_border_padding();
    let content_box = style.box_data.box_sizing == BoxSizing::ContentBox;
    let resolve = |length: Option<Length>| -> Option<LayoutUnit> {
      let value = match TableLength::of(length, sizing) {
        TableLength::Fixed(value) => value,
        TableLength::Percent(percent) => LayoutUnit::from_float(percent * basis? / 100.0),
        TableLength::Other => return None,
      };

      Some(if content_box {
        value + border_padding
      } else {
        value.max(border_padding)
      })
    };
    let margin_sum = margins.left + margins.right;
    let stretch = available.map(|available| (available - margin_sum).max(border_padding));
    // Under a min- or max-content constraint, fitting the content takes that size of it.
    let fit_content = || match (stretch, self.inputs.available_space.width) {
      (Some(stretch), _) => grid_min.max(grid_max.min(stretch)),
      (None, TaffyAvailableSpace::MaxContent) => grid_max,
      (None, _) => grid_min,
    };
    let extent = match style.box_data.width {
      StyleSize::Length(length) => resolve(Some(length)).unwrap_or_else(fit_content),
      StyleSize::MinContent => grid_min,
      StyleSize::MaxContent => grid_max,
      StyleSize::FitContent => fit_content(),
      StyleSize::Stretch => stretch.unwrap_or_else(fit_content),
    };
    let min = resolve(Some(style.box_data.min_width))
      .unwrap_or(LayoutUnit::ZERO)
      .max(grid_min);
    let max =
      resolve(style.box_data.max_width.as_length()).map_or(LayoutUnit::MAX, |max| max.max(min));

    extent.max(min).min(max)
  }

  /// Blink's `ComputeEmptyTableInlineSize`.
  fn empty_table_inline_size(
    &self,
    assignable: LayoutUnit,
    undistributable: LayoutUnit,
    caption_constraint: CaptionConstraint,
  ) -> LayoutUnit {
    let style = &self.render_node().context.style;

    if self.inputs.known_dimensions.width.is_some()
      || !style.box_data.width.is_auto()
      || style.box_data.min_width != Length::Auto
    {
      return assignable + undistributable;
    }
    if caption_constraint.min_size > LayoutUnit::ZERO {
      return caption_constraint
        .min_size
        .max(self.inline_border_padding());
    }
    if self.structure.borders.is_collapsed {
      return LayoutUnit::ZERO;
    }

    assignable + self.inline_border_padding()
  }

  /// Blink's `ComputeTableSizeFromColumns`.
  fn size_from_columns(&self, locations: &[ColumnLocation]) -> LayoutUnit {
    locations.last().map_or(LayoutUnit::ZERO, |last| {
      last.offset + last.size + self.inline_border_padding() + self.spacing.0
    })
  }

  /// Lays every caption out at the table's inline size, Blink's `ComputeCaptionFragments`.
  fn lay_out_captions(&mut self, table_inline_size: LayoutUnit) -> Vec<CaptionResult> {
    let structure = Rc::clone(&self.structure);
    let table_width = table_inline_size.to_float();
    let run_mode = self.inputs.run_mode;

    structure
      .captions
      .iter()
      .map(|&caption| {
        let mut margins = self.tree.margins(caption, Some(table_width));
        let auto_margins = self.tree.auto_inline_margins(caption);
        let width = self.tree.stretched_width(caption, table_width, margins);
        let output = self.tree.compute_child_layout(
          caption,
          LayoutInput {
            run_mode: if run_mode == RunMode::PerformLayout {
              RunMode::PerformLayout
            } else {
              RunMode::ComputeSize
            },
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            known_dimensions: TaffySize {
              width: Some(width),
              height: None,
            },
            known_dimensions_are_definite: TaffySize {
              width: true,
              height: true,
            },
            parent_size: TaffySize {
              width: Some(table_width),
              height: None,
            },
            available_space: TaffySize {
              width: TaffyAvailableSpace::Definite(table_width),
              height: TaffyAvailableSpace::MaxContent,
            },
            vertical_margins_are_collapsible: Line::FALSE,
          },
        );
        // Blink's `ResolveInlineAutoMargins`.
        let free = table_inline_size
          - LayoutUnit::from_float(output.size.width)
          - margins.left
          - margins.right;

        match auto_margins {
          (true, true) => {
            margins.left = (free / 2).clamp_negative_to_zero();
            margins.right = (free - margins.left).clamp_negative_to_zero();
          }
          (true, false) => margins.left = free.clamp_negative_to_zero(),
          (false, true) => margins.right = free.clamp_negative_to_zero(),
          (false, false) => {}
        }

        CaptionResult {
          node: caption,
          size: output.size,
          margins,
          is_bottom: self
            .tree
            .render_node(caption)
            .context
            .style
            .inherited_data
            .caption_side
            == CaptionSide::Bottom,
          output,
        }
      })
      .collect()
  }

  /// Blink's `TableLayoutAlgorithm::ComputeRows`, returning the minimal block size of the table
  /// grid.
  fn compute_rows(
    &mut self,
    grid_inline_size: LayoutUnit,
    column_locations: &[ColumnLocation],
    captions_block_size: LayoutUnit,
    rows: &mut Rows,
  ) -> LayoutUnit {
    let structure = Rc::clone(&self.structure);
    let mut total_min_block_size = LayoutUnit::ZERO;

    for section in &structure.sections {
      self.compute_section_rows(section, grid_inline_size, column_locations, rows);
      total_min_block_size += rows
        .sections
        .last()
        .map_or(LayoutUnit::ZERO, |section| section.block_size);
    }

    let mut minimal_grid_block_size = LayoutUnit::ZERO;

    if let Some(table_block_size) = self.css_block_size(captions_block_size) {
      minimal_grid_block_size = table_block_size;
      let distributable = (table_block_size - self.border_padding.top - self.border_padding.bottom)
        .clamp_negative_to_zero();

      if distributable > total_min_block_size {
        distribute_table_block_size_to_sections(
          self.spacing.1,
          distributable,
          &mut rows.sections,
          &mut rows.rows,
        );
      }
    }

    let row_count = rows.rows.len();

    for row in rows.rows.iter_mut().filter(|row| row.is_collapsed) {
      // Collapsed rows take no block size and shrink the table's minimum.
      if minimal_grid_block_size != LayoutUnit::ZERO {
        minimal_grid_block_size -= row.block_size;
        if row_count > 1 {
          minimal_grid_block_size -= self.spacing.1;
        }
      }
      row.block_size = LayoutUnit::ZERO;
    }

    minimal_grid_block_size
  }

  /// The table's `height` to share among its sections, or `None` when it has none or a flex
  /// container asks for its intrinsic height.
  fn css_block_size(&self, captions_block_size: LayoutUnit) -> Option<LayoutUnit> {
    if let Some(height) = self.inputs.known_dimensions.height {
      return Some(LayoutUnit::from_float(height) - captions_block_size);
    }
    if self.inputs.run_mode == RunMode::ComputeSize
      && self.inputs.sizing_mode == SizingMode::ContentSize
    {
      return None;
    }

    let render_node = self.render_node();
    let style = &render_node.context.style;
    let sizing = &render_node.context.sizing;
    let basis = self.inputs.parent_size.height.map(|height| {
      (LayoutUnit::from_float(height) - captions_block_size).clamp_negative_to_zero()
    });
    let border_padding = self.border_padding.top + self.border_padding.bottom;
    let content_box = style.box_data.box_sizing == BoxSizing::ContentBox;
    let resolve = |length| -> Option<LayoutUnit> {
      let value = match TableLength::of(length, sizing) {
        TableLength::Fixed(value) => value,
        TableLength::Percent(percent) => {
          LayoutUnit::from_float(percent * basis?.to_float() / 100.0)
        }
        TableLength::Other => return None,
      };

      Some(if content_box {
        value + border_padding
      } else {
        value.max(border_padding)
      })
    };
    let height = resolve(style.box_data.height.as_length());
    let min = resolve(Some(style.box_data.min_height));
    let max = resolve(style.box_data.max_height.as_length());

    match (height, min) {
      (None, None) => None,
      (height, min) => {
        let size = height
          .unwrap_or(border_padding)
          .max(min.unwrap_or(LayoutUnit::ZERO));

        Some(max.map_or(size, |max| {
          size.min(max.max(min.unwrap_or(LayoutUnit::ZERO)))
        }))
      }
    }
  }

  /// Blink's `ComputeSectionMinimumRowBlockSizes`.
  fn compute_section_rows(
    &mut self,
    section: &SectionPart,
    grid_inline_size: LayoutUnit,
    column_locations: &[ColumnLocation],
    rows: &mut Rows,
  ) {
    let section_node = self.tree.render_node(section.node);
    let section_style = &section_node.context.style;
    let is_section_collapsed = section_style.inherited_data.visibility == Visibility::Collapse;
    let start_row = rows.rows.len();
    let row_count = section.rows.len();
    let mut rowspan_cells = Vec::new();
    let mut section_block_size = LayoutUnit::ZERO;
    let mut total_row_percent = 0.0_f32;

    for (index, row) in section.rows.iter().enumerate() {
      let mut row_constraint = self.compute_minimum_row_block_size(
        row,
        start_row + index,
        row_count - index,
        grid_inline_size,
        column_locations,
        is_section_collapsed,
        rows,
        &mut rowspan_cells,
      );

      if let Some(percent) = row_constraint.percent.as_mut() {
        *percent = (100.0 - total_row_percent).min(*percent);
        total_row_percent += *percent;
      }
      section_block_size += row_constraint.block_size;
      rows.rows.push(row_constraint);
    }

    rowspan_cells.sort_by(RowspanCell::distribution_order);
    for cell in &rowspan_cells {
      distribute_rowspan_cell_to_rows(cell, self.spacing.1, &mut rows.rows);
    }

    section_block_size = rows.rows[start_row..]
      .iter()
      .fold(LayoutUnit::ZERO, |total, row| total + row.block_size);
    section_block_size += self.spacing.1 * row_count.saturating_sub(1) as i32;

    let sizing = &section_node.context.sizing;
    let section_length = TableLength::of(section_style.box_data.height.as_length(), sizing);

    if let Some(fixed) = section_length.fixed()
      && fixed > section_block_size
    {
      distribute_section_fixed_block_size_to_rows(
        start_row,
        row_count,
        fixed,
        self.spacing.1,
        &mut rows.rows,
      );
      section_block_size = fixed;
    }

    rows.sections.push(Section {
      start_row,
      row_count,
      block_size: section_block_size,
      percent: section_length.percent(),
      is_constrained: !matches!(section_length, TableLength::Other),
      is_tbody: section.is_tbody,
      needs_redistribution: false,
    });
  }

  /// Blink's `ComputeMinimumRowBlockSize`: measures each cell of `row` at the width of its
  /// columns.
  #[allow(clippy::too_many_arguments)]
  fn compute_minimum_row_block_size(
    &mut self,
    row: &RowPart,
    row_index: usize,
    rows_left_in_section: usize,
    grid_inline_size: LayoutUnit,
    column_locations: &[ColumnLocation],
    is_section_collapsed: bool,
    rows: &mut Rows,
    rowspan_cells: &mut Vec<RowspanCell>,
  ) -> Row {
    let start_cell_index = rows.cells.len();
    let mut max_cell_block_size = LayoutUnit::ZERO;
    let mut row_percent: Option<f32> = None;
    let mut is_constrained = false;
    let mut has_rowspan_start = false;
    let mut tabulator = RowBaselineTabulator::default();

    for cell in &row.cells {
      let effective_rowspan = cell.rowspan.min(rows_left_in_section).max(1);
      let has_effective_rowspan = effective_rowspan > 1;
      let render_node = self.tree.render_node(cell.node);
      let style = &render_node.context.style;
      let sizing = &render_node.context.sizing;
      let alignment = CellAlignment::of(style);
      let inline_size = span_inline_size(column_locations, cell.column, cell.colspan);
      let measure = self.measure_cell(cell, inline_size, grid_inline_size, alignment);
      let height = TableLength::of(style.box_data.height.as_length(), sizing);
      let padding = self
        .tree
        .padding(cell.node, Some(grid_inline_size.to_float()));
      let border_padding = cell.borders.top + cell.borders.bottom + padding.top + padding.bottom;
      let constraint = CellBlockConstraint {
        min_block_size: measure.block_size,
        effective_rowspan,
        is_constrained: height.fixed().is_some(),
      };

      is_constrained |= constraint.is_constrained && !has_effective_rowspan;
      tabulator.process_cell(
        measure.block_size,
        measure.first_baseline,
        cell.borders.bottom + padding.bottom,
        alignment,
        self.tree.render_node(cell.node).has_table_cell_content(),
        has_effective_rowspan,
      );
      rows.cells.push(constraint);
      rows.measures.push(measure);

      // https://quirks.spec.whatwg.org/#the-table-cell-height-box-sizing-quirk does not apply,
      // as takumi renders in no-quirks mode.
      let css_block_size = height.fixed().map(|size| {
        if style.box_data.box_sizing == BoxSizing::BorderBox {
          size.max(border_padding)
        } else {
          size + border_padding
        }
      });
      let css_percent = height.percent();

      if has_effective_rowspan {
        has_rowspan_start = true;
        let mut min_block_size = measure.block_size;

        if let Some(size) = css_block_size {
          min_block_size = min_block_size.max(size);
        }
        rowspan_cells.push(RowspanCell {
          start_row: row_index,
          effective_rowspan,
          min_block_size,
        });
      } else {
        if css_block_size.is_some() || css_percent.is_some() {
          is_constrained = true;
        }
        if let Some(percent) = css_percent {
          row_percent = Some(row_percent.unwrap_or(0.0).max(percent));
        }
        max_cell_block_size = max_cell_block_size
          .max(measure.block_size)
          .max(css_block_size.unwrap_or(LayoutUnit::ZERO));
      }
    }

    let row_node = self.tree.render_node(row.node);
    let row_height = TableLength::of(
      row_node.context.style.box_data.height.as_length(),
      &row_node.context.sizing,
    );

    match row_height {
      TableLength::Percent(percent) => {
        is_constrained = true;
        row_percent = Some(row_percent.unwrap_or(0.0).max(percent));
      }
      TableLength::Fixed(size) => {
        is_constrained = true;
        max_cell_block_size = max_cell_block_size.max(size);
      }
      TableLength::Other => {}
    }

    let block_size = tabulator.row_block_size(max_cell_block_size);

    Row {
      block_size,
      start_cell_index,
      baseline: Some(tabulator.baseline(block_size)),
      percent: row_percent,
      is_constrained,
      has_rowspan_start,
      is_collapsed: is_section_collapsed
        || row_node.context.style.inherited_data.visibility == Visibility::Collapse,
    }
  }

  /// Measures a cell at `inline_size` with an indefinite block size: only its size, unless it
  /// aligns to its row's baseline, which takes a full layout to find its first baseline.
  fn measure_cell(
    &mut self,
    cell: &CellPart,
    inline_size: LayoutUnit,
    percentage_inline_size: LayoutUnit,
    alignment: CellAlignment,
  ) -> CellMeasure {
    let baseline = alignment == CellAlignment::Baseline;

    self.tree.set_table_cell_aligned_padding(cell.node, None);
    let output = self.tree.without_fragment_items(|tree| {
      tree.compute_child_layout(
        cell.node,
        LayoutInput {
          run_mode: if baseline {
            RunMode::PerformLayout
          } else {
            RunMode::ComputeSize
          },
          sizing_mode: SizingMode::InherentSize,
          axis: RequestedAxis::Both,
          known_dimensions: TaffySize {
            width: Some(inline_size.to_float()),
            height: None,
          },
          known_dimensions_are_definite: TaffySize {
            width: true,
            height: true,
          },
          parent_size: TaffySize {
            width: Some(percentage_inline_size.to_float()),
            height: None,
          },
          available_space: TaffySize {
            width: TaffyAvailableSpace::Definite(inline_size.to_float()),
            height: TaffyAvailableSpace::MaxContent,
          },
          vertical_margins_are_collapsible: Line::FALSE,
        },
      )
    });

    CellMeasure {
      block_size: LayoutUnit::from_float_round(output.size.height),
      first_baseline: output.baselines.first.map(LayoutUnit::from_float_round),
    }
  }

  /// Blink's `TableLayoutAlgorithm::GenerateFragment`, with each section and row placed as
  /// Blink's section and row algorithms place them.
  fn generate(
    &mut self,
    table_inline_size: LayoutUnit,
    minimal_grid_block_size: LayoutUnit,
    column_locations: &[ColumnLocation],
    rows: &Rows,
    captions: &[CaptionResult],
    spacing: (LayoutUnit, LayoutUnit),
  ) -> LayoutOutput {
    let structure = Rc::clone(&self.structure);
    let perform = self.inputs.run_mode == RunMode::PerformLayout;
    let is_table_block_size_specified = !self.render_node().context.style.box_data.height.is_auto();
    let mut offset = LayoutUnit::ZERO;
    let mut oof_candidates = OofCandidates::NONE;
    let mut first_baseline = None;
    let mut fragment = TableFragment::default();
    let add_caption = |tree: &mut LayoutTree<'r>,
                       caption: &CaptionResult,
                       offset: &mut LayoutUnit,
                       oof: &mut OofCandidates,
                       perform: bool| {
      *offset += caption.margins.top;
      if perform {
        let location = TaffyPoint {
          x: caption.margins.left.to_float(),
          y: offset.to_float(),
        };

        tree.place(caption.node, location, caption.size);
        let mut candidates = caption.output.oof_candidates.clone();

        candidates.translate(location);
        oof.append(&mut candidates);
      }
      *offset += LayoutUnit::from_float(caption.size.height) + caption.margins.bottom;
    };

    for caption in captions.iter().filter(|caption| !caption.is_bottom) {
      add_caption(
        self.tree,
        caption,
        &mut offset,
        &mut oof_candidates,
        perform,
      );
    }

    // The table box starts at the first section, or wherever the last top caption ends.
    let section_inline_size =
      (table_inline_size - self.inline_border_padding() - spacing.0 * 2).clamp_negative_to_zero();
    let section_inline_offset = self.border_padding.left + spacing.0;
    let table_box_start = offset;

    offset += self.border_padding.top;
    let mut table_box_end = offset;
    let mut spacing_after_last_section = LayoutUnit::ZERO;
    let mut entered_section = false;

    for (section_index, section) in structure.sections.iter().enumerate() {
      let section_rows = &rows.sections[section_index];
      let before_section = offset;

      offset += spacing.1;
      let (block_size, section_first_baseline, has_descendants) = self.place_section(
        section,
        section_index,
        section_rows,
        rows,
        column_locations,
        section_inline_size,
        is_table_block_size_specified,
        spacing,
        perform,
        &mut fragment,
        &mut oof_candidates,
        TaffyPoint {
          x: section_inline_offset,
          y: offset,
        },
      );

      spacing_after_last_section = spacing.1;
      if !has_descendants {
        // An empty section takes no border spacing before it.
        offset = before_section;
        if perform {
          self.tree.place(
            section.node,
            TaffyPoint {
              x: section_inline_offset.to_float(),
              y: offset.to_float(),
            },
            TaffySize {
              width: section_inline_size.to_float(),
              height: block_size.to_float(),
            },
          );
        }
      }
      if first_baseline.is_none()
        && let Some(baseline) = section_first_baseline
      {
        first_baseline = Some(offset + baseline);
      }
      offset += block_size;
      table_box_end = offset;
      entered_section = true;
    }

    if !entered_section {
      table_box_end = offset;
    }

    // Blink's `EndTableBoxLayout`.
    table_box_end += spacing_after_last_section + self.border_padding.bottom;
    let sections_total_size = table_box_end - table_box_start;
    let grid_block_size = sections_total_size.max(minimal_grid_block_size);
    let grid_block_size_inflation = grid_block_size - sections_total_size;

    offset = table_box_start + grid_block_size;
    fragment.grid_rect = (table_box_start.to_float(), grid_block_size.to_float());

    for caption in captions.iter().filter(|caption| caption.is_bottom) {
      add_caption(
        self.tree,
        caption,
        &mut offset,
        &mut oof_candidates,
        perform,
      );
    }

    let intrinsic_block_size = offset.clamp_negative_to_zero() - grid_block_size_inflation;
    let block_size = match self.inputs.known_dimensions.height {
      Some(height) => LayoutUnit::from_float(height),
      None => offset.clamp_negative_to_zero(),
    };

    if perform {
      for &column in &structure.columns {
        self.tree.compute_hidden(column);
      }
      fragment.finish(
        self.tree,
        self.tree.render_node(self.node),
        &structure,
        column_locations,
        self.rtl,
      );
      self.tree.set_table_fragment(self.node, fragment);
    }

    let size = TaffySize {
      width: table_inline_size.to_float(),
      height: block_size.to_float(),
    };
    let mut output = LayoutOutput::from_sizes_and_baselines(
      size,
      TaffyRect::ZERO,
      Baselines {
        first: first_baseline.map(LayoutUnit::to_float),
        last: None,
      },
    );

    let _ = intrinsic_block_size;
    output.oof_candidates = oof_candidates;
    output
  }

  /// Places a section's rows at `origin` in the table, Blink's
  /// `TableSectionLayoutAlgorithm::Layout`: its block size, first baseline, and whether it holds
  /// any rows.
  #[allow(clippy::too_many_arguments)]
  fn place_section(
    &mut self,
    section: &SectionPart,
    section_index: usize,
    section_rows: &Section,
    rows: &Rows,
    column_locations: &[ColumnLocation],
    inline_size: LayoutUnit,
    is_table_block_size_specified: bool,
    spacing: (LayoutUnit, LayoutUnit),
    perform: bool,
    fragment: &mut TableFragment,
    oof: &mut OofCandidates,
    origin: TaffyPoint<LayoutUnit>,
  ) -> (LayoutUnit, Option<LayoutUnit>, bool) {
    let mut offset = LayoutUnit::ZERO;
    let mut first_baseline = None;
    let mut is_first_non_collapsed_row = true;
    let mut row_offsets = vec![LayoutUnit::ZERO];

    for (index, row) in section.rows.iter().enumerate() {
      let row_index = section_rows.start_row + index;
      let row_data = &rows.rows[row_index];

      if !is_first_non_collapsed_row && !row_data.is_collapsed {
        offset += spacing.1;
      }

      let row_origin = TaffyPoint {
        x: origin.x,
        y: origin.y + offset,
      };

      if perform {
        self.place_row(
          row,
          row_index,
          rows,
          column_locations,
          inline_size,
          is_table_block_size_specified,
          spacing,
          fragment,
          oof,
          row_origin,
        );
        self.tree.place(
          row.node,
          TaffyPoint {
            x: 0.0,
            y: offset.to_float(),
          },
          TaffySize {
            width: inline_size.to_float(),
            height: row_data.block_size.to_float(),
          },
        );
        if row_data.is_collapsed {
          fragment.hidden.push(row.node);
        }
      }
      if first_baseline.is_none() {
        first_baseline = Some(offset + row_data.baseline.unwrap_or(LayoutUnit::ZERO));
      }
      offset += row_data.block_size;
      is_first_non_collapsed_row &= row_data.is_collapsed;
      row_offsets.push(offset);
    }

    let block_size = if section.rows.is_empty() {
      section_rows.block_size
    } else {
      offset
    };

    if perform {
      self.tree.place(
        section.node,
        TaffyPoint {
          x: origin.x.to_float(),
          y: origin.y.to_float(),
        },
        TaffySize {
          width: inline_size.to_float(),
          height: block_size.to_float(),
        },
      );
      if self.structure.borders.is_collapsed && row_offsets.len() > 1 {
        fragment
          .section_row_offsets
          .push((section_index, section_rows.start_row, row_offsets));
      }
    }

    (block_size, first_baseline, !section.rows.is_empty())
  }

  /// Places a row's cells, Blink's `TableRowLayoutAlgorithm::Layout`: each cell laid out at the
  /// block size of the rows it spans and its content aligned in it.
  #[allow(clippy::too_many_arguments)]
  fn place_row(
    &mut self,
    row: &RowPart,
    row_index: usize,
    rows: &Rows,
    column_locations: &[ColumnLocation],
    row_inline_size: LayoutUnit,
    is_table_block_size_specified: bool,
    spacing: (LayoutUnit, LayoutUnit),
    fragment: &mut TableFragment,
    oof: &mut OofCandidates,
    row_origin: TaffyPoint<LayoutUnit>,
  ) {
    let row_data = &rows.rows[row_index];

    for (index, cell) in row.cells.iter().enumerate() {
      let cell_index = row_data.start_cell_index + index;
      let constraint = &rows.cells[cell_index];
      let measure = rows.measures[cell_index];
      let (rowspan_block_size, is_initial_block_size_indefinite) = cell_block_size(
        constraint,
        &rows.rows,
        row_index,
        spacing.1,
        is_table_block_size_specified,
      );
      let block_size = if constraint.effective_rowspan > 1 {
        rowspan_block_size
      } else {
        row_data.block_size
      };
      let inline_size = span_inline_size(column_locations, cell.column, cell.colspan);
      let render_node = self.tree.render_node(cell.node);
      let alignment = CellAlignment::of(&render_node.context.style);
      let shift = alignment.content_offset(
        block_size,
        measure.block_size,
        measure.first_baseline,
        row_data.baseline,
      );
      // The content lays out moved down, in a content box the size it measured at unless the
      // cell's block size is definite.
      let below = if is_initial_block_size_indefinite {
        (block_size - measure.block_size - shift).clamp_negative_to_zero()
      } else {
        LayoutUnit::ZERO
      };
      let padding = self
        .tree
        .padding(cell.node, Some(row_inline_size.to_float()));

      self.tree.set_table_cell_aligned_padding(
        cell.node,
        Some((
          (padding.top + shift).to_float(),
          (padding.bottom + below).to_float(),
        )),
      );
      let output = self.tree.compute_child_layout(
        cell.node,
        LayoutInput {
          run_mode: RunMode::PerformLayout,
          sizing_mode: SizingMode::InherentSize,
          axis: RequestedAxis::Both,
          known_dimensions: TaffySize {
            width: Some(inline_size.to_float()),
            height: Some(block_size.to_float()),
          },
          known_dimensions_are_definite: TaffySize {
            width: true,
            height: !is_initial_block_size_indefinite,
          },
          parent_size: TaffySize {
            width: Some(row_inline_size.to_float()),
            height: None,
          },
          available_space: TaffySize {
            width: TaffyAvailableSpace::Definite(inline_size.to_float()),
            height: TaffyAvailableSpace::Definite(block_size.to_float()),
          },
          vertical_margins_are_collapsible: Line::FALSE,
        },
      );
      let start = column_locations
        .get(cell.column)
        .map_or(LayoutUnit::ZERO, |location| location.offset)
        - spacing.0;
      let x = if self.rtl {
        row_inline_size - start - inline_size
      } else {
        start
      };
      let location = TaffyPoint {
        x: x.to_float(),
        y: 0.0,
      };

      self.tree.place(cell.node, location, output.size);

      let mut candidates = output.oof_candidates;

      candidates.translate(TaffyPoint {
        x: (row_origin.x + x).to_float(),
        y: row_origin.y.to_float(),
      });
      oof.append(&mut candidates);

      if row_data.is_collapsed
        || is_hidden_due_to_layout(column_locations, cell.column, cell.colspan)
        || (!self.structure.borders.is_collapsed
          && render_node.context.style.inherited_data.empty_cells == EmptyCells::Hide
          && !render_node.has_table_cell_content())
      {
        fragment.hidden.push(cell.node);
      }
      fragment.cell_columns.push((cell.node, cell.column));
    }
  }
}

/// The inline size of the `colspan` columns from `column`, Blink's cell inline size in
/// `SetupTableCellConstraintSpaceBuilder`.
fn span_inline_size(locations: &[ColumnLocation], column: usize, colspan: usize) -> LayoutUnit {
  let Some(last) = locations.len().checked_sub(1) else {
    return LayoutUnit::ZERO;
  };
  let start = column.min(last);
  let end = (column + colspan - 1).min(last);

  locations[end].offset + locations[end].size - locations[start].offset
}

/// Whether every column a cell spans is collapsed, which hides it.
fn is_hidden_due_to_layout(locations: &[ColumnLocation], column: usize, colspan: usize) -> bool {
  let Some(last) = locations.len().checked_sub(1) else {
    return false;
  };

  (column.min(last)..=(column + colspan - 1).min(last)).all(|index| locations[index].is_collapsed)
}

/// The rows, sections, and cells of one table layout, Blink's `TableTypes::Rows`, `Sections`, and
/// `CellBlockConstraints`, with each cell's measure.
#[derive(Default)]
struct Rows {
  rows: Vec<Row>,
  sections: Vec<Section>,
  cells: Vec<CellBlockConstraint>,
  measures: Vec<CellMeasure>,
}

impl<'r> LayoutTree<'r> {
  /// Sets `node`'s unrounded layout at `location` with `size`, its border and padding as its
  /// layout style resolves them.
  fn place(&mut self, node: TaffyNodeId, location: TaffyPoint<f32>, size: TaffySize<f32>) {
    let style = self.taffy_style(node);
    let calc = |value, basis| self.resolve_calc_value(value, basis);
    let border = style.border.resolve_or_zero(None, calc);
    let padding = style.padding.resolve_or_zero(None, calc);

    self.set_unrounded_layout(
      node,
      &Layout {
        location,
        size,
        border,
        padding,
        ..Layout::new()
      },
    );
  }

  /// The border and padding of `node`'s layout style, its percentages of `basis`.
  fn border_padding(&self, node: TaffyNodeId, basis: Option<f32>) -> BoxStrut {
    let border = self.border(node, basis);
    let padding = self.padding(node, basis);

    BoxStrut {
      top: border.top + padding.top,
      right: border.right + padding.right,
      bottom: border.bottom + padding.bottom,
      left: border.left + padding.left,
    }
  }

  fn border(&self, node: TaffyNodeId, basis: Option<f32>) -> BoxStrut {
    let calc = |value, basis| self.resolve_calc_value(value, basis);

    strut(self.taffy_style(node).border.resolve_or_zero(basis, calc))
  }

  fn padding(&self, node: TaffyNodeId, basis: Option<f32>) -> BoxStrut {
    let calc = |value, basis| self.resolve_calc_value(value, basis);

    strut(self.taffy_style(node).padding.resolve_or_zero(basis, calc))
  }

  fn margins(&self, node: TaffyNodeId, basis: Option<f32>) -> BoxStrut {
    let calc = |value, basis| self.resolve_calc_value(value, basis);

    strut(self.taffy_style(node).margin.resolve_or_zero(basis, calc))
  }

  /// Whether `node`'s inline-start and inline-end margins are `auto`.
  fn auto_inline_margins(&self, node: TaffyNodeId) -> (bool, bool) {
    let margin = self.taffy_style(node).margin;

    (
      margin.left == LengthPercentageAuto::auto(),
      margin.right == LengthPercentageAuto::auto(),
    )
  }

  /// The width `node` takes stretched across `container` with `margins`, as a caption's
  /// implicit stretch resolves it: its `width` when set, held between `min-width` and
  /// `max-width`.
  fn stretched_width(&self, node: TaffyNodeId, container: f32, margins: BoxStrut) -> f32 {
    let style = self.taffy_style(node);
    let calc = |value, basis| self.resolve_calc_value(value, basis);
    let border_padding = self.border_padding(node, Some(container));
    let inline_border_padding = (border_padding.left + border_padding.right).to_float();
    let content_box = style.box_sizing == TaffyBoxSizing::ContentBox;
    let to_border_box = |value: f32| {
      if content_box {
        value + inline_border_padding
      } else {
        value.max(inline_border_padding)
      }
    };
    let width = style
      .size
      .width
      .maybe_resolve(Some(container), calc)
      .map_or(
        container - (margins.left + margins.right).to_float(),
        to_border_box,
      );
    let min = style
      .min_size
      .width
      .maybe_resolve(Some(container), calc)
      .map(to_border_box);
    let max = style
      .max_size
      .width
      .maybe_resolve(Some(container), calc)
      .map(to_border_box);
    let width = max.map_or(width, |max| width.min(max));

    min
      .map_or(width, |min| width.max(min))
      .max(inline_border_padding)
  }
}

fn strut(rect: TaffyRect<f32>) -> BoxStrut {
  BoxStrut {
    top: LayoutUnit::from_float(rect.top),
    right: LayoutUnit::from_float(rect.right),
    bottom: LayoutUnit::from_float(rect.bottom),
    left: LayoutUnit::from_float(rect.left),
  }
}

impl RenderNode {
  /// Blink's `ComputedStyle::IsFixedTableLayout`: `table-layout: fixed` with a `width` that is
  /// neither `auto` nor `max-content`.
  pub(crate) fn is_fixed_table_layout(&self) -> bool {
    let style = &self.context.style;

    style.base_data.table_layout == StyleTableLayout::Fixed
      && !style.box_data.width.is_auto()
      && style.box_data.width != StyleSize::MaxContent
  }

  /// Whether a cell holds in-flow content, as Blink's `FinalizeTableCellLayout` asks of the
  /// fragments it built.
  pub(crate) fn has_table_cell_content(&self) -> bool {
    self.has_own_inline_content()
      || self
        .children
        .iter()
        .flatten()
        .any(|child| !child.is_out_of_flow())
  }
}
