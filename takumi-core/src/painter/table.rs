//! What the parts of a table paint, as Blink's
//! [`table_painters.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/table_painters.cc)
//! paints them: a table's decorations fill only its grid, a row's or row group's and a column's
//! background paint into the cells under them, and collapsed borders paint once, from the table,
//! after its cells' backgrounds. Follows Blink under the notice in LICENSE-CHROMIUM.

use super::{PaintDevice, PaintRole, box_side::BoxSideRect};
use crate::{
  Result,
  context::RenderContext,
  geometry::{ComputedLayout, NodeId, Point, Rect, Size},
  layout::{
    border::BorderSide,
    table::{TableBorders, TableFragment, style_rank},
    tree::{LayoutResults, RenderNode},
  },
  layout_unit::LayoutUnit,
  style::{Affine, BorderStyle, Display},
};

/// Where a cell lies, its border box relative to the part painting into it.
#[derive(Clone, Copy, Debug)]
pub struct CellArea {
  /// The cell's border-box top-left.
  pub offset: Point<f32>,
  /// The cell's border-box size.
  pub size: Size<f32>,
}

impl CellArea {
  /// The smallest area covering both.
  fn covering(self, other: Self) -> Self {
    let left = self.offset.x.min(other.offset.x);
    let top = self.offset.y.min(other.offset.y);
    let right = (self.offset.x + self.size.width).max(other.offset.x + other.size.width);
    let bottom = (self.offset.y + self.size.height).max(other.offset.y + other.size.height);

    Self {
      offset: Point { x: left, y: top },
      size: Size {
        width: right - left,
        height: bottom - top,
      },
    }
  }
}

/// A table part's background painting into one cell under it, as Blink paints it over the whole
/// cell.
///
/// Approximate: the background positions against `area`, the part grown to cover the cell, where
/// Blink positions it against the part alone. Only an image on a part that a spanning cell sticks
/// out of drifts.
#[derive(Clone, Copy, Debug)]
pub struct CellBackground {
  /// The cell, which the background paints inside.
  pub cell: CellArea,
  /// The area the background paints over.
  pub area: CellArea,
}

impl CellBackground {
  /// The background of `part` in this cell, which so far covers the cell alone.
  fn under(self, part: CellArea) -> Self {
    Self {
      area: part.covering(self.area),
      ..self
    }
  }
}

/// A column's or column group's background and the cells it paints into.
pub struct ColumnBackground {
  /// The column's child-index path from the table.
  pub path: Vec<usize>,
  /// The column's context, whose background paints, at the table's paint offset.
  pub context: RenderContext,
  /// The cells under it, relative to the table.
  pub cells: Vec<CellBackground>,
}

/// How a box's shadows, background, and border paint when it is part of a table.
pub enum TableDecorations {
  /// Not a part of a table: it paints as any box does.
  None,
  /// A table: its decorations fill only its grid, laid out as `grid` at `grid_offset` in the
  /// table, and its border paints only when it does not collapse. Each column's background then
  /// paints into the cells under it.
  Table {
    /// The grid's layout, at the table's location moved by `grid_offset`.
    grid: ComputedLayout,
    /// The grid's top-left in the table's border box.
    grid_offset: Point<f32>,
    /// Whether the table paints its own border.
    paints_border: bool,
    /// The column backgrounds, columns first and their groups after them in paint order.
    columns: Vec<ColumnBackground>,
  },
  /// A row or row group: its shadows, and its background into each of its cells.
  Part {
    /// The cells under it, relative to it.
    cells: Vec<CellBackground>,
  },
  /// A cell: its decorations, its border only when the table does not collapse it.
  Cell {
    /// Whether the cell paints its own border.
    paints_border: bool,
    /// What its decorations stay inside, relative to it, when they would cover collapsed borders.
    clip: Option<CellArea>,
  },
}

