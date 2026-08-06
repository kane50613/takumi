//! Building outlines for the navigation of the document.
//!
//! An outline is a tree-like structure that stores the hierarchical structure of a document.
//! In particular, in most cases it is used to define the hierarchy of headings in the document.
//! For example, your document might consist of two first-level headings, which in turn have
//! more nested headings, which in turn might be nested even further, and so on. The [`Outline`]
//! allows you to encode this behavior, and makes it easier to navigate the document in PDF
//! viewers that support this feature.
//!
//! The intended usage is to create a new, empty outline in the very beginning. It represents
//! the root of the tree. Then, you can create new outline nodes, which you can recursively
//! nest in any way you wish while processing your document. In the end, you need to push
//! the first layer of children to the [`Outline`] object.
//!
//! Finally, once you are done building your outline tree, you can use the [`Document::set_outline`]
//! function of [`Document`] to store the outline in the document.
//!
//! [`Document`]: crate::krilla::Document
//! [`Document::set_outline`]: crate::krilla::Document::set_outline

use pdf_writer::writers::OutlineItem;
use pdf_writer::{Chunk, Finish, Name, Ref, TextStr};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::interactive::destination::XyzDestination;
use crate::krilla::serialize::SerializeContext;

/// An outline.
///
/// This represents the root of the outline tree.
#[derive(Debug, Clone)]
pub struct Outline {
  children: Vec<OutlineNode>,
}

impl Outline {
  pub(crate) fn is_empty(&self) -> bool {
    self.children.is_empty()
  }
}

impl Default for Outline {
  fn default() -> Self {
    Self::new()
  }
}

trait Outlineable {
  fn first(&mut self, item: Ref) -> &mut Self;
  fn last(&mut self, item: Ref) -> &mut Self;
  fn count(&mut self, count: i32) -> &mut Self;
}

impl Outlineable for pdf_writer::writers::Outline<'_> {
  fn first(&mut self, item: Ref) -> &mut Self {
    self.first(item)
  }

  fn last(&mut self, item: Ref) -> &mut Self {
    self.last(item)
  }

  fn count(&mut self, count: i32) -> &mut Self {
    self.count(count)
  }
}

impl Outlineable for OutlineItem<'_> {
  fn first(&mut self, item: Ref) -> &mut Self {
    self.first(item)
  }

  fn last(&mut self, item: Ref) -> &mut Self {
    self.last(item)
  }

  fn count(&mut self, count: i32) -> &mut Self {
    self.count(count)
  }
}

struct SerializedChildren {
  first: Ref,
  last: Ref,
  visible_count: usize,
}

impl SerializedChildren {
  fn write(&self, outlineable: &mut impl Outlineable, negate_count: bool) {
    outlineable.first(self.first);
    outlineable.last(self.last);

    let mut count = i32::try_from(self.visible_count).unwrap();
    if negate_count {
      count = -count;
    }
    outlineable.count(count);
  }
}

impl Outline {
  /// Create a new, empty outline.
  pub fn new() -> Self {
    Self { children: vec![] }
  }

  /// Push a new child (which may in turn contain other children) to the outline.
  pub fn push_child(&mut self, node: OutlineNode) {
    self.children.push(node)
  }

  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root: Ref,
  ) {
    let mut chunk = sc.new_chunk();
    let children = serialize_children(&self.children, root, &mut chunk, sc);

    let mut outline = chunk.outline(root);
    if let Some(children) = &children {
      children.write(&mut outline, false);
    }
    outline.finish();

    chunk_container.non_stream.outline = Some((root, chunk));
  }
}

/// An outline node.
///
/// This represents either an intermediate node in the outline tree, or a leaf node
/// if it does not contain any further children itself.
#[derive(Debug, Clone)]
pub struct OutlineNode {
  /// The children of the outline node.
  children: Vec<OutlineNode>,
  /// The text of the outline entry.
  text: String,
  /// The destination of the outline entry.
  destination: XyzDestination,
  /// Whether this node is initially open (children shown expanded).
  open: bool,
}

impl OutlineNode {
  /// Create a new outline node.
  ///
  /// `text` is the string that should be displayed in the outline tree, and
  /// `destination` is the destination that should be jumped to when clicking on
  /// the outline entry.
  pub fn new(text: String, destination: XyzDestination) -> Self {
    Self {
      children: vec![],
      text,
      destination,
      open: false,
    }
  }

  /// Set whether this node is initially open (children shown expanded).
  ///
  /// If `open` is `true`, the node's children will be shown expanded when the
  /// document is opened in a PDF viewer; if `false`, the children will be
  /// collapsed. Leaf nodes (nodes without children) are unaffected by this flag.
  ///
  /// By default, this flag is set to `false`.
  pub fn with_open(mut self, open: bool) -> Self {
    self.open = open;
    self
  }

  /// Add a new child to the outline node.
  pub fn push_child(&mut self, node: OutlineNode) {
    self.children.push(node)
  }

  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    parent: Ref,
    root: Ref,
    next: Option<Ref>,
    prev: Option<Ref>,
    chunk: &mut Chunk,
  ) -> usize {
    let children = serialize_children(&self.children, root, chunk, sc);
    let mut outline_entry = chunk.outline_item(root);
    outline_entry.parent(parent);

    if let Some(next) = next {
      outline_entry.next(next);
    }

    if let Some(prev) = prev {
      outline_entry.prev(prev);
    }

    if let Some(children) = &children {
      children.write(&mut outline_entry, !self.open);
    }

    outline_entry.title(TextStr(&self.text));

    let dest_ref = sc.register_xyz_destination(self.destination.clone());
    outline_entry.pair(Name(b"Dest"), dest_ref);

    outline_entry.finish();

    // See the algorithm described in the PDF spec. When recursing down, we
    // do not go into nodes that whose count is negative (i.e. nodes that are
    // closed). Therefore, in case the node is closed, the visible count is
    // just the node itself, so 1.

    if self.open {
      1 + children.map_or(0, |children| children.visible_count)
    } else {
      1
    }
  }
}

fn serialize_children(
  children: &[OutlineNode],
  parent: Ref,
  chunk: &mut Chunk,
  sc: &mut SerializeContext,
) -> Option<SerializedChildren> {
  let mut visible_count = 0;

  if !children.is_empty() {
    let first = sc.new_ref();
    let mut last = first;

    let mut prev = None;
    let mut cur = Some(first);

    for i in 0..children.len() {
      let next = if i < children.len() - 1 {
        Some(sc.new_ref())
      } else {
        None
      };

      last = cur.unwrap();

      let child_visible_count = children[i].serialize(sc, parent, last, next, prev, chunk);
      visible_count += child_visible_count;

      prev = cur;
      cur = next;
    }

    Some(SerializedChildren {
      first,
      last,
      visible_count,
    })
  } else {
    None
  }
}
