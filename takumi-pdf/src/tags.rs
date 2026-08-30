//! Builds a tagged-PDF structure tree from the HTML-derived node tree.
//!
//! The emitters record a marked-content identifier per source node while
//! drawing; afterwards the source tree is walked in logical order and nodes
//! with a semantic HTML tag become structure elements owning those
//! identifiers. Containers without a role lift their content to the nearest
//! tagged ancestor, and bare text under an untagged ancestor wraps in `P` so
//! no content stays outside the tree. Lowered tables rebuild their rows and
//! row groups from the [`TablePart`] stamps lowering leaves behind.

use std::{
  collections::{HashMap, HashSet},
  mem,
  num::{NonZeroU16, NonZeroU32},
};

use takumi_core::{
  layout::tree::{RenderNode, TablePart},
  style::{Display, FlexDirection, GridPlacement, GridPlacementSpan, ListStyleType},
};

use crate::krilla::tagging::{
  Identifier, ListNumbering, TableHeaderScope, Tag, TagGroup, TagId, TagKind, TagTree,
};

/// Marked-content identifiers recorded during emission, keyed by the source
/// node's path from the root.
#[derive(Default)]
pub(crate) struct TagCollector {
  identifiers: HashMap<Vec<usize>, Vec<Identifier>>,
  /// Generated list-label identifiers per source list item.
  labels: HashMap<Vec<usize>, Vec<Identifier>>,
  /// Link-annotation identifiers per source node, joined into that node's
  /// `Link` element (or wrapped in one) so annotations sit inside the tree.
  annotations: HashMap<Vec<usize>, Vec<Identifier>>,
}

impl TagCollector {
  pub(crate) fn record(&mut self, path: &[usize], identifier: Identifier) {
    self
      .identifiers
      .entry(path.to_vec())
      .or_default()
      .push(identifier);
  }

  pub(crate) fn record_label(&mut self, path: &[usize], identifier: Identifier) {
    self
      .labels
      .entry(path.to_vec())
      .or_default()
      .push(identifier);
  }

  pub(crate) fn record_annotation(&mut self, path: &[usize], identifier: Identifier) {
    self
      .annotations
      .entry(path.to_vec())
      .or_default()
      .push(identifier);
  }

  fn take(&mut self, path: &[usize]) -> Vec<Identifier> {
    self.identifiers.remove(path).unwrap_or_default()
  }

  fn take_labels(&mut self, path: &[usize]) -> Vec<Identifier> {
    self.labels.remove(path).unwrap_or_default()
  }

  fn take_annotations(&mut self, path: &[usize]) -> Vec<Identifier> {
    self.annotations.remove(path).unwrap_or_default()
  }
}

/// Walks the source tree in logical order and builds the structure tree from
/// the recorded identifiers.
/// The id a destination uses to name the structure element built from `path`.
pub(crate) fn tag_id(path: &[usize]) -> TagId {
  let mut bytes = Vec::with_capacity(path.len() * 3 + 1);

  bytes.push(b'n');
  for index in path {
    bytes.push(b'.');
    bytes.extend_from_slice(index.to_string().as_bytes());
  }
  TagId::from(bytes)
}

pub(crate) fn build_tag_tree(
  root: &RenderNode,
  lang: Option<&str>,
  collector: &mut TagCollector,
  targets: &HashSet<Vec<usize>>,
) -> TagTree {
  let mut tree = TagTree::new().with_lang(lang.map(str::to_string));
  let mut top = Vec::new();
  let mut pending = Vec::new();
  let mut walk = Walk {
    collector,
    headings: Vec::new(),
    targets,
  };

  build_node(
    root,
    &mut Vec::new(),
    &mut walk,
    &mut top,
    &mut pending,
    Nesting::default(),
  );
  flush_paragraph(&mut pending, &mut top);
  for group in top {
    tree.push(group);
  }
  tree
}

/// Drains a run of bare-content identifiers into a single `P`. Bare content
/// under an untagged ancestor still needs a structure parent, and one
/// paragraph per block container mirrors HTML text flow without emitting a
/// structure element per text node.
fn flush_paragraph(pending: &mut Vec<Identifier>, parent: &mut Vec<TagGroup>) {
  if pending.is_empty() {
    return;
  }
  let mut group = TagGroup::new(Tag::P);

  for identifier in pending.drain(..) {
    group.push(identifier);
  }
  parent.push(group);
}

/// State carried across the walk: the identifiers to place, plus the source
/// heading levels open at this point, whose depth becomes the emitted `Hn`.
struct Walk<'c> {
  collector: &'c mut TagCollector,
  headings: Vec<u8>,
  /// Paths a destination points at, whose structure elements need an id.
  targets: &'c HashSet<Vec<usize>>,
}

