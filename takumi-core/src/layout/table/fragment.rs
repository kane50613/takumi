//! What paint needs of a laid-out table, Blink's table fragment data: where its grid lies apart
//! from its captions, which cells and rows layout hid, the geometry of its columns and column
//! groups, and its collapsed borders with the rows and columns they run along.

use taffy::{NodeId as TaffyNodeId, RoundTree};

use crate::{
  geometry::{NodeId, Point, Size},
  layout::{
    table::{
      borders::TableBorders,
      columns::{ColumnLocation, ColumnVisitor, visit_columns},
      layout::TableStructure,
    },
    tree::{LayoutTree, RenderNode},
  },
  layout_unit::LayoutUnit,
};

/// A `<col>` or `<colgroup>` and the columns it covers, Blink's `TableColumnGeometry`.
#[derive(Clone, Debug)]
pub struct ColumnGeometry {
  /// The child index of the column or group in the table, then of a column in its group, or
  /// `usize::MAX` for a column straight in the table.
  pub(crate) path: [usize; 2],
  pub(crate) start_column: usize,
  pub(crate) span: usize,
  /// Its inline-start edge past the first column's.
  pub(crate) inline_offset: f32,
  pub(crate) inline_size: f32,
  pub(crate) is_group: bool,
}

/// One section's rows in the collapsed-border grid: its index among the table's sections, the
/// table row it starts at, and each row's block-start edge then the last row's block-end edge.
pub(crate) type SectionRowOffsets = (usize, usize, Vec<LayoutUnit>);

/// A laid-out table's paint data.
#[derive(Default, Debug)]
pub struct TableFragment {
  /// The block offset and size of the table box in the table's border box: everything but its
  /// captions, Blink's `TableGridRect`.
  pub(crate) grid_rect: (f32, f32),
  /// The cells and rows layout hid: collapsed, in collapsed columns, or empty with
  /// `empty-cells: hide`.
  pub(crate) hidden: Vec<TaffyNodeId>,
  /// Each cell's first column.
  pub(crate) cell_columns: Vec<(TaffyNodeId, usize)>,
  pub(crate) section_row_offsets: Vec<SectionRowOffsets>,
  /// The sections, in layout order.
  pub(crate) sections: Vec<NodeId>,
  /// Its columns and column groups, each group before the columns in it.
  pub(crate) columns: Vec<ColumnGeometry>,
  /// The union of its sections, which column backgrounds stretch over.
  pub(crate) columns_rect: (Point<f32>, Size<f32>),
  /// The inline-start edge of each column, then the inline-end edge of the last, across the
  /// sections.
  pub(crate) column_edges: Vec<LayoutUnit>,
  /// The collapsed borders, when the table collapses them.
  pub(crate) borders: Option<TableBorders>,
  /// The widest collapsed border.
  pub(crate) max_border_width: LayoutUnit,
  pub(crate) rtl: bool,
}

impl TableFragment {
  /// Gathers the column geometry and collapsed-border grid of a table laid out with
  /// `column_locations`.
  pub(crate) fn finish(
    &mut self,
    tree: &LayoutTree<'_>,
    table: &RenderNode,
    structure: &TableStructure,
    column_locations: &[ColumnLocation],
    rtl: bool,
  ) {
    self.rtl = rtl;
    self.sections = structure
      .sections
      .iter()
      .map(|section| NodeId::from_taffy(section.node))
      .collect();

    let mut union: Option<(Point<f32>, Size<f32>)> = None;

    for section in &structure.sections {
      let layout = tree.get_unrounded_layout(section.node);
      let origin = Point::from_taffy(layout.location);
      let size = Size::from_taffy(layout.size);

      union = Some(match union {
        None => (origin, size),
        Some((union_origin, union_size)) => {
          let left = union_origin.x.min(origin.x);
          let top = union_origin.y.min(origin.y);
          let right = (union_origin.x + union_size.width).max(origin.x + size.width);
          let bottom = (union_origin.y + union_size.height).max(origin.y + size.height);

          (
            Point { x: left, y: top },
            Size {
              width: right - left,
              height: bottom - top,
            },
          )
        }
      });
    }
    self.columns_rect = union.unwrap_or_default();

    if !structure.grouped.columns.is_empty() && !column_locations.is_empty() {
      let mut builder = ColumnGeometries {
        geometries: Vec::new(),
        locations: column_locations,
      };

      visit_columns(
        table,
        &structure.grouped,
        column_locations.len(),
        &mut builder,
      );
      self.columns = builder.into_sorted();
    }

    if structure.borders.is_collapsed && !structure.borders.edges.is_empty() {
      self.column_edges = column_locations
        .iter()
        .map(|location| location.offset)
        .chain(column_locations.last().map(|last| last.offset + last.size))
        .collect();
      self.max_border_width = (0..structure.borders.edges.len())
        .map(|index| structure.borders.border_width(index))
        .max()
        .unwrap_or(LayoutUnit::ZERO);
      self.borders = Some(structure.borders.clone());
    }
  }

  /// Whether layout hid the cell or row `node`, which then paints nothing.
  pub fn hides(&self, node: NodeId) -> bool {
    self.hidden.contains(&node.into_taffy())
  }

  /// The first column of the cell `node`.
  pub(crate) fn cell_column(&self, node: NodeId) -> Option<usize> {
    let node = node.into_taffy();

    self
      .cell_columns
      .iter()
      .find(|(cell, _)| *cell == node)
      .map(|&(_, column)| column)
  }
}

/// Blink's `ColumnGeometriesBuilder`.
struct ColumnGeometries<'l> {
  geometries: Vec<ColumnGeometry>,
  locations: &'l [ColumnLocation],
}

impl ColumnGeometries<'_> {
  fn geometry(&self, path: [usize; 2], start_column: usize, span: usize) -> Option<ColumnGeometry> {
    let end = (start_column + span).checked_sub(1)?;
    let start = self.locations.get(start_column)?;
    let last = self.locations.get(end)?;

    Some(ColumnGeometry {
      path,
      start_column,
      span,
      inline_offset: (start.offset - self.locations[0].offset).to_float(),
      inline_size: (last.offset + last.size - start.offset).to_float(),
      is_group: false,
    })
  }

  /// The geometries with each group before the columns it holds, and the columns in order, as
  /// Blink's `ColumnGeometriesBuilder::Sort` orders them.
  fn into_sorted(mut self) -> Vec<ColumnGeometry> {
    self.geometries.sort_by(|a, b| {
      a.start_column
        .cmp(&b.start_column)
        .then_with(|| a.is_group.cmp(&b.is_group).reverse())
    });
    self.geometries
  }
}

impl ColumnVisitor for ColumnGeometries<'_> {
  fn visit_column(&mut self, _column: &RenderNode, path: [usize; 2], start: usize, span: usize) {
    self.geometries.extend(self.geometry(path, start, span));
  }

  fn leave_group(
    &mut self,
    _group: &RenderNode,
    index: usize,
    start: usize,
    span: usize,
    _has_children: bool,
  ) {
    if span == 0 {
      return;
    }
    if let Some(mut geometry) = self.geometry([index, usize::MAX], start, span) {
      geometry.is_group = true;
      self.geometries.push(geometry);
    }
  }
}
