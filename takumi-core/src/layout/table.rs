//! Lowering `display: table` onto the grid layout algorithm.
//!
//! taffy has no table algorithm, but grid already provides the one thing a
//! table needs and flex cannot give: a column track shared by every row. So a
//! table subtree is rewritten into a single grid whose items are the cells,
//! with the row and row-group boxes dropped — they exist to group, and grid
//! rows come from the track count instead. A row's background is copied onto
//! its cells on the way out, since striping and header bands live there.
//!
//! This is auto table layout approximated by `auto` tracks. It follows Blink
//! where following is cheap (row group order, `border-spacing`) and diverges
//! where the grid algorithm cannot express a table: `border-collapse` and row
//! borders are not implemented.
//!
//! `vertical-align: middle` and `bottom` on a cell are lowered to a flex
//! column: the cell box still stretches to its grid area so borders and
//! backgrounds cover it, and its content moves inside that box. `baseline`
//! and `top` both leave the content at the top, so a `baseline` cell is only
//! right when its neighbors share a first-line baseline.
//!
//! The width distribution differs too. Blink's `kAboveMax` branch
//! (`table_layout_utils.cc`) grows each auto column by
//! `excess × max_content_column / Σ max_content`, so the columns end up scaled
//! in proportion to their content; grid spreads free space equally across
//! `auto` tracks. Two columns of equal content land on the same pixel either
//! way, and wider tables drift — measured at 15–48pt against headless Chrome on
//! a three- and four-column fixture.

use crate::{
  layout::{node::NodeKind, tree::RenderNode},
  style::{
    CaptionSide, ColorInput, Display, FlexDirection, FromCssStr, Gap, GridPlacement,
    GridPlacementSpan, GridTemplateComponents, JustifyContent, Length, ToCss, VerticalAlign,
    VerticalAlignKeyword,
  },
};

/// Blink's `table { border-spacing: 2px }`.
const DEFAULT_BORDER_SPACING_PX: f32 = 2.0;

/// Blink's `kMaxColSpan` (`core/html/table_constants.h`).
const MAX_COLSPAN: u16 = 1000;

/// Blink's `kMaxRowSpan`.
const MAX_ROWSPAN: u16 = 65534;

/// Rewrites every `display: table` box in the tree into a grid.
pub(crate) fn lower_tables(node: &mut RenderNode) {
  if node.context.style.display == Display::Table {
    lower_table(node);
  }

  if let Some(children) = node.children.as_mut() {
    for child in children {
      lower_tables(child);
    }
  }
}

/// How many tracks a cell occupies, from `colspan` / `rowspan`.
///
/// The name is matched case-insensitively: HTML parsing lowercases attributes,
/// but JSX hands `colSpan` and `rowSpan` through verbatim. Values are clamped
/// the way Blink clamps them, which also keeps the track count in range.
fn span_attribute(cell: &RenderNode, name: &str, max: u16) -> u16 {
  cell
    .node
    .as_ref()
    .and_then(|node| node.metadata.attributes.as_ref())
    .and_then(|attributes| {
      attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
    })
    .and_then(|value| value.trim().parse::<u32>().ok())
    .map_or(1, |value| value.clamp(1, u32::from(max)) as u16)
}

/// Row groups render header first and footer last, whatever the source order.
fn group_order(display: Display) -> u8 {
  match display {
    Display::TableHeaderGroup => 0,
    Display::TableFooterGroup => 2,
    _ => 1,
  }
}