impl TableDecorations {
  /// How the fragment `fragment_index` of the box at `path` with `node_id` paints its
  /// decorations.
  pub fn of(
    root: &RenderNode,
    results: &LayoutResults,
    path: &[usize],
    node_id: NodeId,
    fragment_index: usize,
  ) -> Result<Self> {
    let Some(node) = root.node_at_path(path) else {
      return Ok(Self::None);
    };

    Ok(match node.context.style.base_data.display {
      Display::Table | Display::InlineTable => {
        let Some(fragment) = results.table_fragment(node_id, fragment_index) else {
          return Ok(Self::None);
        };
        let layout = results.fragment_layout(node_id, fragment_index)?;
        let (grid_offset, grid_size) = fragment.grid_rect;

        Self::Table {
          grid: ComputedLayout {
            location: layout.location
              + Point {
                x: 0.0,
                y: grid_offset,
              },
            size: Size {
              width: layout.size.width,
              height: grid_size,
            },
            ..layout
          },
          grid_offset: Point {
            x: 0.0,
            y: grid_offset,
          },
          paints_border: fragment.borders.is_none(),
          columns: column_backgrounds(node, node_id, fragment_index, results, fragment)?,
        }
      }
      Display::TableHeaderGroup
      | Display::TableRowGroup
      | Display::TableFooterGroup
      | Display::TableRow
        if !node.has_background() =>
      {
        Self::Part { cells: Vec::new() }
      }
      Display::TableHeaderGroup | Display::TableRowGroup | Display::TableFooterGroup => {
        let mut cells = Vec::new();

        for row in results.ordered_children_in(node_id, fragment_index)? {
          let Some(row_fragment) = results.fragment_in(row.node_id, fragment_index) else {
            continue;
          };
          let row_layout = results.fragment_layout(row.node_id, row_fragment)?;
          let Some(row_node) = child(node, row.render_index) else {
            continue;
          };

          collect_cells(
            row_node,
            results,
            row.node_id,
            row_fragment,
            row_layout.location,
            node.has_layer() || row_node.has_layer(),
            |_| true,
            &mut cells,
          )?;
        }

        Self::Part {
          cells: part_backgrounds(
            results.fragment_layout(node_id, fragment_index)?.size,
            cells,
          ),
        }
      }
      Display::TableRow => {
        let mut cells = Vec::new();
        let section_has_layer = path
          .split_last()
          .and_then(|(_, section)| root.node_at_path(section))
          .is_some_and(RenderNode::has_layer);

        collect_cells(
          node,
          results,
          node_id,
          fragment_index,
          Point::ZERO,
          section_has_layer || node.has_layer(),
          |_| true,
          &mut cells,
        )?;
        Self::Part {
          cells: part_backgrounds(
            results.fragment_layout(node_id, fragment_index)?.size,
            cells,
          ),
        }
      }
      Display::TableCell => {
        let ancestor_has_layer = [1, 2].into_iter().any(|depth| {
          path
            .len()
            .checked_sub(depth)
            .and_then(|end| root.node_at_path(&path[..end]))
            .is_some_and(RenderNode::has_layer)
        });
        let layout = results.fragment_layout(node_id, fragment_index)?;

        Self::Cell {
          paints_border: !node.context.collapsed_borders,
          clip: node.background_clip(
            CellArea {
              offset: Point::ZERO,
              size: layout.size,
            },
            layout,
            ancestor_has_layer,
          ),
        }
      }
      _ => Self::None,
    })
  }
}

/// The backgrounds a part of `size` paints into `cells`.
fn part_backgrounds(size: Size<f32>, cells: Vec<CellBackground>) -> Vec<CellBackground> {
  let part = CellArea {
    offset: Point::ZERO,
    size,
  };

  cells.into_iter().map(|cell| cell.under(part)).collect()
}

fn child(node: &RenderNode, index: usize) -> Option<&RenderNode> {
  node.children.as_deref()?.get(index)
}

