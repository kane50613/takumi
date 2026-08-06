//! Builds a tagged-PDF structure tree from the HTML-derived node tree.
//!
//! The emitters record a marked-content identifier per source node while
//! drawing; afterwards the source tree is walked in logical order and nodes
//! with a semantic HTML tag become structure elements owning those
//! identifiers. Containers without a role lift their content to the nearest
//! tagged ancestor, and bare text under an untagged ancestor wraps in `P` so
//! no content stays outside the tree.

use std::{collections::HashMap, num::NonZeroU16};

use takumi_core::{
  layout::tree::RenderNode,
  style::{Display, FlexDirection},
};

use crate::krilla::tagging::{Identifier, ListNumbering, Tag, TagGroup, TagKind, TagTree};

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
pub(crate) fn build_tag_tree(
  root: &RenderNode,
  lang: Option<&str>,
  collector: &mut TagCollector,
) -> TagTree {
  let mut tree = TagTree::new().with_lang(lang.map(str::to_string));
  let mut top = Vec::new();
  let mut pending = Vec::new();

  build_node(
    root,
    &mut Vec::new(),
    collector,
    &mut top,
    &mut pending,
    false,
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

fn build_node(
  node: &RenderNode,
  path: &mut Vec<usize>,
  collector: &mut TagCollector,
  parent: &mut Vec<TagGroup>,
  pending: &mut Vec<Identifier>,
  in_row: bool,
) {
  let identifiers = collector.take(path);
  let annotations = collector.take_annotations(path);

  match role(node) {
    Some(kind) => {
      flush_paragraph(pending, parent);
      let is_link = matches!(kind, TagKind::Link(_));
      let is_list_item = matches!(kind, TagKind::LI(_));
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
      build_children(node, path, collector, &mut children, &mut child_pending);
      flush_paragraph(&mut child_pending, &mut children);
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
      } else {
        push_link_wrappers(annotations, parent);
      }
      // Inline formatting inside a text run leaves its element without any
      // content of its own; an empty structure element is pure noise.
      if has_content {
        parent.push(group);
      }
    }
    None => {
      // Items of a row-direction flex container read as one visual line, so
      // their block boundaries do not split the paragraph run.
      let block = is_block(node) && !in_row;

      if block {
        flush_paragraph(pending, parent);
      }
      pending.extend(identifiers);
      push_link_wrappers(annotations, parent);
      build_children(node, path, collector, parent, pending);
      if block {
        flush_paragraph(pending, parent);
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
  collector: &mut TagCollector,
  parent: &mut Vec<TagGroup>,
  pending: &mut Vec<Identifier>,
) {
  let Some(children) = node.children.as_ref() else {
    return;
  };
  let in_row = is_row_flex(node);

  for (index, child) in children.iter().enumerate() {
    path.push(index);
    build_node(child, path, collector, parent, pending, in_row);
    path.pop();
  }
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

fn role(node: &RenderNode) -> Option<TagKind> {
  let source = node.node.as_ref()?;
  let tag_name = source.tag_name()?;

  match tag_name {
    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
      let level = tag_name.as_bytes()[1] - b'0';
      let title = Some(text_content(node)).filter(|title| !title.is_empty());

      NonZeroU16::new(u16::from(level)).map(|level| Tag::Hn(level, title).into())
    }
    "p" => Some(Tag::P.into()),
    "img" => Some(Tag::Figure(source.alt().map(str::to_string)).into()),
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

fn text_content(node: &RenderNode) -> String {
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