impl Walk<'_> {
  /// PDF/UA rejects a heading sequence that skips a level or opens below
  /// `H1`, which HTML happily writes. Numbering by nesting depth keeps the
  /// document's hierarchy and always produces a sequence validators accept.
  fn heading_level(&mut self, source: u8) -> NonZeroU16 {
    while self.headings.last().is_some_and(|open| *open >= source) {
      self.headings.pop();
    }
    self.headings.push(source);

    NonZeroU16::new(self.headings.len().min(6) as u16).unwrap_or(NonZeroU16::MIN)
  }
}

#[derive(Default, Clone, Copy)]
struct Nesting {
  /// Inside a row-direction flex container, whose items read as one line.
  in_row: bool,
  /// Inside an `L`, so a list item already has the parent it requires.
  in_list: bool,
  /// Inside a `Figure`, which already encloses the image's content.
  in_figure: bool,
}

fn build_node(
  node: &RenderNode,
  path: &mut Vec<usize>,
  walk: &mut Walk,
  parent: &mut Vec<TagGroup>,
  pending: &mut Vec<Identifier>,
  nesting: Nesting,
) {
  if node.table_part == Some(TablePart::Table) {
    flush_paragraph(pending, parent);
    build_table(node, path, walk, parent, nesting);
    return;
  }
  let mut role = role(node, walk, nesting);

  if role.is_none() && walk.targets.contains(path.as_slice()) {
    // A destination names a structure element, and PDF/UA-2 asks every link
    // inside a document to be one. A target that carries no meaning of its own
    // still has to exist for the link to land on.
    role = Some(Tag::P.into());
  }

  match role {
    Some(kind) => {
      flush_paragraph(pending, parent);
      let is_list_item = matches!(kind, TagKind::LI(_));

      if let Some(group) = build_element(node, path, walk, kind, nesting, false) {
        // An `LI` outside a list is invalid on its own, so it brings a list
        // of its own along.
        if is_list_item && !nesting.in_list {
          let mut list = TagGroup::new(Tag::L(ListNumbering::None));

          list.push(group);
          parent.push(list);
        } else {
          parent.push(group);
        }
      }
    }
    None => {
      let identifiers = walk.collector.take(path);
      let labels = walk.collector.take_labels(path);
      let annotations = walk.collector.take_annotations(path);
      // Items of a row-direction flex container read as one visual line, so
      // their block boundaries do not split the paragraph run.
      let block = is_block(node) && !nesting.in_row;

      if block {
        flush_paragraph(pending, parent);
      }
      pending.extend(labels.into_iter().chain(identifiers));
      build_children(node, path, walk, parent, pending, nesting);
      if block {
        flush_paragraph(pending, parent);
      }
      // The content a link annotation decorates sits in the run collected so
      // far; flush it first so the `Link` element follows it in reading order.
      if !annotations.is_empty() {
        flush_paragraph(pending, parent);
        push_link_wrappers(annotations, parent);
      }
    }
  }
}

/// Builds one structure element: the node's own identifiers, its subtree, and
/// its wrapped annotations under `kind`. `None` when the element would be
/// empty and nothing names it, unless `keep_empty` holds it in place — a table
/// cell with no content still counts toward its row's cell count.
fn build_element(
  node: &RenderNode,
  path: &mut Vec<usize>,
  walk: &mut Walk,
  kind: TagKind,
  nesting: Nesting,
  keep_empty: bool,
) -> Option<TagGroup> {
  let identifiers = walk.collector.take(path);
  let labels = walk.collector.take_labels(path);
  let mut annotations = walk.collector.take_annotations(path);
  let is_link = matches!(kind, TagKind::Link(_));
  let is_list_item = matches!(kind, TagKind::LI(_));
  let is_list = matches!(kind, TagKind::L(_));
  let is_figure = matches!(kind, TagKind::Figure(_));
  let mut kind = kind;

  let is_target = walk.targets.contains(path.as_slice());

  if is_target {
    kind.set_id(Some(tag_id(path)));
  }
  let mut group = TagGroup::new(kind);
  let mut children = Vec::new();
  let mut child_pending = Vec::new();
  let mut has_content = !identifiers.is_empty() || !labels.is_empty();

  // `LI` only admits `Lbl`/`LBody` children, so the marker label and the
  // item's whole subtree wrap in one of each.
  if is_list_item {
    if !labels.is_empty() {
      let mut label = TagGroup::new(Tag::Lbl);

      for identifier in labels {
        label.push(identifier);
      }
      group.push(label);
    }
    child_pending.extend(identifiers);
  } else {
    // A marker paints at the line's start, so its label reads first.
    for identifier in labels.into_iter().chain(identifiers) {
      group.push(identifier);
    }
  }
  build_children(
    node,
    path,
    walk,
    &mut children,
    &mut child_pending,
    Nesting {
      in_list: is_list,
      in_figure: nesting.in_figure || is_figure,
      ..nesting
    },
  );
  flush_paragraph(&mut child_pending, &mut children);
  // Wrapped annotations join the element's own children so they read
  // after the content they decorate, not before the element.
  if !is_link {
    push_link_wrappers(mem::take(&mut annotations), &mut children);
  }
  has_content |= !children.is_empty();
  if is_list_item {
    if !children.is_empty() {
      let mut body = TagGroup::new(Tag::LBody);

      for child in children {
        body.push(child);
      }
      group.push(body);
    }
  } else {
    for child in children {
      group.push(child);
    }
  }
  if is_link {
    has_content |= !annotations.is_empty();
    for annotation in annotations {
      group.push(annotation);
    }
  }
  // Inline formatting inside a text run leaves its element without any
  // content of its own; an empty structure element is pure noise. An
  // element a destination names is the exception: dropping it leaves the
  // link pointing at nothing.
  (has_content || is_target || keep_empty).then_some(group)
}