/// The rows of a table, with row groups flattened away and reordered, plus how
/// many leading rows came from header groups.
///
/// Anything that is neither a row, a row group, nor a caption stays where it is
/// as a full-width item, which keeps stray content visible instead of dropping
/// it on the floor. Real anonymous table box fixup is not implemented.
fn collect_rows(
  table: &mut RenderNode,
) -> (Vec<RenderNode>, Vec<RenderNode>, usize, Vec<RenderNode>) {
  let mut captions = Vec::new();
  let mut groups: Vec<(u8, usize, Vec<RenderNode>)> = Vec::new();
  let mut strays = Vec::new();

  let children = table.children.take().map_or_else(Vec::new, Vec::from);

  for (index, mut child) in children.into_iter().enumerate() {
    match child.context.style.display {
      Display::TableCaption => captions.push(child),
      Display::TableRow => groups.push((1, index, vec![child])),
      Display::TableHeaderGroup | Display::TableRowGroup | Display::TableFooterGroup => {
        let order = group_order(child.context.style.display);
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

  groups.sort_by_key(|(order, index, _)| (*order, *index));

  let header_rows = groups
    .iter()
    .filter(|(order, ..)| *order == 0)
    .map(|(.., rows)| rows.len())
    .sum();
  let rows = groups.into_iter().flat_map(|(.., rows)| rows).collect();

  (captions, rows, header_rows, strays)
}

/// Each row's cells as `(column, colspan)`, advanced past tracks a preceding
/// row's `rowspan` still covers — the occupancy grid auto-placement will see.
fn resolve_columns(rows: &[RenderNode]) -> Vec<Vec<(usize, u16)>> {
  // Per track, how many more rows a rowspan keeps it covered.
  let mut covered: Vec<u16> = Vec::new();
  let mut placements = Vec::with_capacity(rows.len());

  for row in rows {
    let mut column = 0usize;
    let mut cells = Vec::new();

    for cell in row.children.as_deref().unwrap_or_default() {
      if !is_cell(cell) {
        continue;
      }

      while covered.get(column).is_some_and(|rows_left| *rows_left > 0) {
        column += 1;
      }

      let colspan = span_attribute(cell, "colspan", MAX_COLSPAN);
      let rowspan = span_attribute(cell, "rowspan", MAX_ROWSPAN);
      let end = column + usize::from(colspan);

      if covered.len() < end {
        covered.resize(end, 0);
      }

      for track in &mut covered[column..end] {
        *track = rowspan;
      }

      cells.push((column, colspan));
      column = end;
    }

    placements.push(cells);

    for track in &mut covered {
      *track = track.saturating_sub(1);
    }
  }

  placements
}

/// Widest row, counting `colspan` and rowspan occupancy, which is how many
/// tracks the grid needs.
fn track_count(placements: &[Vec<(usize, u16)>]) -> u16 {
  placements
    .iter()
    .flatten()
    .map(|(column, colspan)| *column as u32 + u32::from(*colspan))
    .max()
    .unwrap_or(1)
    .clamp(1, u32::from(MAX_COLSPAN)) as u16
}

/// True for a row child the lowering places as a cell.
///
/// Blink wraps a row child that is not a `table-cell` in an anonymous cell, so
/// a `<td style="display: flex">` is laid out instead of dropped. Only element
/// children qualify: real anonymous table box fixup, which would also wrap the
/// text between cells, is not implemented.
fn is_cell(child: &RenderNode) -> bool {
  let display = child.context.style.display;

  display == Display::TableCell
    || (display != Display::None
      && child
        .node
        .as_ref()
        .is_some_and(|node| !matches!(node.kind, NodeKind::Text(_))))
}

/// Moves a cell's content down its box for `vertical-align: middle` and
/// `bottom`.
///
/// The cell becomes a flex column holding one anonymous block, so the content
/// is a single flex item `justify-content` can move while the cell box keeps
/// stretching to the row. The wrapper also keeps the content's own formatting
/// context: making the cell itself the flex container would turn each inline
/// run into a separate flex item.
fn align_cell_content(cell: &mut RenderNode) {
  let justify = match cell.context.style.vertical_align {
    VerticalAlign::Keyword(VerticalAlignKeyword::Middle) => JustifyContent::Center,
    VerticalAlign::Keyword(VerticalAlignKeyword::Bottom) => JustifyContent::FlexEnd,
    _ => return,
  };
  let Some(children) = cell.children.take() else {
    return;
  };

  let content = RenderNode::anonymous_block_container(&cell.context, children.into_vec());
  let style = &mut cell.context.style;

  style.display = Display::Flex;
  style.flex_direction = FlexDirection::Column;
  style.justify_content = justify;

  cell.children = Some(Box::new([content]));
}

/// Turns a cell into a grid item at its resolved position. The column is
/// explicit rather than auto-placed: taffy's placement cursor does not return
/// to the row start on a row a `rowspan` reaches into.
fn lower_cell(cell: &mut RenderNode, line: i16, column: usize, colspan: u16) {
  let rowspan = span_attribute(cell, "rowspan", MAX_ROWSPAN);

  align_cell_content(cell);

  if cell.context.style.display == Display::TableCell {
    cell.context.style.display = Display::Block;
  }

  cell.context.style.grid_row_start = GridPlacement::Line(line);
  cell.context.style.grid_row_end = GridPlacement::Span(GridPlacementSpan::Span(rowspan));
  cell.context.style.grid_column_start = GridPlacement::Line(column as i16 + 1);
  cell.context.style.grid_column_end = GridPlacement::Span(GridPlacementSpan::Span(colspan));
}

/// Copies a row's background onto a cell that has none of its own.
///
/// The row box is dropped by the lowering, so its background would go with it —
/// and zebra striping and header bands are set on the row. Painting it per cell
/// leaves the `border-spacing` gaps unpainted, where a real table row would
/// cover them. Row borders are lost outright.
fn inherit_row_background(row: &RenderNode, cell: &mut RenderNode) {
  let row_style = &row.context.style;

  if row_style.background_color == ColorInput::transparent() && row_style.background_image.is_none()
  {
    return;
  }

  let cell_style = &mut cell.context.style;

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

/// Spans the full width of one row, like a caption or a stray child.
fn lower_full_width(node: &mut RenderNode, line: i16, columns: u16) {
  node.context.style.display = Display::Block;
  node.context.style.grid_row_start = GridPlacement::Line(line);
  node.context.style.grid_row_end = GridPlacement::Span(GridPlacementSpan::Span(1));
  node.context.style.grid_column_start = GridPlacement::Line(1);
  node.context.style.grid_column_end = GridPlacement::Span(GridPlacementSpan::Span(columns));
}

fn lower_table(table: &mut RenderNode) {
  let (captions, rows, header_rows, strays) = collect_rows(table);
  let placements = resolve_columns(&rows);
  let columns = track_count(&placements);
  let tracks = track_sizes(&rows, &placements, columns);
  let mut items = Vec::new();
  let mut line: i16 = 1;
  let (top_captions, bottom_captions): (Vec<_>, Vec<_>) = captions
    .into_iter()
    .partition(|caption| caption.context.style.caption_side == CaptionSide::Top);

  for mut caption in top_captions {
    lower_full_width(&mut caption, line, columns);
    items.push(caption);
    line = line.saturating_add(1);
  }

  // A header that is the whole table has nothing to repeat over.
  if header_rows > 0 && header_rows < rows.len() {
    let start = line;

    table.table_header_lines = Some((start, start.saturating_add(header_rows as i16)));
  }

  for (mut row, positions) in rows.into_iter().zip(placements) {
    let cells = row.children.take().map_or_else(Vec::new, Vec::from);
    let mut positions = positions.into_iter();

    for mut cell in cells {
      if !is_cell(&cell) {
        continue;
      }

      let Some((column, colspan)) = positions.next() else {
        break;
      };

      inherit_row_background(&row, &mut cell);
      lower_cell(&mut cell, line, column, colspan);
      items.push(cell);
    }

    line = line.saturating_add(1);
  }

  for mut stray in strays {
    lower_full_width(&mut stray, line, columns);
    items.push(stray);
    line = line.saturating_add(1);
  }

  for mut caption in bottom_captions {
    lower_full_width(&mut caption, line, columns);
    items.push(caption);
    line = line.saturating_add(1);
  }

  let style = &mut table.context.style;

  style.display = Display::Grid;
  // `auto` tracks put free space into the columns rather than between them,
  // which is the shape of Blink's auto table layout.
  style.grid_template_columns = Some(tracks);

  if style.column_gap == Gap::Normal {
    style.column_gap = Gap::Length(Length::Px(DEFAULT_BORDER_SPACING_PX));
  }

  if style.row_gap == Gap::Normal {
    style.row_gap = Gap::Length(Length::Px(DEFAULT_BORDER_SPACING_PX));
  }

  table.children = Some(items.into_boxed_slice());
}

/// Track sizes for the grid: a column whose first single-column cell declares a
/// width gets that length, the rest stay `auto`.
///
/// This is Blink's constrained-versus-auto column split, decided from the cell's
/// specified width the same way, but read off the first row that states one
/// instead of resolving the whole column.
fn track_sizes(
  rows: &[RenderNode],
  placements: &[Vec<(usize, u16)>],
  columns: u16,
) -> GridTemplateComponents {
  let mut tracks = vec![String::from("auto"); usize::from(columns)];

  for (row, cells) in rows.iter().zip(placements) {
    let table_cells = row
      .children
      .as_deref()
      .unwrap_or_default()
      .iter()
      .filter(|cell| is_cell(cell));

    for (cell, (column, colspan)) in table_cells.zip(cells) {
      let width = &cell.context.style.width;

      if *colspan == 1
        && *width != Length::Auto
        && tracks.get(*column).is_some_and(|track| track == "auto")
      {
        tracks[*column] = width.to_css_string();
      }
    }
  }

  GridTemplateComponents::from_css_str(&tracks.join(" ")).unwrap_or_default()
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use crate::{
    context::RenderContext,
    layout::{node::Node, tree::RenderNode},
    resources::font::Fonts,
    style::{
      Color, ColorInput, Display, FlexDirection, GridPlacement, GridPlacementSpan, JustifyContent,
      Length, SizingContext, Style, StyleDeclaration, StyleSheet, ToCss,
    },
    viewport::Viewport,
  };

  /// Lowers a tree whose displays come from a stylesheet, standing in for the
  /// element presets the HTML and JSX front ends apply.
  fn lower(root: Node) -> RenderNode {
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
        .flex { display: flex }
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

    RenderNode::from_node(&context, root)
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
    assert_eq!(cell.context.style.justify_content, JustifyContent::Center);
    assert_eq!(cell.children.as_deref().expect("wrapped content").len(), 1);
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

    assert_eq!(ids(&tree), ["flex", "b"]);

    let cell = &tree.children.as_deref().expect("children")[0];

    assert_eq!(cell.context.style.display, Display::Flex);
    assert_eq!(cell.context.style.grid_column_start, GridPlacement::Line(1));
  }

  fn with_span(node: Node, name: &str, value: &str) -> Node {
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert(name.into(), value.into());

    node.with_attributes(attributes)
  }

  #[test]
  fn jsx_camel_case_span_counts_the_same_as_the_html_attribute() {
    for name in ["colSpan", "colspan"] {
      let tree = lower(
        Node::container([row([with_span(cell("wide"), name, "2"), cell("c")])])
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

    // Blink's kMaxColSpan, and the reason summing the row cannot wrap.
    assert_eq!(
      tree.children.as_deref().expect("children")[0]
        .context
        .style
        .grid_column_end,
      GridPlacement::Span(GridPlacementSpan::Span(1000))
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

    assert_eq!(
      tree
        .context
        .style
        .grid_template_columns
        .as_ref()
        .expect("template")
        .to_css_string(),
      "220px auto"
    );
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

    assert_eq!(
      tree
        .context
        .style
        .grid_template_columns
        .as_ref()
        .expect("template")
        .to_css_string(),
      "auto 220px"
    );
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
}
