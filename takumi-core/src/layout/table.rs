//! Lowering `display: table` onto the grid layout algorithm.
//!
//! taffy has no table algorithm. Grid gives every row a shared column track,
//! which flex cannot. Rows and row groups are dropped, so a row's background
//! and borders are copied onto its cells.
//!
//! Cell alignment mirrors Blink's `ComputeContentAlignment` in
//! `block_layout_algorithm_utils.cc`. `baseline` is naive: it uses block-start
//! borders and padding instead of a measured first baseline.
//!
//! Grid distributes free space evenly across `auto` tracks. Blink's
//! `kAboveMax` in `table_layout_utils.cc` uses max-content proportions. The
//! measured drift is 15–48pt against headless Chrome on three- and four-column
//! fixtures.

use taffy::LengthPercentageAuto;

use crate::{
  layout::{
    node::NodeKind,
    table_borders::CollapsedBorders,
    tree::{NodeOrigin, RenderNode},
  },
  style::{
    BorderCollapse, BorderStyle, CaptionSide, ColorInput, ComputedStyle, Display, FlexDirection,
    FromCssStr, Gap, GridPlacement, GridPlacementSpan, GridTemplateComponents, JustifyContent,
    Length, LineWidth, ToCss, VerticalAlign, VerticalAlignKeyword,
  },
};

/// Blink's `table { border-spacing: 2px }`.
const DEFAULT_BORDER_SPACING_PX: f32 = 2.0;

/// Blink's `kMaxColSpan` (`core/html/table_constants.h`).
const MAX_COLSPAN: u16 = 1000;

/// Blink's `kMaxRowSpan`.
pub(crate) const MAX_ROWSPAN: u16 = 65534;

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