/// Rebuilds `Table → THead/TBody/TFoot → TR → TH/TD` from a lowered table,
/// whose render tree only has captions and cells left. Rows come back from
/// each cell's grid line, groups from the [`TablePart`] lowering stamped on
/// it. A page-spanning table stays one `Table`, as ISO 14289-2:2024 §8.2.2
/// requires: replayed header bands are artifacts, so only the first
/// occurrence carries content.
struct TableBuilder {
  /// Finished direct children of `Table`: captions, row groups, stray rows.
  groups: Vec<TagGroup>,
  /// The open row group and the rows collected for it so far.
  section: Option<(TablePart, Vec<TagGroup>)>,
  /// The open row: its grid line and the `TR` collecting its cells.
  row: Option<(i16, TagGroup)>,
}

impl TableBuilder {
  fn close_row(&mut self) {
    if let Some((_, row)) = self.row.take()
      && let Some((_, rows)) = self.section.as_mut()
    {
      rows.push(row);
    }
  }

  fn close_section(&mut self) {
    self.close_row();
    if let Some((part, rows)) = self.section.take() {
      let mut group = TagGroup::new(match part {
        TablePart::HeaderCell => TagKind::from(Tag::THead),
        TablePart::FooterCell => Tag::TFoot.into(),
        _ => Tag::TBody.into(),
      });

      for row in rows {
        group.push(row);
      }
      self.groups.push(group);
    }
  }

  /// The `TR` for `line` in a `part` row group, opening either as needed.
  fn row(&mut self, part: TablePart, line: i16) -> &mut TagGroup {
    if self.section.as_ref().is_none_or(|(open, _)| *open != part) {
      self.close_section();
      self.section = Some((part, Vec::new()));
    }
    if self.row.as_ref().is_some_and(|(open, _)| *open != line) {
      self.close_row();
    }
    &mut self
      .row
      .get_or_insert_with(|| (line, TagGroup::new(Tag::TR)))
      .1
  }

  /// Content that was never a cell still needs a place inside the table's
  /// content model, so it rides in a row of its own.
  fn push_stray(&mut self, children: Vec<TagGroup>) {
    self.close_section();

    let mut cell = TagGroup::new(Tag::TD);

    for child in children {
      cell.push(child);
    }
    let mut row = TagGroup::new(Tag::TR);

    row.push(cell);
    self.groups.push(row);
  }

  fn push_caption(&mut self, caption: TagGroup) {
    self.close_section();
    self.groups.push(caption);
  }
}

/// The span a lowered cell carries on a grid axis.
fn placement_span(placement: &GridPlacement) -> Option<NonZeroU32> {
  match placement {
    GridPlacement::Span(GridPlacementSpan::Span(span)) if *span > 1 => {
      NonZeroU32::new((*span).into())
    }
    _ => None,
  }
}