/// The cells in the fragment `row_fragment_index` of the row `row` that `includes`, layout did not
/// hide, and are visible, at `offset` from the part painting into them, `parent_has_layer` when the
/// row or its row group has one.
#[allow(clippy::too_many_arguments)]
fn collect_cells(
  row: &RenderNode,
  results: &LayoutResults,
  row_id: NodeId,
  row_fragment_index: usize,
  offset: Point<f32>,
  parent_has_layer: bool,
  includes: impl Fn(NodeId) -> bool,
  cells: &mut Vec<CellBackground>,
) -> Result<()> {
  for cell in results.box_children(row_id)? {
    if results.is_hidden_for_paint(cell.node_id) || !includes(cell.node_id) {
      continue;
    }
    let Some(cell_node) = child(row, cell.render_index) else {
      continue;
    };

    if cell_node.context.style.base_data.display != Display::TableCell
      || !cell_node.context.style.is_visible()
    {
      continue;
    }

    let Some(cell_fragment) = results.fragment_in(cell.node_id, row_fragment_index) else {
      continue;
    };
    let layout = results.fragment_layout(cell.node_id, cell_fragment)?;
    let area = CellArea {
      offset: offset + layout.location,
      size: layout.size,
    };

    cells.push(CellBackground {
      cell: cell_node
        .background_clip(area, layout, parent_has_layer)
        .unwrap_or(area),
      area,
    });
  }

  Ok(())
}

/// Blink's `TablePainter::PaintBoxDecorationBackground` for `<col>` and `<colgroup>`: each one with
/// a background, over the union of the table's sections, into the cells in its columns.
fn column_backgrounds(
  table: &RenderNode,
  table_id: NodeId,
  table_fragment_index: usize,
  results: &LayoutResults,
  fragment: &TableFragment,
) -> Result<Vec<ColumnBackground>> {
  let mut backgrounds = Vec::new();
  let (columns_origin, columns_size) = fragment.columns_rect;

  for geometry in &fragment.columns {
    let Some(column) = child(table, geometry.path[0]).and_then(|group| {
      if geometry.path[1] == usize::MAX {
        Some(group)
      } else {
        child(group, geometry.path[1])
      }
    }) else {
      continue;
    };

    if !column.has_background() {
      continue;
    }

    let x = if fragment.rtl {
      columns_size.width - geometry.inline_offset - geometry.inline_size
    } else {
      geometry.inline_offset
    };
    let area = CellArea {
      offset: Point {
        x: columns_origin.x + x,
        y: columns_origin.y,
      },
      size: Size {
        width: geometry.inline_size,
        height: columns_size.height,
      },
    };
    let mut cells = Vec::new();

    for section in results.box_children(table_id)? {
      if !fragment.sections.contains(&section.node_id) {
        continue;
      }
      let Some(section_node) = child(table, section.render_index) else {
        continue;
      };
      let Some(section_fragment) = results.fragment_in(section.node_id, table_fragment_index)
      else {
        continue;
      };
      let section_layout = results.fragment_layout(section.node_id, section_fragment)?;

      for row in results.ordered_children_in(section.node_id, section_fragment)? {
        let Some(row_node) = child(section_node, row.render_index) else {
          continue;
        };
        let Some(row_fragment) = results.fragment_in(row.node_id, section_fragment) else {
          continue;
        };
        let row_layout = results.fragment_layout(row.node_id, row_fragment)?;

        collect_cells(
          row_node,
          results,
          row.node_id,
          row_fragment,
          section_layout.location + row_layout.location,
          section_node.has_layer() || row_node.has_layer(),
          |cell| {
            fragment.cell_column(cell).is_some_and(|index| {
              index >= geometry.start_column && index < geometry.start_column + geometry.span
            })
          },
          &mut cells,
        )?;
      }
    }

    let context = column.context.clone();

    context.enter_fragment(table.context.paint_offset(), context.fragment_index());
    backgrounds.push(ColumnBackground {
      path: geometry
        .path
        .into_iter()
        .filter(|index| *index != usize::MAX)
        .collect(),
      context,
      cells: cells.into_iter().map(|cell| cell.under(area)).collect(),
    });
  }

  Ok(backgrounds)
}

impl RenderNode {
  /// Whether the table paints its collapsed borders, which an anonymous table does too unless
  /// hidden.
  pub fn paints_collapsed_borders(&self) -> bool {
    self.context.style.is_visible()
  }

  /// Whether the box paints in a layer of its own, as Blink's `LayoutObject::HasLayer`.
  fn has_layer(&self) -> bool {
    let style = &self.context.style;

    style.participates_in_positioned_paint_bucket(false) || style.creates_stacking_context(false)
  }

