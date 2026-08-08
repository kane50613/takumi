//! Destinations in a PDF document.
//!
//! In some cases, you might want to refer to other locations within the same document, for
//! example when defining the outline, or when link to a different section in the document
//! from a link. To achieve this, you can use destinations, which are associated with a page
//! and a specific location on that page.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use pdf_writer::{Obj, Ref, Str};
use tiny_skia_path::Transform;

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::error::{KrillaError, KrillaResult};
use crate::krilla::geom::Point;
use crate::krilla::interchange::tagging::TagId;
use crate::krilla::serialize::{PageInfo, SerializeContext};

/// The type of destination.
#[derive(Hash)]
pub enum Destination {
  /// An XYZ destination.
  Xyz(XyzDestination),
  /// A named destination.
  Named(NamedDestination),
}

impl Destination {
  pub(crate) fn serialize(&self, sc: &mut SerializeContext, buffer: Obj) -> KrillaResult<()> {
    match self {
      Destination::Xyz(xyz) => {
        let ref_ = sc.register_xyz_destination(xyz.clone());
        buffer.primitive(ref_);

        Ok(())
      }
      Destination::Named(named) => named.serialize(sc, buffer),
    }
  }
}

/// A destination associated with a name.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct NamedDestination {
  pub(crate) name: Arc<String>,
  pub(crate) xyz_dest: Arc<XyzDestination>,
}

impl From<NamedDestination> for Destination {
  fn from(val: NamedDestination) -> Self {
    Destination::Named(val)
  }
}

impl NamedDestination {
  /// Create a new named destination.
  ///
  /// When used as part of a link annotation, the destination will be automatically registered
  /// with the [`Document`](crate::krilla::Document).
  ///
  /// That said, you can also manually register a destination without linking to it by calling
  /// [`Document::register_named_destination`](crate::krilla::Document::register_named_destination).
  pub fn new(name: String, xyz_dest: XyzDestination) -> Self {
    Self {
      name: Arc::new(name),
      xyz_dest: Arc::new(xyz_dest),
    }
  }

  pub(crate) fn serialize(&self, sc: &mut SerializeContext, destination: Obj) -> KrillaResult<()> {
    sc.register_named_destination(self.clone())
      .ok_or_else(|| KrillaError::DuplicateNamedDestination(Arc::clone(&self.name)))?;
    destination.primitive(Str(self.name.as_bytes()));

    Ok(())
  }
}

#[derive(Debug)]
struct XyzDestRepr {
  page_index: usize,
  point: Point,
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
  pub fn new(page_index: usize, point: Point) -> Self {
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
    let chunk = &mut chunk_container.non_stream.destinations;
    let destination = chunk.destination(root_ref);

    let page_info = sc.page_infos().get(self.0.page_index).unwrap_or_else(|| {
      panic!(
        "attempted to link to page {}, but document only has {} pages",
        self.0.page_index + 1,
        sc.page_infos().len()
      )
    });

    let (ref_, surface_size) = match page_info {
      PageInfo::Krilla {
        ref_, surface_size, ..
      } => (ref_, surface_size),
      PageInfo::Pdf { ref_, size, .. } => (ref_, size),
    };

    let page_ref = *ref_;
    let page_size = surface_size.height();

    let mut mapped_point = self.0.point.to_tsp();
    // Convert to PDF coordinates
    let invert_transform = Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, page_size);
    invert_transform.map_point(&mut mapped_point);

    let mut destination = destination;

    match self.0.structure.as_ref().and_then(|id| sc.tag_ref(id)) {
      Some(struct_ref) => destination.item(struct_ref),
      None => destination.item(page_ref),
    };
    destination.xyz(mapped_point.x, mapped_point.y, None);
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use crate::krilla::Document;
  use crate::krilla::annotation::{LinkAnnotation, Target};
  use crate::krilla::error::KrillaError;
  use crate::krilla::geom::{Point, Rect};

  use super::{NamedDestination, XyzDestination};

  #[test]
  fn named_duplicate_rejected() {
    let mut document = Document::new();
    assert_eq!(
      document.register_named_destination(NamedDestination::new(
        "same".to_string(),
        XyzDestination::new(0, Point::from_xy(0.0, 0.0)),
      )),
      Some(())
    );
    assert_eq!(
      document.register_named_destination(NamedDestination::new(
        "same".to_string(),
        XyzDestination::new(0, Point::from_xy(100.0, 100.0)),
      )),
      None
    );
  }

  #[test]
  fn named_duplicate_same_location_allowed() {
    let mut document = Document::new();
    assert_eq!(
      document.register_named_destination(NamedDestination::new(
        "same".to_string(),
        XyzDestination::new(0, Point::from_xy(0.0, 0.0)),
      )),
      Some(())
    );
    assert_eq!(
      document.register_named_destination(NamedDestination::new(
        "same".to_string(),
        XyzDestination::new(0, Point::from_xy(0.0, 0.0)),
      )),
      Some(())
    );

    assert!(document.finish().is_ok());
  }

  #[test]
  fn named_duplicate_annotation_rejected() {
    let mut document = Document::new();
    let mut page = document.start_page();

    page.add_annotation(
      LinkAnnotation::new(
        Rect::from_xywh(0.0, 0.0, 100.0, 100.0).unwrap(),
        Target::Destination(
          NamedDestination::new(
            "same".to_string(),
            XyzDestination::new(0, Point::from_xy(0.0, 0.0)),
          )
          .into(),
        ),
      )
      .into(),
    );
    page.add_annotation(
      LinkAnnotation::new(
        Rect::from_xywh(0.0, 100.0, 100.0, 100.0).unwrap(),
        Target::Destination(
          NamedDestination::new(
            "same".to_string(),
            XyzDestination::new(0, Point::from_xy(100.0, 100.0)),
          )
          .into(),
        ),
      )
      .into(),
    );
    drop(page);

    assert_eq!(
      document.finish(),
      Err(KrillaError::DuplicateNamedDestination(Arc::new(
        "same".to_string()
      )))
    );
  }

  #[test]
  fn named_duplicate_manual_then_annotation_rejected() {
    let mut document = Document::new();
    assert_eq!(
      document.register_named_destination(NamedDestination::new(
        "same".to_string(),
        XyzDestination::new(0, Point::from_xy(0.0, 0.0)),
      )),
      Some(())
    );

    let mut page = document.start_page();
    page.add_annotation(
      LinkAnnotation::new(
        Rect::from_xywh(0.0, 0.0, 100.0, 100.0).unwrap(),
        Target::Destination(
          NamedDestination::new(
            "same".to_string(),
            XyzDestination::new(0, Point::from_xy(100.0, 100.0)),
          )
          .into(),
        ),
      )
      .into(),
    );
    drop(page);

    assert_eq!(
      document.finish(),
      Err(KrillaError::DuplicateNamedDestination(Arc::new(
        "same".to_string()
      )))
    );
  }
}