/// `TH` or `TD` for a lowered cell. The source tag decides when there is one;
/// a tagless cell in a header row group is still a header. A `TH` takes its
/// `Scope` from the `scope` attribute, defaulting to `Column` in a header row
/// group and `Row` elsewhere, per ISO 32000-2 §14.8.4.8.3's algorithm.
fn cell_kind(cell: &RenderNode, part: TablePart) -> TagKind {
  let source = cell.node.as_ref();
  let tag_name = source.and_then(|node| node.tag_name());
  let row_span = placement_span(&cell.context.style.grid_row_end);
  let col_span = placement_span(&cell.context.style.grid_column_end);
  let is_header = match tag_name {
    Some("th") => true,
    Some("td") => false,
    _ => part == TablePart::HeaderCell,
  };

  if !is_header {
    return Tag::TD
      .with_row_span(row_span)
      .with_col_span(col_span)
      .into();
  }
  let scope = source
    .and_then(|node| node.scope())
    .and_then(|scope| match scope.trim() {
      value if value.eq_ignore_ascii_case("row") || value.eq_ignore_ascii_case("rowgroup") => {
        Some(TableHeaderScope::Row)
      }
      value if value.eq_ignore_ascii_case("col") || value.eq_ignore_ascii_case("colgroup") => {
        Some(TableHeaderScope::Column)
      }
      _ => None,
    })
    .unwrap_or(if part == TablePart::HeaderCell {
      TableHeaderScope::Column
    } else {
      TableHeaderScope::Row
    });

  Tag::TH(scope)
    .with_row_span(row_span)
    .with_col_span(col_span)
    .into()
}

fn build_table(
  node: &RenderNode,
  path: &mut Vec<usize>,
  walk: &mut Walk,
  parent: &mut Vec<TagGroup>,
  nesting: Nesting,
) {
  let identifiers = walk.collector.take(path);
  let annotations = walk.collector.take_annotations(path);
  let mut kind: TagKind = Tag::Table.into();
  let is_target = walk.targets.contains(path.as_slice());

  if is_target {
    kind.set_id(Some(tag_id(path)));
  }
  let mut builder = TableBuilder {
    groups: Vec::new(),
    section: None,
    row: None,
  };
  // A list or flex row outside the table does not reach into its cells; an
  // enclosing `Figure` does.
  let nesting = Nesting {
    in_row: false,
    in_list: false,
    in_figure: nesting.in_figure,
  };

  // The table box itself has no content slot in the model, so anything
  // recorded against it rides in a stray row like non-cell children do.
  if !identifiers.is_empty() {
    let mut wrap = Vec::new();
    let mut pending = identifiers;

    flush_paragraph(&mut pending, &mut wrap);
    builder.push_stray(wrap);
  }
  for (index, child) in node.children.iter().flatten().enumerate() {
    path.push(index);
    match child.table_part {
      Some(TablePart::Caption) => {
        if let Some(caption) = build_element(child, path, walk, Tag::Caption.into(), nesting, false)
        {
          builder.push_caption(caption);
        }
      }
      Some(part @ (TablePart::HeaderCell | TablePart::BodyCell | TablePart::FooterCell)) => {
        let GridPlacement::Line(line) = child.context.style.grid_row_start else {
          path.pop();
          continue;
        };
        let kind = cell_kind(child, part);

        // An empty cell still holds its place in the row, so the row keeps
        // the cell count the spec's regularity asks for.
        if let Some(cell) = build_element(child, path, walk, kind, nesting, true) {
          builder.row(part, line).push(cell);
        }
      }
      _ => {
        let mut wrap = Vec::new();
        let mut pending = Vec::new();

        build_node(child, path, walk, &mut wrap, &mut pending, nesting);
        flush_paragraph(&mut pending, &mut wrap);
        if !wrap.is_empty() {
          builder.push_stray(wrap);
        }
      }
    }
    path.pop();
  }
  builder.close_section();

  let mut table = TagGroup::new(kind);
  let has_content = !builder.groups.is_empty();

  for group in builder.groups {
    table.push(group);
  }
  if has_content || is_target {
    parent.push(table);
  }
  push_link_wrappers(annotations, parent);
}

/// Wraps loose link-annotation identifiers (inline anchors have no painted
/// box of their own) in `Link` elements of their own.
fn push_link_wrappers(annotations: Vec<Identifier>, parent: &mut Vec<TagGroup>) {
  for annotation in annotations {
    let mut group = TagGroup::new(Tag::Link);

    group.push(annotation);
    parent.push(group);
  }
}

fn build_children(
  node: &RenderNode,
  path: &mut Vec<usize>,
  walk: &mut Walk,
  parent: &mut Vec<TagGroup>,
  pending: &mut Vec<Identifier>,
  nesting: Nesting,
) {
  let Some(children) = node.children.as_ref() else {
    return;
  };
  let nesting = Nesting {
    in_row: is_row_flex(node),
    ..nesting
  };

  for (index, child) in children.iter().enumerate() {
    path.push(index);
    build_node(child, path, walk, parent, pending, nesting);
    path.pop();
  }
}

