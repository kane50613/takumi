//! Collapsing adjacent table borders onto one grid line.
//!
//! Naive: every cell still paints its own box, so the winner is drawn whole
//! inside the cell below or right of the line instead of straddling it, and
//! the table's outer edge lands half a border width inside Blink's. Only
//! cell, row and table borders reach the CSS 2.2 section 17.6.2 cascade, and
//! a spanning cell resolves one winner for its whole edge rather than per
//! grid-line segment.

use crate::{
  layout::{table::MAX_ROWSPAN, tree::RenderNode},
  style::{BorderStyle, ColorInput, ComputedStyle, Length, LineWidth, SizingContext},
};

/// Which box a candidate border came from, innermost last.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EdgeOrigin {
  Table,
  Row,
  Cell,
}

#[derive(Clone, Copy)]
struct BorderEdge {
  width: LineWidth,
  style: BorderStyle,
  color: ColorInput,
  px: f32,
  origin: EdgeOrigin,
}

impl BorderEdge {
  fn none() -> Self {
    Self {
      width: LineWidth::Length(Length::zero()),
      style: BorderStyle::None,
      color: ColorInput::transparent(),
      px: 0.0,
      origin: EdgeOrigin::Table,
    }
  }

  /// CSS 2.2 section 17.6.2: wider wins, then the style order, then the
  /// innermost box.
  fn rank(self) -> (f32, u8, EdgeOrigin) {
    let style = match self.style {
      BorderStyle::Double => 8,
      BorderStyle::Solid => 7,
      BorderStyle::Dashed => 6,
      BorderStyle::Dotted => 5,
      BorderStyle::Ridge | BorderStyle::Inset => 4,
      BorderStyle::Groove | BorderStyle::Outset => 3,
      BorderStyle::None | BorderStyle::Hidden => 0,
    };

    (self.px, style, self.origin)
  }
}

#[derive(Clone, Copy)]
enum Side {
  Top,
  Right,
  Bottom,
  Left,
}

impl Side {
  fn of(self, style: &ComputedStyle, sizing: &SizingContext, origin: EdgeOrigin) -> BorderEdge {
    let (width, border_style, color) = match self {
      Self::Top => (
        style.border_top_width,
        style.border_top_style,
        style.border_top_color,
      ),
      Self::Right => (
        style.border_right_width,
        style.border_right_style,
        style.border_right_color,
      ),
      Self::Bottom => (
        style.border_bottom_width,
        style.border_bottom_style,
        style.border_bottom_color,
      ),
      Self::Left => (
        style.border_left_width,
        style.border_left_style,
        style.border_left_color,
      ),
    };

    BorderEdge {
      width,
      style: collapsed_style(border_style),
      color,
      px: if border_style.is_rendered() {
        Length::from(width).to_px(sizing, 0.0)
      } else {
        0.0
      },
      origin,
    }
  }

  fn apply(self, style: &mut ComputedStyle, edge: BorderEdge) {
    match self {
      Self::Top => {
        style.border_top_width = edge.width;
        style.border_top_style = edge.style;
        style.border_top_color = edge.color;
      }
      Self::Right => {
        style.border_right_width = edge.width;
        style.border_right_style = edge.style;
        style.border_right_color = edge.color;
      }
      Self::Bottom => {
        style.border_bottom_width = edge.width;
        style.border_bottom_style = edge.style;
        style.border_bottom_color = edge.color;
      }
      Self::Left => {
        style.border_left_width = edge.width;
        style.border_left_style = edge.style;
        style.border_left_color = edge.color;
      }
    }
  }
}

/// Winning border per grid line, indexed by the cell that owns it.
pub(crate) struct CollapsedBorders {
  edges: Vec<Vec<[BorderEdge; 4]>>,
}

/// Which cell covers each track, so a shared line can see both sides.
type OwnerGrid = Vec<Vec<Option<(usize, usize)>>>;