/// Reads a case-insensitive span attribute within Blink's limit.
pub(crate) fn span_attribute(cell: &RenderNode, name: &str, max: u16) -> u16 {
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

fn group_order(display: Display) -> u8 {
  match display {
    Display::TableHeaderGroup => 0,
    Display::TableFooterGroup => 2,
    _ => 1,
  }
}

/// Extracts rows without CSS anonymous table-box fixup.
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

fn resolve_columns(rows: &[RenderNode]) -> Vec<Vec<(usize, u16)>> {
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

fn track_count(placements: &[Vec<(usize, u16)>]) -> u16 {
  placements
    .iter()
    .flatten()
    .map(|(column, colspan)| *column as u32 + u32::from(*colspan))
    .max()
    .unwrap_or(1)
    .clamp(1, u32::from(MAX_COLSPAN)) as u16
}

/// Recognizes authored cells without CSS anonymous table-box fixup.
pub(crate) fn is_cell(child: &RenderNode) -> bool {
  let display = child.context.style.display;

  if display == Display::TableCell {
    return true;
  }

  display != Display::None
    && matches!(child.origin, NodeOrigin::Authored { .. })
    && child
      .node
      .as_ref()
      .is_some_and(|node| !matches!(node.kind, NodeKind::Text(_)))
}

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

/// Wraps content to preserve its formatting context during alignment.
fn wrap_cell_content(cell: &mut RenderNode) -> Option<&mut RenderNode> {
  let children = cell.children.take()?;
  let content = RenderNode::anonymous_block_container(&cell.context, children.into_vec());

  cell.children = Some(Box::new([content]));
  cell.children.as_deref_mut()?.first_mut()
}

fn align_cell_content(cell: &mut RenderNode) {
  let Some(justify) = CellAlignment::of(&cell.context.style).justify_content() else {
    return;
  };

  if wrap_cell_content(cell).is_none() {
    return;
  }

  let style = &mut cell.context.style;

  style.display = Display::Flex;
  style.flex_direction = FlexDirection::Column;
  style.justify_content = justify;
}

/// Approximates the first-baseline offset as block-start border and padding.
/// It drops the first line's ascent and half-leading, so cells drift once a
/// row mixes fonts or line heights.
fn content_inset_top(cell: &RenderNode) -> f32 {
  let style = &cell.context.style;
  let sizing = &cell.context.sizing;
  let border = if style.border_top_style.is_rendered() {
    Length::from(style.border_top_width).to_px(sizing, 0.0)
  } else {
    0.0
  };

  border + style.padding_top.to_px(sizing, 0.0)
}

fn align_row_baselines(cells: &mut [RenderNode]) {
  let baselines: Vec<usize> = cells
    .iter()
    .enumerate()
    .filter(|(_, cell)| CellAlignment::of(&cell.context.style) == CellAlignment::Baseline)
    .map(|(index, _)| index)
    .collect();

  if baselines.len() < 2 {
    return;
  }

  let deepest = baselines
    .iter()
    .map(|&index| content_inset_top(&cells[index]))
    .fold(0.0, f32::max);

  for index in baselines {
    let shift = deepest - content_inset_top(&cells[index]);

    if shift <= 0.0 {
      continue;
    }

    if let Some(content) = wrap_cell_content(&mut cells[index])
      && let Some(layout_style) = content.layout_style_override.as_mut()
    {
      layout_style.margin.top = LengthPercentageAuto::length(shift);
    }
  }
}

/// Places the cell explicitly: taffy's cursor does not return to the row start
/// on a row a `rowspan` reaches into.
fn lower_cell(cell: &mut RenderNode, line: i16, column: usize, colspan: u16) {
  let rowspan = span_attribute(cell, "rowspan", MAX_ROWSPAN);

  if cell.context.style.display == Display::TableCell {
    align_cell_content(cell);

    if cell.context.style.display == Display::TableCell {
      cell.context.style.display = Display::Block;
    }
  }

  cell.context.style.grid_row_start = GridPlacement::Line(line);
  cell.context.style.grid_row_end = GridPlacement::Span(GridPlacementSpan::Span(rowspan));
  cell.context.style.grid_column_start = GridPlacement::Line(column as i16 + 1);
  cell.context.style.grid_column_end = GridPlacement::Span(GridPlacementSpan::Span(colspan));
}

/// Approximation: row backgrounds leave `border-spacing` gaps unpainted.
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
  let collapse = table.context.style.border_collapse == BorderCollapse::Collapse;
  let collapsed = collapse.then(|| {
    CollapsedBorders::resolve(
      &table.context.style,
      &rows,
      &placements,
      usize::from(columns),
    )
  });
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

  if header_rows > 0 && header_rows < rows.len() {
    let start = line;

    table.table_header_lines = Some((start, start.saturating_add(header_rows as i16)));
  }

  for (index, (mut row, positions)) in rows.into_iter().zip(placements).enumerate() {
    let mut cells = row.children.take().map_or_else(Vec::new, Vec::from);
    let mut positions = positions.into_iter();

    cells.retain(is_cell);
    align_row_baselines(&mut cells);

    for (cell_index, mut cell) in cells.into_iter().enumerate() {
      let Some((column, colspan)) = positions.next() else {
        break;
      };

      inherit_row_background(&row, &mut cell);

      if let Some(collapsed) = collapsed.as_ref() {
        collapsed.apply(index, cell_index, &mut cell.context.style);
      }

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
  style.grid_template_columns = Some(tracks);

  if collapse {
    style.column_gap = Gap::Length(Length::zero());
    style.row_gap = Gap::Length(Length::zero());
    clear_border(style);
  } else {
    if style.column_gap == Gap::Normal {
      style.column_gap = Gap::Length(Length::Px(DEFAULT_BORDER_SPACING_PX));
    }

    if style.row_gap == Gap::Normal {
      style.row_gap = Gap::Length(Length::Px(DEFAULT_BORDER_SPACING_PX));
    }
  }

  table.children = Some(items.into_boxed_slice());
}

/// The collapsed border lives on the edge cells, so the table box stops
/// painting its own.
fn clear_border(style: &mut ComputedStyle) {
  style.border_top_style = BorderStyle::None;
  style.border_right_style = BorderStyle::None;
  style.border_bottom_style = BorderStyle::None;
  style.border_left_style = BorderStyle::None;
  style.border_top_width = LineWidth::Length(Length::zero());
  style.border_right_width = LineWidth::Length(Length::zero());
  style.border_bottom_width = LineWidth::Length(Length::zero());
  style.border_left_width = LineWidth::Length(Length::zero());
}

/// Approximates Blink constrained columns from the first declared cell width.
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

  use taffy::LengthPercentageAuto;

  use crate::{
    context::RenderContext,
    layout::{node::Node, tree::RenderNode},
    resources::font::Fonts,
    style::{
      BorderStyle, Color, ColorInput, Display, FlexDirection, Gap, GridPlacement,
      GridPlacementSpan, JustifyContent, Length, SizingContext, Style, StyleDeclaration,
      StyleSheet, ToCss,
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
  fn a_baseline_cell_drops_to_the_deepest_padding_in_its_row() {
    let tree = lower(
      Node::container([row([
        Node::container([Node::text("padded")])
          .with_class_name("padded")
          .with_id("padded"),
        cell("flush"),
      ])])
      .with_class_name("table"),
    );

    let cells = tree.children.as_deref().expect("children");
    let margin_top = |cell: &RenderNode| {
      cell
        .children
        .as_deref()
        .and_then(<[RenderNode]>::first)
        .and_then(|content| content.layout_style_override.as_ref())
        .map(|style| style.margin.top)
    };

    assert_eq!(margin_top(&cells[0]), None);
    assert_eq!(
      margin_top(&cells[1]),
      Some(LengthPercentageAuto::length(10.0))
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

    assert_eq!(ids(&tree), ["flex", "b"]);

    let cell = &tree.children.as_deref().expect("children")[0];

    assert_eq!(cell.context.style.display, Display::Flex);
    assert_eq!(cell.context.style.flex_direction, FlexDirection::Row);
    assert_eq!(cell.context.style.grid_column_start, GridPlacement::Line(1));
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
}
