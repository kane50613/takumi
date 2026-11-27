//! Destinations in a PDF document.
//!
//! In some cases, you might want to refer to other locations within the same document, for
//! example when defining the outline, or when link to a different section in the document
//! from a link. To achieve this, you can use destinations, which are associated with a page
//! and a specific location on that page.

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use takumi_core::{geometry::Point, style::Affine};

use pdf_writer::{Obj, Ref};

use crate::writer::chunk_container::ChunkContainer;
use crate::writer::error::WriterResult;
use crate::writer::interchange::tagging::TagId;
use crate::writer::serialize::SerializeContext;

/// The type of destination.
#[derive(Hash)]
pub enum Destination {
  /// An XYZ destination.
  Xyz(XyzDestination),
}

impl Destination {
  pub(crate) fn serialize(&self, sc: &mut SerializeContext, buffer: Obj) -> WriterResult<()> {
    match self {
      Destination::Xyz(xyz) => {
        let ref_ = sc.register_xyz_destination(xyz.clone());
        buffer.primitive(ref_);

        Ok(())
      }
    }
  }
}

#[derive(Debug)]
struct XyzDestRepr {
  page_index: usize,
  point: Point<f32>,
  /// The structure element the destination targets, if the caller tagged one.
  /// PDF/UA-2 requires an in-document destination to name a structure element
  /// rather than a page position.
  structure: Option<TagId>,
}

impl Hash for XyzDestRepr {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.page_index.hash(state);
    self.point.x.to_bits().hash(state);
    self.point.y.to_bits().hash(state);
    self.structure.hash(state);
  }
}

impl PartialEq for XyzDestRepr {
  fn eq(&self, other: &Self) -> bool {
    self.page_index == other.page_index
      && self.point.x == other.point.x
      && self.point.y == other.point.y
      && self.structure == other.structure
  }
}

impl Eq for XyzDestRepr {}

/// A destination pointing to a specific location at a specific page.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct XyzDestination(Arc<XyzDestRepr>);

impl From<XyzDestination> for Destination {
  fn from(val: XyzDestination) -> Self {
    Destination::Xyz(val)
  }
}

impl XyzDestination {
  /// Create a new XYZ destination. `page_index` should be the index (i.e. number) of the
  /// target page, and point indicates the specific location on that page that should be
  /// targeted. If the `page_index` is out of range, export will panic.
  pub fn new(page_index: usize, point: Point<f32>) -> Self {
    Self(Arc::new(XyzDestRepr {
      page_index,
      point,
      structure: None,
    }))
  }

  /// Targets the structure element carrying `id`, making this a structure
  /// destination. The tag tree must contain the id or export fails.
  pub fn with_structure(self, id: TagId) -> Self {
    Self(Arc::new(XyzDestRepr {
      page_index: self.0.page_index,
      point: self.0.point,
      structure: Some(id),
    }))
  }

  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    // A destination past the last page points at the last page.
    let Some(page_info) = sc
      .page_infos()
      .get(self.0.page_index)
      .or_else(|| sc.page_infos().last())
    else {
      return;
    };

    let page_ref = page_info.ref_();
    let page_size = page_info.size().height;

    let point = self.0.point;
    // Convert to PDF coordinates
    let invert_transform = Affine {
      a: 1.0,
      b: 0.0,
      c: 0.0,
      d: -1.0,
      x: 0.0,
      y: page_size,
    };
    let (x, y) = invert_transform.transform_point(point.x, point.y);

    let mut destination = chunk_container
      .non_stream
      .destinations
      .destination(root_ref);

    match self.0.structure.as_ref().and_then(|id| sc.tag_ref(id)) {
      Some(struct_ref) => destination.item(struct_ref),
      None => destination.item(page_ref),
    };
    destination.xyz(x, y, None);
  }
}