  /// Blink's `TableCellBackgroundClipper`: the cell at `area` less its collapsed borders, snapped
  /// to pixels, when it
  /// or, by `ancestor_has_layer`, its row or row group paints in a layer of its own, after the
  /// table's collapsed borders.
  fn background_clip(
    &self,
    area: CellArea,
    layout: ComputedLayout,
    ancestor_has_layer: bool,
  ) -> Option<CellArea> {
    if !self.context.collapsed_borders || !(ancestor_has_layer || self.has_layer()) {
      return None;
    }

    let border = layout.border;
    let clip = snapped_rect(
      self.context.box_paint_offset(layout),
      border.left,
      border.top,
      (area.size.width - border.left - border.right).max(0.0),
      (area.size.height - border.top - border.bottom).max(0.0),
    );

    Some(CellArea {
      offset: area.offset
        + Point {
          x: clip.left,
          y: clip.top,
        },
      size: Size {
        width: clip.right - clip.left,
        height: clip.bottom - clip.top,
      },
    })
  }

  /// Whether the box has a background colour or image to paint.
  fn has_background(&self) -> bool {
    let background = &self.context.style.background_data;

    background
      .background_color
      .resolve(self.context.current_color)
      .0[3]
      != 0
      || background.background_image.is_some()
  }
}

/// One edge of a table's collapsed-border grid and what paints it, Blink's `TableCollapsedEdge`.
#[derive(Clone, Copy)]
struct CollapsedEdge<'b> {
  borders: &'b TableBorders,
  /// The edge's index, or `None` past either end of the grid.
  index: Option<usize>,
  width: LayoutUnit,
  style: BorderStyle,
}

impl<'b> CollapsedEdge<'b> {
  fn new(borders: &'b TableBorders, index: Option<usize>) -> Self {
    let index = index.filter(|index| *index < borders.edges.len());

    Self {
      borders,
      index,
      width: index.map_or(LayoutUnit::ZERO, |index| borders.border_width(index)),
      style: index.map_or(BorderStyle::None, |index| borders.border_style(index)),
    }
  }

  /// The edge `offset` edges past this one.
  fn offset(&self, offset: isize) -> Self {
    let index = self
      .index
      .and_then(|index| index.checked_add_signed(offset));

    Self::new(self.borders, index)
  }

  fn can_paint(&self) -> bool {
    self.index.is_some() && self.style.is_rendered() && self.width != LayoutUnit::ZERO
  }

  fn box_order(&self) -> u32 {
    self
      .index
      .and_then(|index| self.borders.edges.get(index))
      .map_or(0, |edge| edge.box_order)
  }

  /// Whether the edge runs along a row line rather than a column line.
  fn is_inline_axis(&self, index: usize) -> bool {
    !(index % self.borders.edges_per_row).is_multiple_of(2)
  }

  /// Blink's `TableCollapsedEdge::CompareForPaint`: which of two edges paints the joint they
  /// meet at, -1 when `rhs` does, 1 when `lhs` does, 0 for neither.
  fn compare_for_paint(lhs: &Self, rhs: &Self) -> i8 {
    if lhs.index == rhs.index {
      return 0;
    }
    match (lhs.can_paint(), rhs.can_paint()) {
      (true, true) => {
        if lhs.width != rhs.width {
          return if lhs.width > rhs.width { 1 } else { -1 };
        }
        if lhs.style == rhs.style {
          // The box visited earlier wins, as Blink's `CompareBoxOrder` has it.
          return match lhs.box_order().cmp(&rhs.box_order()) {
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Greater => -1,
            std::cmp::Ordering::Equal => 0,
          };
        }
        if rhs.style == BorderStyle::Hidden {
          return 1;
        }
        if lhs.style == BorderStyle::Hidden {
          return -1;
        }
        if style_rank(lhs.style) > style_rank(rhs.style) {
          1
        } else {
          -1
        }
      }
      (false, false) => 0,
      (false, true) => -1,
      (true, false) => 1,
    }
  }
}

/// The joints at the start and end of one edge, Blink's `ComputeEdgeJoints`: how far each joint
/// reaches across the edge, and whether the edge fills it.
struct EdgeJoints {
  start: (LayoutUnit, LayoutUnit),
  end: (LayoutUnit, LayoutUnit),
  start_wins: bool,
  end_wins: bool,
}

