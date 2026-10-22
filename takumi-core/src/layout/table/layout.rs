//! Blink's
//! [`TableLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_layout_algorithm.cc),
//! which lays its sections out through their own algorithm, under the notice in LICENSE-CHROMIUM.
//!
//! Naive next to Blink: a cell's percentage-height descendants are not tracked, so a row's
//! baseline is always the one measured before the rows grow, and alignment moves a cell's
//! content by growing its padding, so a `content-box` background and the measured runs of the
//! cell's own text start at the moved content.

use std::rc::Rc;

#[cfg(feature = "fragmentation")]
use crate::{
  geometry::Rect,
  layout::{
    box_fragment_builder::BoxFragmentBuilder,
    fragmentation_utils::setup_fragment_builder_for_fragmentation, logical_rect::LogicalBoxStrut,
  },
};

use crate::{
  geometry::{NodeId, Point, Size},
  layout::{
    constraint_space::{AutoSizeBehavior, CacheSlot, ConstraintSpace},
    content_alignment::BlockContentAlignment,
    fragmentation_layout_input::{ChildBreakToken, FragmentationLayoutInput},
    layout_result::{LayoutResult, OutOfFlowCandidate},
    layout_style::LayoutStyle,
    length_utils::{LengthResolver, inline_size_from_aspect_ratio},
    logical_rect::LogicalSize,
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult, SizeType},
    out_of_flow::ContainingBlock,
    table::{
      borders::TableBorders,
      columns::{
        CellInlineConstraint, ColspanCell, ColumnConstraints, ColumnLocation,
        TABLE_MAX_INLINE_SIZE, TableLength, span_inline_size,
      },
      constraint_space_data::{TableConstraintSpaceData, TablePart},
      fragment::TableFragment,
      fragmentation::{ResumedTableBox, TableBoxExtent, TableChild},
      parts::TableGroupedChildren,
      rows::{
        CellBlockConstraint, CellMeasure, ColspanCellTabulator, Row, RowBaselineTabulator, Rows,
        RowspanCell, Section, distribute_rowspan_cell_to_rows,
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
  pub(crate) node: NodeId,
  pub(crate) column: usize,
  pub(crate) colspan: usize,
  pub(crate) rowspan: usize,
  /// The border it lays out with.
  pub(crate) borders: BoxStrut,
}

pub(crate) struct RowPart {
  pub(crate) node: NodeId,
  pub(crate) cells: Vec<CellPart>,
}

pub(crate) struct SectionPart {
  pub(crate) node: NodeId,
  pub(crate) rows: Vec<RowPart>,
  pub(crate) is_tbody: bool,
}

/// A table's parts as layout reads them, gathered once per layout tree.
pub(crate) struct TableStructure {
  pub(crate) grouped: TableGroupedChildren,
  pub(crate) borders: TableBorders,
  pub(crate) sections: Vec<SectionPart>,
  pub(crate) captions: Vec<NodeId>,
  pub(crate) columns: Vec<NodeId>,
  /// Whether a column percentage widens the table's max-content width, which Blink's
  /// `AllowColumnPercentages` denies a table inside a cell, flex item, or grid item.
  pub(crate) allows_intrinsic_column_percentages: bool,
}

impl TableStructure {
  /// The index of the header group among the sections, which lead with it.
  pub(super) fn header_section(&self) -> Option<usize> {
    self.grouped.header.map(|_| 0)
  }

  /// The index of the footer group among the sections, which close with it.
  pub(super) fn footer_section(&self) -> Option<usize> {
    self.grouped.footer.map(|_| self.sections.len() - 1)
  }
}

/// Blink's `TableTypes::Caption`: the widest a caption is at its narrowest.
#[derive(Clone, Copy, Default)]
struct CaptionConstraint {
  min_size: LayoutUnit,
}

/// A caption laid out at the table's inline size, Blink's `CaptionResult`.
/// A section the table placed in its fragment.
pub(super) struct PlacedSection {
  pub(super) node: NodeId,
  pub(super) fragment_index: usize,
  /// The table row its rows start at, then their block-start edges and the last one's block end.
  row_offsets: Option<(usize, Vec<LayoutUnit>)>,
}

struct CaptionResult {
  node: NodeId,
  result: Rc<LayoutResult>,
  margins: BoxStrut,
  is_bottom: bool,
}

impl<'r> LayoutTree<'r> {
  /// Gathers every table's parts and gives them the layout styles the table algorithm expects:
  /// a table part has no border or padding of its own, a cell lays out with the border the table
  /// resolves for it and takes the size the table gives it, and a column does not lay out.
  pub(in crate::layout) fn prepare_tables(&mut self, start: usize) {
    let mut inside_cell_flex_or_grid = vec![false; self.node_count()];

    for index in start..self.node_count() {
      let node = NodeId::new(index);
      let render_node = self.render_node(node);
      let display = render_node.context.style.base_data.display;
      let denies_percentages = inside_cell_flex_or_grid[index]
        || display == Display::TableCell
        || display.is_flex()
        || display.is_grid();

      for child in self.child_nodes(node) {
        inside_cell_flex_or_grid[usize::from(child)] = denies_percentages;
      }

      if display.is_table() {
        let structure = self.table_structure_of(node, !inside_cell_flex_or_grid[index]);

        self.set_table_structure(node, structure);
      }
    }
  }

  fn table_structure_of(&mut self, table: NodeId, allows_percentages: bool) -> TableStructure {
    let render_node = self.render_node(table);
    let grouped = TableGroupedChildren::of(render_node);
    let borders = TableBorders::of(render_node, &grouped);
    let children = self.child_nodes(table);
    let mut sections = Vec::new();
    let mut row_index = 0;

    for (section_index, (index, is_tbody)) in grouped.sections().enumerate() {
      let section = children[index];
      let mut tabulator = ColspanCellTabulator::default();
      let mut rows = Vec::new();

      for row in self.child_nodes(section) {
        let mut cells = Vec::new();

        tabulator.start_row();
        for cell in self.child_nodes(row) {
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
    let columns: Vec<NodeId> = grouped
      .columns
      .iter()
      .map(|&index| children[index])
      .collect();

    for &column in &columns {
      self.set_table_box_style(column, TableBoxStyle::HIDDEN);
      for col in self.child_nodes(column) {
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
}

/// Blink's `TableLayoutAlgorithm`, with the `TableNode` methods it calls.
pub(crate) struct TableLayoutAlgorithm<'t, 'r> {
  pub(super) tree: &'t mut LayoutTree<'r>,
  pub(super) node: NodeId,
  pub(super) space: &'t ConstraintSpace,
  #[cfg_attr(not(feature = "fragmentation"), allow(dead_code))]
  pub(super) input: &'t FragmentationLayoutInput,
  pub(super) style: Rc<LayoutStyle>,
  pub(super) structure: Rc<TableStructure>,
  border_padding: BoxStrut,
  /// The inline and block `border-spacing`, none when borders collapse.
  spacing: (LayoutUnit, LayoutUnit),
  is_fixed_layout: bool,
  rtl: bool,
  #[cfg(feature = "fragmentation")]
  pub(super) builder: BoxFragmentBuilder,
  /// Blink's `is_known_to_be_last_table_box_`: the table box ends in this fragment.
  #[cfg(feature = "fragmentation")]
  pub(super) is_known_to_be_last_table_box: bool,
}

impl<'t, 'r> TableLayoutAlgorithm<'t, 'r> {
  /// The algorithm for the table `node` in `space`, resumed as `input` says, none for a box the
  /// tree gathered no table parts for.
  pub(crate) fn new(
    tree: &'t mut LayoutTree<'r>,
    node: NodeId,
    space: &'t ConstraintSpace,
    input: &'t FragmentationLayoutInput,
  ) -> Option<Self> {
    let structure = tree.table_structure(node)?;
    let style = Rc::clone(tree.layout_style_rc(node));
    let render_node = tree.render_node(node);
    let computed_style = &render_node.context.style;
    let is_collapsed = computed_style.inherited_data.border_collapse == BorderCollapse::Collapse;
    let spacing = if is_collapsed {
      (LayoutUnit::ZERO, LayoutUnit::ZERO)
    } else {
      let spacing = computed_style.inherited_data.border_spacing.0;
      let sizing = &render_node.context.sizing;

      (
        LayoutUnit::from_float(spacing.x.to_px(sizing, 0.0)),
        LayoutUnit::from_float(spacing.y.to_px(sizing, 0.0)),
      )
    };
    let border_padding = tree.border_padding(node, space.percentage_resolution_inline_size);
    #[cfg(feature = "fragmentation")]
    let builder = BoxFragmentBuilder::new(
      space,
      &style,
      LogicalBoxStrut::from_physical(
        Rect {
          left: border_padding.left,
          right: border_padding.right,
          top: border_padding.top,
          bottom: border_padding.bottom,
        },
        style.direction,
      ),
      input.break_token.clone(),
    );

    Some(Self {
      is_fixed_layout: render_node.is_fixed_table_layout(),
      rtl: computed_style.inherited_data.direction == Direction::Rtl,
      tree,
      node,
      space,
      input,
      style,
      structure,
      border_padding,
      spacing,
      #[cfg(feature = "fragmentation")]
      builder,
      #[cfg(feature = "fragmentation")]
      is_known_to_be_last_table_box: false,
    })
  }

  fn render_node(&self) -> &'r RenderNode {
    self.tree.render_node(self.node)
  }

  /// Blink's `Layout`.
  pub(crate) fn layout(mut self) -> LayoutResult {
    let columns = self.column_constraints();
    let undistributable =
      columns.undistributable_space(self.inline_border_padding(), self.spacing.0);
    let caption_constraint = self.caption_constraint();
    let table_inline_size = if self.space.is_fixed_inline_size {
      self.space.inline_size()
    } else {
      self.table_inline_size(&columns, undistributable, caption_constraint)
    };
    #[cfg(feature = "fragmentation")]
    {
      setup_fragment_builder_for_fragmentation(
        self.space,
        &self.style,
        table_inline_size,
        &mut self.builder,
      );
      if self.is_known_to_be_last_table_box {
        self.builder.should_clone_box_end_decorations = false;
        self
          .builder
          .should_prevent_break_before_block_end_decorations = true;
      }
    }

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
    let captions = self.lay_out_captions(table_inline_size);
    let captions_block_size = captions
      .iter()
      .map(|caption| caption.result.size.block_size + caption.margins.top + caption.margins.bottom)
      .fold(LayoutUnit::ZERO, |total, size| total + size);
    let grid_inline_size = table_inline_size_before_collapse - self.inline_border_padding();
    let (rows, minimal_grid_block_size, total_table_min_block_size) =
      self.compute_rows(grid_inline_size, &column_locations, captions_block_size);

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
      total_table_min_block_size,
      &column_locations,
      Rc::new(rows),
      &captions,
      spacing,
    )
  }

  /// Blink's `ComputeMinMaxSizes`.
  pub(crate) fn compute_min_max_sizes(mut self) -> MinMaxSizesResult {
    let columns = self.column_constraints();
    let undistributable =
      columns.undistributable_space(self.inline_border_padding(), self.spacing.0);
    let caption_constraint = self.caption_constraint();
    let style = &self.render_node().context.style;
    let allows_percentages = self.structure.allows_intrinsic_column_percentages
      && style.box_data.width != StyleSize::MaxContent;
    let (grid_min, grid_max) =
      columns.grid_inline_min_max(undistributable, self.is_fixed_layout, allows_percentages);
    let mut sizes = MinMaxSizes {
      min_size: grid_min.max(caption_constraint.min_size),
      max_size: grid_max.max(caption_constraint.min_size),
    };

    if self.is_fixed_layout
      && matches!(
        style.box_data.width.as_length(),
        Some(Length::Percentage(_))
      )
    {
      sizes.max_size = TABLE_MAX_INLINE_SIZE;
    }
    sizes.max_size = sizes.max_size.max(sizes.min_size);

    MinMaxSizesResult::new(sizes, false)
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
        let mut space = ConstraintSpace::new(None, tree.layout_style(cell.node).direction);

        space.is_new_formatting_context = true;
        space.is_table_cell = true;
        space.cache_slot = CacheSlot::Measure;

        let sizes = tree
          .min_max_sizes(cell.node, &space, SizeType::Intrinsic)
          .sizes;

        (sizes.min_size, sizes.max_size)
      },
    )
  }

  /// Blink's `ComputeCaptionConstraint`: the widest caption's min-content contribution with its
  /// margins.
  fn caption_constraint(&mut self) -> CaptionConstraint {
    let structure = Rc::clone(&self.structure);
    let mut constraint = CaptionConstraint::default();

    for &caption in &structure.captions {
      let mut space = ConstraintSpace::new(None, self.tree.layout_style(caption).direction);

      space.is_new_formatting_context = true;
      space.percentage_resolution_block_size = self.space.percentage_resolution_block_size;
      space.cache_slot = CacheSlot::Measure;

      let width = self
        .tree
        .min_and_max_content_contribution(caption, &space)
        .sizes
        .min_size;
      let margins = self
        .tree
        .margins(caption, self.space.percentage_resolution_inline_size);

      constraint.min_size = constraint
        .min_size
        .max(width + margins.left + margins.right);
    }

    constraint
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
    if self.space.is_fixed_inline_size {
      return (self.space.inline_size() - undistributable).clamp_negative_to_zero();
    }

    let allows_percentages =
      self.render_node().context.style.box_data.width != StyleSize::MaxContent;
    let (min_size, max_size) =
      columns.grid_inline_min_max(undistributable, self.is_fixed_layout, allows_percentages);
    let used = self
      .used_inline_size(MinMaxSizes { min_size, max_size })
      .max(caption_constraint.min_size);

    used - undistributable
  }

  /// Blink's `ComputeUsedInlineSizeForTableFragment`: the table's inline size with its content
  /// sized as its grid, `grid_min_max`.
  fn used_inline_size(&self, grid_min_max: MinMaxSizes) -> LayoutUnit {
    let style = &self.style;
    let resolver = LengthResolver::new(self.space, style);

    resolver.inline_size_for_fragment_ignoring_fixed(&mut |size_type| {
      if let (Some(aspect_ratio), SizeType::Content) = (style.aspect_ratio, size_type)
        && let Some(block_size) = resolver.block_size_for_fragment(None, None, None)
      {
        let inline_size = inline_size_from_aspect_ratio(
          resolver.border_padding,
          aspect_ratio,
          style.box_sizing_for_aspect_ratio(),
          block_size,
        );

        return MinMaxSizesResult::new(MinMaxSizes::both(inline_size), false);
      }

      MinMaxSizesResult::new(grid_min_max, false)
    })
  }

  /// Blink's `ComputeEmptyTableInlineSize`.
  fn empty_table_inline_size(
    &self,
    assignable: LayoutUnit,
    undistributable: LayoutUnit,
    caption_constraint: CaptionConstraint,
  ) -> LayoutUnit {
    let style = &self.render_node().context.style;

    if self.space.is_fixed_inline_size
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
    (0..self.structure.captions.len())
      .map(|index| {
        self.lay_out_caption(index, table_inline_size, &ChildBreakToken::default(), None)
      })
      .collect()
  }

  /// Blink's `LayoutCaption` for the caption `index` resumed at `token`, laid out where a page
  /// may break it when `block_offset` places its margin box.
  fn lay_out_caption(
    &mut self,
    index: usize,
    table_inline_size: LayoutUnit,
    token: &ChildBreakToken,
    block_offset: Option<LayoutUnit>,
  ) -> CaptionResult {
    let caption = self.structure.captions[index];
    let caption_style = Rc::clone(self.tree.layout_style_rc(caption));
    let mut margins = self.tree.margins(caption, Some(table_inline_size));

    self.adjust_caption_margins_for_fragmentation(token, &mut margins);

    // Blink's `CreateCaptionConstraintSpace`.
    let mut space = ConstraintSpace::new(Some(table_inline_size), caption_style.direction);

    space.is_new_formatting_context = true;
    space.inline_auto_behavior = AutoSizeBehavior::StretchImplicit;
    space.cache_slot = self.space.cache_slot;
    if let Some(block_offset) = block_offset {
      self.setup_child_space_for_fragmentation(&mut space, caption, block_offset + margins.top);
    }

    let result = self.lay_out_child(caption, &space, token);
    // Blink's `ResolveInlineAutoMargins`.
    let free = table_inline_size - result.size.inline_size - margins.left - margins.right;

    match (
      caption_style.margin.left.is_auto(),
      caption_style.margin.right.is_auto(),
    ) {
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
      result,
      margins,
      is_bottom: self
        .tree
        .render_node(caption)
        .context
        .style
        .inherited_data
        .caption_side
        == CaptionSide::Bottom,
    }
  }

  /// Blink's `TableLayoutAlgorithm::ComputeRows`: the rows, the minimal block size of the table
  /// grid, and the sections' block size before the table's own grew them. A resumed table reuses
  /// the rows its first fragment computed.
  fn compute_rows(
    &mut self,
    grid_inline_size: LayoutUnit,
    column_locations: &[ColumnLocation],
    captions_block_size: LayoutUnit,
  ) -> (Rows, LayoutUnit, LayoutUnit) {
    let (mut rows, total_min_block_size) = match self.resumed_rows() {
      Some(resumed) => resumed,
      None => {
        let structure = Rc::clone(&self.structure);
        let mut rows = Rows::default();
        let mut total_min_block_size = LayoutUnit::ZERO;

        for section in &structure.sections {
          self.compute_section_rows(section, grid_inline_size, column_locations, &mut rows);
          total_min_block_size += rows
            .sections
            .last()
            .map_or(LayoutUnit::ZERO, |section| section.block_size);
        }
        (rows, total_min_block_size)
      }
    };

    let mut minimal_grid_block_size = LayoutUnit::ZERO;

    if let Some(table_block_size) = self.css_block_size(grid_inline_size, captions_block_size) {
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

    (rows, minimal_grid_block_size, total_min_block_size)
  }

  /// The table's block size to share among its sections, Blink's `css_table_block_size`: none
  /// when it has none, or a flex container asks for its intrinsic block size.
  fn css_block_size(
    &self,
    grid_inline_size: LayoutUnit,
    captions_block_size: LayoutUnit,
  ) -> Option<LayoutUnit> {
    let space = self.space;

    if space.is_initial_block_size_indefinite && !space.is_fixed_block_size {
      return None;
    }

    let style = &self.style;
    let mut resolver = LengthResolver::new(space, style);

    resolver.border_padding = self.border_padding_for_size(resolver.border_padding);

    let min_length = style.min_size.height;
    // A definite intrinsic block size makes a resolvable `min-height` resolve.
    let intrinsic_block_size = (!min_length.is_auto()
      && resolver
        .resolve_main_block_length(min_length, None, &mut |_| None, None)
        .is_some())
    .then(|| resolver.border_padding.block_sum());
    let override_available_block_size = space
      .available_block_size
      .map(|block_size| (block_size - captions_block_size).clamp_negative_to_zero());

    resolver.block_size_for_fragment(
      intrinsic_block_size,
      Some(grid_inline_size),
      override_available_block_size,
    )
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
      let alignment = BlockContentAlignment::for_table_cell(style);
      let inline_size = span_inline_size(column_locations, cell.column, cell.colspan);
      let measure = self.measure_cell(cell, inline_size, grid_inline_size);
      let height = TableLength::of(style.box_data.height.as_length(), sizing);
      let padding = self.tree.padding(cell.node, Some(grid_inline_size));
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

  /// Measures a cell at `inline_size` with an indefinite block size.
  fn measure_cell(
    &mut self,
    cell: &CellPart,
    inline_size: LayoutUnit,
    percentage_inline_size: LayoutUnit,
  ) -> CellMeasure {
    self.tree.set_table_cell_aligned_padding(cell.node, None);

    let space = ConstraintSpace::for_table_cell(
      self.tree.layout_style(cell.node).direction,
      inline_size,
      None,
      false,
      percentage_inline_size,
      CacheSlot::Measure,
    );
    let result = self.tree.layout(cell.node, &space);

    CellMeasure {
      block_size: result.size.block_size,
      first_baseline: result.first_baseline,
    }
  }

  /// Blink's `TableLayoutAlgorithm::GenerateFragment`: the table's captions and sections stacked
  /// in this fragment, resumed where the last one broke.
  #[allow(clippy::too_many_arguments)]
  fn generate(
    mut self,
    table_inline_size: LayoutUnit,
    mut minimal_grid_block_size: LayoutUnit,
    total_table_min_block_size: LayoutUnit,
    column_locations: &[ColumnLocation],
    rows: Rc<Rows>,
    captions: &[CaptionResult],
    spacing: (LayoutUnit, LayoutUnit),
  ) -> LayoutResult {
    let structure = Rc::clone(&self.structure);
    let perform = self.space.cache_slot == CacheSlot::Layout;
    let is_table_block_size_specified = !self.style.size.height.is_auto();
    let fragmentainer_space_at_start = self.fragmentainer_space_at_start();
    let previously_consumed_block_size = self.previously_consumed_block_size();
    let resumed = self.resumed_table_box();
    let mut includes_block_start = true;
    let mut includes_block_end = true;
    let mut spacing_before_first_section = spacing.1;
    let mut is_past_table_box = false;

    if let Some(resumed) = resumed {
      minimal_grid_block_size -= resumed.consumed_table_box_block_size;
      is_past_table_box = resumed.is_past_table_box;
      if resumed.has_entered_table_box {
        // The block-start border is in an earlier fragment, unless a cloned box repeats it.
        if !self.should_clone_box_decorations() || is_past_table_box {
          includes_block_start = false;
        }
        spacing_before_first_section = LayoutUnit::ZERO;
      }
      if is_past_table_box {
        includes_block_end = false;
      }
    }

    let border_padding = self.border_padding;
    let block_start_border_padding = |includes_block_start: bool| {
      if includes_block_start {
        border_padding.top
      } else {
        LayoutUnit::ZERO
      }
    };
    let section_inline_size =
      (table_inline_size - self.inline_border_padding() - spacing.0 * 2).clamp_negative_to_zero();
    let section_inline_offset = border_padding.left + spacing.0;
    let data = Rc::new(TableConstraintSpaceData::new(
      Rc::clone(&structure),
      Rc::clone(&rows),
      column_locations.to_vec(),
      section_inline_size,
      spacing,
      is_table_block_size_specified,
      self.rtl,
    ));
    // The captions laid out to size the grid serve, unless pages break them.
    let relayout_captions = self.involved_in_block_fragmentation();
    let mut offset = LayoutUnit::ZERO;
    let mut out_of_flow = Vec::new();
    let mut first_baseline = None;
    let mut spacing_after_last_section = LayoutUnit::ZERO;
    let mut has_container_separation = false;
    let mut table_box: Option<TableBoxExtent> = None;
    let mut grid_block_size_inflation = LayoutUnit::ZERO;
    let mut broke_inside = false;
    let mut has_ended_table_box_layout = false;
    let mut placed_sections = Vec::new();

    if !relayout_captions {
      for caption in captions.iter().filter(|caption| !caption.is_bottom) {
        self.add_caption(caption, &mut offset, &mut out_of_flow, perform);
      }
    }

    let entries = self.child_entries();
    let repeated = self.repeated_sections(
      &entries,
      &rows,
      spacing.1
        + if structure.borders.is_collapsed {
          border_padding.bottom
        } else {
          LayoutUnit::ZERO
        },
      resumed.is_some_and(|resumed| resumed.has_entered_table_box),
    );
    let repeated_footer_block_size = repeated.footer.unwrap_or(LayoutUnit::ZERO);
    let mut has_pending_repeated_footer = repeated.footer.is_some();
    let monolithic_overflow = self.monolithic_overflow();
    // Only content past a non-repeated section overflows a page monolithically.
    let mut has_entered_non_repeated_section = monolithic_overflow > LayoutUnit::ZERO;
    let mut repeated_header_block_size = LayoutUnit::ZERO;
    let mut next_entry = 0;

    while let Some((child, token)) = entries.get(next_entry).cloned() {
      let node = child.node(&structure);

      next_entry += 1;
      if self.breaks_before_early_break_target(node) {
        broke_inside = true;
        if Some(node)
          == structure
            .footer_section()
            .map(|index| structure.sections[index].node)
        {
          has_pending_repeated_footer = false;
        }
        break;
      }

      let mut new_table_box = None;
      let mut is_repeated_section = false;
      let mut offset_before_repeated_header = None;
      let mut has_overlapping_repeated_header = false;
      let mut margins = BoxStrut::default();
      let result;
      let inline_offset;

      match child {
        TableChild::Caption(index) => {
          if !relayout_captions {
            continue;
          }
          if captions[index].is_bottom && !is_past_table_box {
            // The first bottom caption ends the sections, so the grid's size places it.
            let extent = table_box.get_or_insert_with(|| {
              TableBoxExtent::begin(offset, block_start_border_padding(includes_block_start))
            });

            (offset, grid_block_size_inflation) = extent.end_layout(
              border_padding.bottom,
              spacing_after_last_section,
              minimal_grid_block_size,
            );
            has_ended_table_box_layout = true;
            is_past_table_box =
              fragmentainer_space_at_start.is_none_or(|space_left| extent.end <= space_left);
          }

          let caption = self.lay_out_caption(index, table_inline_size, &token, Some(offset));

          margins = caption.margins;
          inline_offset = caption.margins.left;
          result = caption.result;
        }
        TableChild::Section(index) => {
          let collapsible_spacing = if table_box.is_some() {
            spacing.1
          } else {
            // The table box starts at its first section in this fragment.
            new_table_box = Some(TableBoxExtent::begin(
              offset,
              block_start_border_padding(includes_block_start),
            ));
            offset += block_start_border_padding(includes_block_start);
            spacing_before_first_section
          };
          let offset_for_childless_section = offset;
          let mut may_repeat_again = false;

          if Some(index) == structure.header_section() && repeated.header {
            is_repeated_section = true;
            // A header repeats again unless this fragment is known to end the table box.
            may_repeat_again = !self.is_known_to_be_last_table_box();
            offset_before_repeated_header = Some(offset);
            if monolithic_overflow > LayoutUnit::ZERO {
              // The header goes at the page start, over the monolithic content an earlier page
              // pushed down.
              offset = -monolithic_overflow;
              has_overlapping_repeated_header = true;
            }
            // A repeated header draws the collapsed border it shares with the table's start.
            if structure.borders.is_collapsed && !includes_block_start {
              offset += border_padding.top;
            }
          } else if Some(index) == structure.footer_section() && has_pending_repeated_footer {
            is_repeated_section = true;
            has_pending_repeated_footer = false;
            may_repeat_again = !self.is_known_to_be_last_table_box() && repeated.header;
          }
          offset += collapsible_spacing;

          let mut space = data.space(TablePart::Section(index), self.space.cache_slot);

          if is_repeated_section {
            self.setup_repeated_section_space(&mut space, may_repeat_again);
            result = self.lay_out_repeated_section(node, &space, &token);
          } else {
            // A repeated header takes no part in breaking, so the content after it starts the page
            // as far as breaking goes.
            self.setup_child_space_for_fragmentation(
              &mut space,
              node,
              offset - repeated_header_block_size,
            );
            Self::reserve_space_in_fragmentainer(
              &mut space,
              repeated_header_block_size + repeated_footer_block_size,
            );
            result = self.lay_out_child(node, &space, &token);
          }
          inline_offset = section_inline_offset;
          spacing_after_last_section = spacing.1;
          if structure.sections[index].rows.is_empty() {
            // An empty section takes no border spacing before it.
            offset = offset_for_childless_section;
          } else if Self::resumes_past_block_end(&token) {
            spacing_after_last_section = LayoutUnit::ZERO;
          }
        }
      }

      if !(is_repeated_section && Self::is_repeated(&token)) {
        match self.break_before_child_if_needed(
          node,
          &result,
          margins.top + offset - repeated_header_block_size,
          has_container_separation,
        ) {
          Ok(true) => {
            broke_inside = true;
            break;
          }
          Ok(false) => {}
          Err(status) => return self.relayout_and_break_earlier(status),
        }
      }

      if let TableChild::Section(_) = child
        && first_baseline.is_none()
        && let Some(baseline) = result.first_baseline
      {
        first_baseline = Some(offset + baseline);
      }
      offset += margins.top;

      let location = Point::new(inline_offset, offset);

      if perform {
        self.place_table_child(node, &token, location, &result);
        if is_repeated_section {
          self.repeat_section_fragments(node, &token);
        }
      }
      out_of_flow.extend(
        result
          .out_of_flow
          .iter()
          .map(|candidate| candidate.moved_by(location)),
      );
      offset += result.size.block_size + margins.bottom;

      match child {
        TableChild::Section(_) => {
          if !is_repeated_section {
            has_entered_non_repeated_section = true;
          }
          if let Some(before) = offset_before_repeated_header {
            repeated_header_block_size = offset - before + spacing.1;
            if has_overlapping_repeated_header {
              offset = offset.max(before);
            }
          }
          if table_box.is_none() {
            table_box = new_table_box;
          }
          if let Some(extent) = table_box.as_mut() {
            extent.end = offset;
          }
          placed_sections.push(PlacedSection {
            node,
            fragment_index: Self::child_fragment_index(&token),
            row_offsets: result.table_section_row_offsets.clone(),
          });
        }
        TableChild::Caption(index) if captions[index].is_bottom => is_past_table_box = true,
        TableChild::Caption(_) => {}
      }
      if self.space.has_block_fragmentation() {
        has_container_separation |= !is_repeated_section;
        if self.has_inflow_child_break_inside() {
          broke_inside = true;
          break;
        }
      }
    }

    if table_box.is_none() && !is_past_table_box && !broke_inside {
      table_box = Some(TableBoxExtent::begin(
        offset,
        block_start_border_padding(includes_block_start),
      ));
    }
    if has_pending_repeated_footer && table_box.is_some() {
      next_entry = self.add_pending_repeated_footer(
        &data,
        &entries,
        next_entry,
        Point::new(section_inline_offset, offset),
        repeated_footer_block_size,
        has_entered_non_repeated_section,
        perform,
      );
    }
    if next_entry == entries.len() {
      self.set_has_seen_all_children();
    }
    if let Some(extent) = table_box.as_mut()
      && !is_past_table_box
    {
      if broke_inside {
        // A break inside fills the page, and the border spacing after the sections goes.
        if let Some(space_left) = fragmentainer_space_at_start {
          extent.end = extent.end.max(space_left);
        }
        spacing_after_last_section = LayoutUnit::ZERO;
      } else if let Some(space_left) = fragmentainer_space_at_start {
        // The border spacing after the sections stops at the page end.
        spacing_after_last_section = spacing_after_last_section
          .min(space_left - offset - border_padding.bottom)
          .clamp_negative_to_zero();
      }
      if !has_ended_table_box_layout {
        (offset, grid_block_size_inflation) = extent.end_layout(
          border_padding.bottom,
          spacing_after_last_section,
          minimal_grid_block_size,
        );
        has_ended_table_box_layout = true;
        if !broke_inside {
          is_past_table_box =
            fragmentainer_space_at_start.is_none_or(|space_left| extent.end <= space_left);
        }
      }
    }
    if table_box.is_none() {
      includes_block_start = false;
    }
    if !is_past_table_box && (!self.should_clone_box_end_decorations() || table_box.is_none()) {
      includes_block_end = false;
    }
    if !relayout_captions {
      for caption in captions.iter().filter(|caption| caption.is_bottom) {
        self.add_caption(caption, &mut offset, &mut out_of_flow, perform);
      }
    }

    let mut block_size = offset.clamp_negative_to_zero();
    let mut intrinsic_block_size = block_size - grid_block_size_inflation;

    if !has_ended_table_box_layout && !is_past_table_box {
      // The block-end border, padding, and spacing count, so finishing decides whether they fit.
      let block_end = border_padding.bottom + spacing_after_last_section;

      intrinsic_block_size += block_end;
      block_size += block_end;
    }
    block_size += previously_consumed_block_size;
    if self.space.is_fixed_block_size {
      block_size = self.space.available_block_size.unwrap_or_default();
      if self.space.min_block_size_should_encompass_intrinsic_size() {
        block_size = block_size.max(previously_consumed_block_size + intrinsic_block_size);
      }
    }

    let (block_size, intrinsic_block_size) = match self.finish_fragmentation(
      block_size,
      intrinsic_block_size,
      (includes_block_start, includes_block_end),
    ) {
      Ok(sizes) => sizes,
      Err(status) => return self.relayout_and_break_earlier(status),
    };

    if self.involved_in_block_fragmentation()
      && let Some(extent) = table_box.as_mut()
    {
      // Finishing may have shrunk the fragment, and the grid with it.
      extent.end = extent.end.min(block_size);
    }

    let table_box_will_continue = table_box.is_some() && !is_past_table_box && broke_inside;
    let grid_block_size = table_box.map_or(LayoutUnit::ZERO, |extent| extent.end - extent.start);
    let consumed_table_box_block_size =
      resumed.unwrap_or_default().consumed_table_box_block_size + grid_block_size;
    let has_entered_table_box =
      resumed.is_some_and(|resumed| resumed.has_entered_table_box) || table_box.is_some();

    // The table box ends here, which the layout could not know up front: a header laid out to
    // repeat, or block-end decorations cloned for a later page, have to lay out again without.
    if (repeated.header || self.should_clone_box_end_decorations())
      && has_entered_table_box
      && !table_box_will_continue
      && !self.is_known_to_be_last_table_box()
    {
      return self.relayout_as_last_table_box();
    }

    self.keep_break_token_data(
      rows,
      total_table_min_block_size,
      ResumedTableBox {
        consumed_table_box_block_size,
        has_entered_table_box,
        is_past_table_box,
      },
    );

    if perform {
      let mut fragment = TableFragment {
        grid_rect: table_box.map_or((0.0, 0.0), |extent| {
          (extent.start.to_float(), grid_block_size.to_float())
        }),
        ..TableFragment::default()
      };

      self.record_parts(&data, &placed_sections, &mut fragment);
      for &column in &structure.columns {
        self.tree.hide(column);
      }
      fragment.finish(
        self.tree,
        self.tree.render_node(self.node),
        &structure,
        &placed_sections,
        column_locations,
        self.rtl,
      );
      self
        .tree
        .set_table_fragment(self.node, self.fragment_index(), fragment);
    }

    let size = LogicalSize::new(table_inline_size, block_size);
    let mut result = LayoutResult::of_box(size, &self.style, self.space);

    if !out_of_flow.is_empty() {
      let claims = self.tree.claims(self.node);
      let containing_block = ContainingBlock::padding_box(
        Size {
          width: size.inline_size,
          height: size.block_size,
        },
        result.border,
      );

      out_of_flow = match self.defer_out_of_flow(claims, out_of_flow, containing_block) {
        Ok(deferred) => deferred,
        Err(out_of_flow) => self.tree.lay_out_out_of_flow(
          self.node,
          claims,
          out_of_flow,
          containing_block,
          self.space,
        ),
      };
    }

    result.intrinsic_block_size = intrinsic_block_size;
    result.bfc_line_offset = self.space.bfc_offset.line_offset;
    result.bfc_block_offset = Some(self.space.bfc_offset.block_offset);
    result.first_baseline = first_baseline;
    result.out_of_flow = out_of_flow;
    self.finish_break_info(&mut result);
    result
  }

  /// Blink's repeated footer of a fragment the table broke in: break tokens for the sections from
  /// `next_entry` the fragment did not reach, then the footer at `location` or the page end, or a
  /// break before it. Returns the entry past the footer.
  #[allow(clippy::too_many_arguments)]
  fn add_pending_repeated_footer(
    &mut self,
    data: &Rc<TableConstraintSpaceData>,
    entries: &[(TableChild, ChildBreakToken)],
    mut next_entry: usize,
    location: Point<LayoutUnit>,
    footer_block_size: LayoutUnit,
    has_entered_non_repeated_section: bool,
    perform: bool,
  ) -> usize {
    let structure = Rc::clone(&self.structure);
    let footer = structure.footer_section();

    while let Some((child, token)) = entries.get(next_entry).cloned() {
      let node = child.node(&structure);

      next_entry += 1;

      let index = match child {
        TableChild::Section(index) if Some(index) == footer => index,
        _ => {
          self.add_break_token_before(node);
          continue;
        }
      };
      let mut block_offset = location.y;

      // Monolithic content may overflow the page, and the footer still goes at its end.
      if has_entered_non_repeated_section
        && let Some(end_of_page) = self.footer_offset_at_end_of_page(footer_block_size)
      {
        block_offset = block_offset.min(end_of_page);
      }

      let mut space = data.space(TablePart::Section(index), self.space.cache_slot);

      self.setup_repeated_section_space(&mut space, true);

      let result = self.lay_out_repeated_section(node, &space, &token);

      if !self.fits_repeated_footer(node, &token, &result, block_offset) {
        self.break_before_repeated_footer(node, &result);
      } else if perform {
        self.place_table_child(node, &token, Point::new(location.x, block_offset), &result);
        self.repeat_section_fragments(node, &token);
      }
      break;
    }
    next_entry
  }

  /// Places a caption laid out to size the grid at `offset`, past its margins.
  fn add_caption(
    &mut self,
    caption: &CaptionResult,
    offset: &mut LayoutUnit,
    out_of_flow: &mut Vec<OutOfFlowCandidate>,
    perform: bool,
  ) {
    *offset += caption.margins.top;

    let location = Point::new(caption.margins.left, *offset);

    if perform {
      self.tree.place_child(
        caption.node,
        0,
        self.fragment_index(),
        location,
        &caption.result,
      );
    }
    out_of_flow.extend(
      caption
        .result
        .out_of_flow
        .iter()
        .map(|candidate| candidate.moved_by(location)),
    );
    *offset += caption.result.size.block_size + caption.margins.bottom;
  }

  /// Records which rows and cells layout hid, the column each cell starts at, and the rows each
  /// placed section's collapsed borders run along.
  fn record_parts(
    &self,
    data: &TableConstraintSpaceData,
    placed_sections: &[PlacedSection],
    fragment: &mut TableFragment,
  ) {
    for (section_index, section) in data.structure.sections.iter().enumerate() {
      let start_row = data.rows.sections[section_index].start_row;

      for (index, row) in section.rows.iter().enumerate() {
        let is_collapsed = data.rows.rows[start_row + index].is_collapsed;

        for cell in &row.cells {
          let render_node = self.tree.render_node(cell.node);

          if is_collapsed
            || is_hidden_due_to_layout(&data.column_locations, cell.column, cell.colspan)
            || (!self.structure.borders.is_collapsed
              && render_node.context.style.inherited_data.empty_cells == EmptyCells::Hide
              && !render_node.has_table_cell_content())
          {
            fragment.hidden.push(cell.node);
          }
          fragment.cell_columns.push((cell.node, cell.column));
        }
        if is_collapsed {
          fragment.hidden.push(row.node);
        }
      }
    }
    for (placed_index, section) in placed_sections.iter().enumerate() {
      if let Some((start_row, row_offsets)) = &section.row_offsets {
        fragment
          .section_row_offsets
          .push((placed_index, *start_row, row_offsets.clone()));
      }
    }
  }
}

/// Whether every column a cell spans is collapsed, which hides it.
fn is_hidden_due_to_layout(locations: &[ColumnLocation], column: usize, colspan: usize) -> bool {
  let Some(last) = locations.len().checked_sub(1) else {
    return false;
  };

  (column.min(last)..=(column + colspan - 1).min(last)).all(|index| locations[index].is_collapsed)
}

impl LayoutTree<'_> {
  /// The border and padding of `node`'s layout style, its percentages of `basis`.
  fn border_padding(&self, node: NodeId, basis: Option<LayoutUnit>) -> BoxStrut {
    let style = self.layout_style(node);

    BoxStrut::from(style.border.map(LayoutUnit::from_float)) + self.padding(node, basis)
  }

  pub(super) fn padding(&self, node: NodeId, basis: Option<LayoutUnit>) -> BoxStrut {
    self
      .layout_style(node)
      .padding
      .map(|padding| padding.resolve_or_zero(basis))
      .into()
  }

  fn margins(&self, node: NodeId, basis: Option<LayoutUnit>) -> BoxStrut {
    self
      .layout_style(node)
      .margin
      .map(|margin| margin.resolve_or_zero(basis))
      .into()
  }
}

impl ConstraintSpace {
  /// Blink's `SetupTableCellConstraintSpaceBuilder`: a space fixing a cell's inline size, and its
  /// block size when there is one.
  pub(super) fn for_table_cell(
    direction: Direction,
    inline_size: LayoutUnit,
    block_size: Option<LayoutUnit>,
    is_initial_block_size_indefinite: bool,
    percentage_inline_size: LayoutUnit,
    cache_slot: CacheSlot,
  ) -> Self {
    let mut space = Self::new(Some(inline_size), direction);

    space.is_new_formatting_context = true;
    space.is_table_cell = true;
    space.is_fixed_inline_size = true;
    space.available_block_size = block_size;
    space.is_fixed_block_size = block_size.is_some();
    space.is_initial_block_size_indefinite = is_initial_block_size_indefinite;
    space.percentage_resolution_inline_size = Some(percentage_inline_size);
    space.cache_slot = cache_slot;
    space
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