impl CollapsedBorders {
  pub(crate) fn resolve(
    table: &ComputedStyle,
    rows: &[RenderNode],
    placements: &[Vec<(usize, u16)>],
    columns: usize,
  ) -> Self {
    let cells = row_cells(rows);
    let owner = owner_grid(&cells, placements, rows.len(), columns);
    let mut edges = Vec::with_capacity(rows.len());

    for (index, (row, positions)) in rows.iter().zip(placements).enumerate() {
      let mut line = Vec::with_capacity(positions.len());

      for (cell_index, &(column, colspan)) in positions.iter().enumerate() {
        let Some(cell) = cells[index].get(cell_index) else {
          break;
        };

        let sizing = &cell.context.sizing;
        let span = usize::from(colspan);
        let rowspan = usize::from(cell.span_attribute("rowspan", MAX_ROWSPAN));
        let last_row = index + rowspan >= rows.len();
        let last_column = column + span >= columns;
        let mut top = Vec::new();

        if let Some(above_index) = index.checked_sub(1) {
          for track in column..(column + span).min(columns) {
            let Some(&Some((above_row, above_cell))) =
              owner.get(above_index).and_then(|line| line.get(track))
            else {
              continue;
            };
            let Some(above) = cells[above_row].get(above_cell) else {
              continue;
            };

            top.push(Side::Bottom.of(&above.context.style, sizing, EdgeOrigin::Cell));
          }

          top.push(Side::Bottom.of(&rows[above_index].context.style, sizing, EdgeOrigin::Row));
        }

        top.push(Side::Top.of(&cell.context.style, sizing, EdgeOrigin::Cell));
        top.push(Side::Top.of(&row.context.style, sizing, EdgeOrigin::Row));

        if index == 0 {
          top.push(Side::Top.of(table, sizing, EdgeOrigin::Table));
        }

        let mut left = Vec::new();

        if let Some(&Some((left_row, left_cell))) = column
          .checked_sub(1)
          .and_then(|track| owner.get(index).and_then(|line| line.get(track)))
          && let Some(neighbour) = cells[left_row].get(left_cell)
        {
          left.push(Side::Right.of(&neighbour.context.style, sizing, EdgeOrigin::Cell));
        }

        left.push(Side::Left.of(&cell.context.style, sizing, EdgeOrigin::Cell));

        if column == 0 {
          left.push(Side::Left.of(&row.context.style, sizing, EdgeOrigin::Row));
          left.push(Side::Left.of(table, sizing, EdgeOrigin::Table));
        }

        let bottom = last_row.then(|| {
          let final_row = &rows[(index + rowspan - 1).min(rows.len() - 1)];

          vec![
            Side::Bottom.of(&cell.context.style, sizing, EdgeOrigin::Cell),
            Side::Bottom.of(&final_row.context.style, sizing, EdgeOrigin::Row),
            Side::Bottom.of(table, sizing, EdgeOrigin::Table),
          ]
        });
        let right = last_column.then(|| {
          vec![
            Side::Right.of(&cell.context.style, sizing, EdgeOrigin::Cell),
            Side::Right.of(&row.context.style, sizing, EdgeOrigin::Row),
            Side::Right.of(table, sizing, EdgeOrigin::Table),
          ]
        });

        line.push([
          win(&top),
          right.map_or_else(BorderEdge::none, |candidates| win(&candidates)),
          bottom.map_or_else(BorderEdge::none, |candidates| win(&candidates)),
          win(&left),
        ]);
      }

      edges.push(line);
    }

    Self { edges }
  }

  pub(crate) fn apply(&self, row: usize, cell_index: usize, style: &mut ComputedStyle) {
    let Some(edges) = self.edges.get(row).and_then(|line| line.get(cell_index)) else {
      return;
    };

    for (side, edge) in [Side::Top, Side::Right, Side::Bottom, Side::Left]
      .into_iter()
      .zip(edges)
    {
      side.apply(style, *edge);
    }
  }
}

/// css-backgrounds-3 border-style: the collapsing model draws `outset` as
/// `groove` and `inset` as `ridge`, so both resolve as the style they draw.
fn collapsed_style(style: BorderStyle) -> BorderStyle {
  match style {
    BorderStyle::Outset => BorderStyle::Groove,
    BorderStyle::Inset => BorderStyle::Ridge,
    other => other,
  }
}

/// `hidden` clears the line.
fn win(candidates: &[BorderEdge]) -> BorderEdge {
  if candidates
    .iter()
    .any(|edge| edge.style == BorderStyle::Hidden)
  {
    return BorderEdge::none();
  }

  candidates
    .iter()
    .copied()
    .reduce(|winner, edge| {
      if edge.rank() > winner.rank() {
        edge
      } else {
        winner
      }
    })
    .unwrap_or_else(BorderEdge::none)
}

fn row_cells(rows: &[RenderNode]) -> Vec<Vec<&RenderNode>> {
  rows
    .iter()
    .map(|row| {
      row
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|cell| cell.is_cell())
        .collect()
    })
    .collect()
}

fn owner_grid(
  cells: &[Vec<&RenderNode>],
  placements: &[Vec<(usize, u16)>],
  rows: usize,
  columns: usize,
) -> OwnerGrid {
  let mut grid = vec![vec![None; columns]; rows];

  for (index, positions) in placements.iter().enumerate() {
    for (cell_index, &(column, colspan)) in positions.iter().enumerate() {
      let Some(cell) = cells[index].get(cell_index) else {
        break;
      };

      let rowspan = usize::from(cell.span_attribute("rowspan", MAX_ROWSPAN));

      for line in grid
        .iter_mut()
        .take((index + rowspan).min(rows))
        .skip(index)
      {
        for track in line
          .iter_mut()
          .take((column + usize::from(colspan)).min(columns))
          .skip(column)
        {
          *track = Some((index, cell_index));
        }
      }
    }
  }

  grid
}