impl EdgeJoints {
  fn of(edge: &CollapsedEdge<'_>, index: usize, over_boundary: bool, under_boundary: bool) -> Self {
    let per_row = edge.borders.edges_per_row as isize;
    let inline = edge.is_inline_axis(index);
    let empty = CollapsedEdge::new(edge.borders, None);
    // Each joint's size is (inline, block).
    let joint = |before: CollapsedEdge<'_>,
                 after: CollapsedEdge<'_>,
                 over: CollapsedEdge<'_>,
                 under: CollapsedEdge<'_>,
                 drops_block: bool| {
      let inline_compare = CollapsedEdge::compare_for_paint(&before, &after);
      let block = if drops_block {
        LayoutUnit::ZERO
      } else if inline_compare == 1 {
        before.width
      } else {
        after.width
      };
      let block_compare = CollapsedEdge::compare_for_paint(&over, &under);
      let inline_size = if block_compare == 1 {
        over.width
      } else {
        under.width
      };
      let inline_vs_block = CollapsedEdge::compare_for_paint(
        if inline_compare == 1 { &before } else { &after },
        if block_compare == 1 { &over } else { &under },
      );

      (
        (inline_size, block),
        inline_compare,
        block_compare,
        inline_vs_block,
      )
    };

    // The joint at the start, then the one at the end.
    let (start, inline_compare, block_compare, inline_vs_block) = if inline {
      joint(
        edge.offset(-2),
        edge.offset(0),
        if over_boundary {
          empty
        } else {
          edge.offset(-(per_row + 1))
        },
        if under_boundary {
          empty
        } else {
          edge.offset(-1)
        },
        over_boundary || under_boundary,
      )
    } else {
      joint(
        edge.offset(-1),
        edge.offset(1),
        if over_boundary {
          empty
        } else {
          edge.offset(-per_row)
        },
        edge.offset(0),
        over_boundary,
      )
    };
    let start_wins = if inline {
      inline_vs_block != -1 && inline_compare != 1
    } else {
      inline_vs_block != 1 && block_compare != 1
    };
    let (end, inline_compare, block_compare, inline_vs_block) = if inline {
      joint(
        edge.offset(0),
        edge.offset(2),
        if over_boundary {
          empty
        } else {
          edge.offset(-(per_row - 1))
        },
        if under_boundary {
          empty
        } else {
          edge.offset(1)
        },
        over_boundary || under_boundary,
      )
    } else {
      joint(
        edge.offset(per_row - 1),
        edge.offset(per_row + 1),
        edge.offset(0),
        if under_boundary {
          empty
        } else {
          edge.offset(per_row)
        },
        under_boundary,
      )
    };
    let end_wins = if inline {
      inline_vs_block != -1 && inline_compare != -1
    } else {
      inline_vs_block != 1 && block_compare != -1
    };

    Self {
      start,
      end,
      start_wins,
      end_wins,
    }
  }
}

