//! Builds a tagged-PDF structure tree from the HTML-derived node tree.
//!
//! The emitters record a marked-content identifier per source node while
//! drawing; afterwards the source tree is walked in logical order and nodes
//! with a semantic HTML tag become structure elements owning those
//! identifiers. Containers without a role lift their content to the nearest
//! tagged ancestor, and bare text under an untagged ancestor wraps in `P` so
//! no content stays outside the tree.

use std::{
  collections::{HashMap, HashSet},
  mem,
  num::NonZeroU16,
};

use takumi_core::{
  layout::tree::RenderNode,
  style::{Display, FlexDirection},
};

use crate::krilla::tagging::{Identifier, ListNumbering, Tag, TagGroup, TagId, TagKind, TagTree};

/// Marked-content identifiers recorded during emission, keyed by the source
/// node's path from the root.
#[derive(Default)]
pub(crate) struct TagCollector {
  identifiers: HashMap<Vec<usize>, Vec<Identifier>>,
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
  let identifiers = walk.collector.take(path);
  let mut annotations = walk.collector.take_annotations(path);

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
      let mut has_content = !identifiers.is_empty();

      // `LI` only admits `Lbl`/`LBody` children, so its whole subtree wraps
      // in a single body element.
      if is_list_item {
        child_pending.extend(identifiers);
      } else {
        for identifier in identifiers {
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
      if has_content || is_target {
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
      // Items of a row-direction flex container read as one visual line, so
      // their block boundaries do not split the paragraph run.
      let block = is_block(node) && !nesting.in_row;

      if block {
        flush_paragraph(pending, parent);
      }
      pending.extend(identifiers);
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
    "ul" => Some(Tag::L(ListNumbering::Disc).into()),
    "ol" => Some(Tag::L(ListNumbering::Decimal).into()),
    "li" => Some(Tag::LI.into()),
    "strong" | "b" => Some(Tag::Strong.into()),
    "em" | "i" => Some(Tag::Em.into()),
    "code" => Some(Tag::Code.into()),
    "figcaption" => Some(Tag::Caption.into()),
    _ => None,
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