/// The alternate description a `<figure>` takes from the image it illustrates.
fn figure_alt(node: &RenderNode) -> Option<String> {
  if let Some(source) = node.node.as_ref()
    && source.tag_name() == Some("img")
  {
    return source
      .alt()
      .filter(|alt| !alt.is_empty())
      .map(str::to_string);
  }
  node.children.iter().flatten().find_map(figure_alt)
}

/// Whether the node opens a block-level container, i.e. a paragraph boundary
/// for bare text runs.
fn is_block(node: &RenderNode) -> bool {
  !matches!(
    node.context.style.display,
    Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid
  )
}

/// Whether the node lays its children out on one horizontal line.
fn is_row_flex(node: &RenderNode) -> bool {
  matches!(
    node.context.style.display,
    Display::Flex | Display::InlineFlex
  ) && matches!(
    node.context.style.flex_direction,
    FlexDirection::Row | FlexDirection::RowReverse
  )
}

fn role(node: &RenderNode, walk: &mut Walk, nesting: Nesting) -> Option<TagKind> {
  let source = node.node.as_ref()?;
  let tag_name = source.tag_name()?;

  match tag_name {
    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
      let title = Some(text_content(node)).filter(|title| !title.is_empty());

      // A heading with nothing in it is dropped further down, and numbering it
      // would still shift every heading that follows.
      if title.is_none()
        && node
          .children
          .as_ref()
          .is_none_or(|children| children.is_empty())
      {
        return None;
      }
      let level = walk.heading_level(tag_name.as_bytes()[1] - b'0');

      Some(Tag::Hn(level, title).into())
    }
    "p" => Some(Tag::P.into()),
    // A `Figure` encloses everything the illustration is made of, so the
    // caption has a parent to sit under and the image inside adds no element
    // of its own. Without an alternate description there is nothing to
    // enclose that a `Figure` would describe, and PDF/UA rejects one that
    // carries no text.
    "figure" => Some(Tag::Figure(Some(figure_alt(node)?)).into()),
    "img" if nesting.in_figure => None,
    // `alt=""` marks a decorative image: emitted as an artifact, no element.
    "img" if source.alt() != Some("") => Some(Tag::Figure(source.alt().map(str::to_string)).into()),
    "a" if source.href().is_some() => Some(Tag::Link.into()),
    "blockquote" => Some(Tag::BlockQuote.into()),
    "section" => Some(Tag::Section.into()),
    "article" => Some(Tag::Article.into()),
    "ul" | "ol" => Some(Tag::L(list_numbering(node)).into()),
    "li" => Some(Tag::LI.into()),
    "strong" | "b" => Some(Tag::Strong.into()),
    "em" | "i" => Some(Tag::Em.into()),
    "code" => Some(Tag::Code.into()),
    "figcaption" => Some(Tag::Caption.into()),
    _ => None,
  }
}

/// The numbering the list's counter style advertises; per-item overrides and
/// marker images are not consulted.
fn list_numbering(node: &RenderNode) -> ListNumbering {
  match &node.context.style.list_style_type {
    ListStyleType::None | ListStyleType::String(_) => ListNumbering::None,
    ListStyleType::Disc => ListNumbering::Disc,
    ListStyleType::Circle => ListNumbering::Circle,
    ListStyleType::Square => ListNumbering::Square,
    ListStyleType::Decimal | ListStyleType::DecimalLeadingZero => ListNumbering::Decimal,
    ListStyleType::LowerAlpha => ListNumbering::LowerAlpha,
    ListStyleType::UpperAlpha => ListNumbering::UpperAlpha,
    ListStyleType::LowerRoman => ListNumbering::LowerRoman,
    ListStyleType::UpperRoman => ListNumbering::UpperRoman,
    _ => ListNumbering::None,
  }
}

/// The text a node contributes, including the anonymous runs the layout
/// merges inline children into.
pub(crate) fn text_content(node: &RenderNode) -> String {
  let mut out = String::new();

  collect_text(node, &mut out);
  out.trim().to_string()
}

fn collect_text(node: &RenderNode, out: &mut String) {
  use takumi_core::layout::node::NodeKind;

  if let Some(NodeKind::Text(text)) = node.node.as_ref().map(|source| &source.kind) {
    out.push_str(&text.text);
  }
  if let Some(anonymous) = node.anonymous_text_content.as_deref() {
    out.push_str(anonymous);
  }
  if let Some(children) = node.children.as_ref() {
    for child in children.iter() {
      collect_text(child, out);
    }
  }
}