/// Blink's `TablePainter::PaintCollapsedBorders`: every collapsed edge of the table `node_id`'s
/// fragment `fragment_index`, as a box side under `transform`, the table's border-box top-left at the origin and at
/// `paint_offset` in the space paint snaps to pixels in. With `visible`, a block range of the
/// table's border box, only the rows near it paint, as Blink paints one table fragment per page.
pub fn paint_collapsed_borders(
  results: &LayoutResults,
  node_id: NodeId,
  fragment_index: usize,
  paint_offset: Point<f32>,
  transform: Affine,
  visible: Option<(f32, f32)>,
  device: &mut dyn PaintDevice,
) -> Result<()> {
  let Some(fragment) = results.table_fragment(node_id, fragment_index) else {
    return Ok(());
  };
  let Some(borders) = fragment.borders.as_ref() else {
    return Ok(());
  };
  if borders.edges_per_row == 0 {
    return Ok(());
  }

  device.set_role(PaintRole::Border);

  let per_row = borders.edges_per_row;
  // The last row each section painted, so the next does not paint its inline edges again.
  let mut previous_painted_row = None;

  for &(section_index, start_row, ref row_offsets) in &fragment.section_row_offsets {
    let Some(&section) = fragment.sections.get(section_index) else {
      continue;
    };
    let Some(section_fragment) = results.fragment_in(section, fragment_index) else {
      continue;
    };
    let section_layout = results.fragment_layout(section, section_fragment)?;
    let section_width = LayoutUnit::from_float(section_layout.size.width);
    let rows = visible.map_or(0..row_offsets.len(), |(top, bottom)| {
      let reach = borders.max_border_width;
      let section_top = LayoutUnit::from_float(section_layout.location.y);
      let top = LayoutUnit::from_float(top) - section_top - reach;
      let bottom = LayoutUnit::from_float(bottom) - section_top + reach;
      // The row above the first edge in view runs its column lines into view.
      let first = row_offsets
        .partition_point(|offset| *offset < top)
        .saturating_sub(1);

      first..row_offsets.partition_point(|offset| *offset <= bottom)
    });
    let end = ((start_row + rows.end) * per_row).min(borders.edges.len());
    let mut index = (start_row + rows.start) * per_row;

    while index < end {
      let edge = CollapsedEdge::new(borders, Some(index));
      let table_row = index / per_row;
      let table_column = index % per_row / 2;
      let fragment_row = table_row - start_row;

      index += 1;

      if !edge.can_paint() {
        continue;
      }

      let row_start = row_offsets[fragment_row];
      let Some(&column_start) = fragment.column_edges.get(table_column) else {
        continue;
      };
      let inline_axis = edge.is_inline_axis(index - 1);
      let joints = EdgeJoints::of(&edge, index - 1, false, false);
      let (mut inline_start, mut block_start, mut inline_size, mut block_size);

      if inline_axis {
        if previous_painted_row == Some(table_row) {
          continue;
        }
        let Some(&column_end) = fragment.column_edges.get(table_column + 1) else {
          continue;
        };

        inline_start = column_start;
        inline_size = column_end - column_start;
        block_start = row_start - edge.width / 2;
        block_size = edge.width;
        if joints.start_wins {
          inline_start -= joints.start.0 / 2;
          inline_size += joints.start.0 / 2;
        } else {
          inline_start += joints.start.0 / 2;
          inline_size -= joints.start.0 / 2;
        }
        if joints.end_wins {
          inline_size += joints.end.0 / 2;
        } else {
          inline_size -= joints.end.0 / 2;
        }
      } else {
        // A column line runs only to the last row of the section.
        if fragment_row + 1 >= row_offsets.len() {
          continue;
        }

        block_start = row_start;
        block_size = row_offsets[fragment_row + 1] - row_start;
        inline_start = column_start - edge.width / 2;
        inline_size = edge.width;
        if joints.start_wins {
          block_start -= joints.start.1 / 2;
          block_size += joints.start.1 / 2;
        } else {
          block_start += joints.start.1 / 2;
          block_size -= joints.start.1 / 2;
        }
        if joints.end_wins {
          block_size += joints.end.1 / 2;
        } else {
          block_size -= joints.end.1 / 2;
        }
      }

      let x = if fragment.rtl {
        section_width - inline_start - inline_size
      } else {
        inline_start
      };
      let rect = snapped_rect(
        paint_offset,
        section_layout.location.x + x.to_float(),
        section_layout.location.y + block_start.to_float(),
        inline_size.to_float(),
        block_size.to_float(),
      );
      let side = if inline_axis {
        BorderSide::Top
      } else {
        BorderSide::Left
      };

      BoxSideRect {
        side,
        x1: rect.left,
        y1: rect.top,
        x2: rect.right,
        y2: rect.bottom,
      }
      .paint(
        borders.border_color(index - 1),
        edge.style,
        [0, 0],
        transform,
        device,
      );
    }
    previous_painted_row = Some(start_row + row_offsets.len() - 1);
  }

  Ok(())
}

/// Blink's `ToPixelSnappedRect` of a rectangle at `paint_offset` plus `(x, y)`, back relative to
/// `paint_offset`.
fn snapped_rect(paint_offset: Point<f32>, x: f32, y: f32, width: f32, height: f32) -> Rect<f32> {
  let (x, y) = (paint_offset.x + x, paint_offset.y + y);
  let left = x.round();
  let top = y.round();

  Rect {
    left: left - paint_offset.x,
    top: top - paint_offset.y,
    right: (x + width).round().max(left) - paint_offset.x,
    bottom: (y + height).round().max(top) - paint_offset.y,
  }
}
