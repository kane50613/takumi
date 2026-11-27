//! PDF annotations, allowing you to add extra "content" to specific pages.
//!
//! PDF has the concept of annotations, which allow you to associate certain regions of
//! a page with an "annotation". The PDF reference defines many different actions, however,
//! The writer does not and never will expose all of them. As of right now, the only annotations
//! that are supported are "link annotations", which allow you associate a certain region of
//! the page with a link.

use crate::writer::geom::pdf_rect;
use pdf_writer::types::AnnotationFlags;
use pdf_writer::{Finish, Name, Ref, TextStr};
use takumi_core::geometry::Rect;

use crate::writer::chunk_container::ChunkContainer;
use crate::writer::configure::ValidationError;
use crate::writer::error::WriterResult;
use crate::writer::interactive::action::LinkAction;
use crate::writer::interactive::destination::Destination;
use crate::writer::page::page_root_transform;
use crate::writer::serialize::SerializeContext;
use crate::writer::surface::Location;

/// An annotation.
pub struct Annotation {
  pub(crate) annotation_type: AnnotationType,
  pub(crate) alt: Option<String>,
  pub(crate) struct_parent: Option<i32>,
  pub(crate) location: Option<Location>,
}

impl Annotation {
  /// Create a new link annotation with some alt text.
  ///
  /// Note that the alt text might be required in some cases, for example
  /// when exporting to PDF/UA.
  pub fn new_link(annotation: LinkAnnotation, alt_text: Option<String>) -> Self {
    Self {
      annotation_type: AnnotationType::Link(annotation),
      alt: alt_text,
      struct_parent: None,
      location: None,
    }
  }
}

impl From<LinkAnnotation> for Annotation {
  fn from(value: LinkAnnotation) -> Self {
    Self {
      annotation_type: AnnotationType::Link(value),
      alt: None,
      struct_parent: None,
      location: None,
    }
  }
}

impl Annotation {
  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
    page_height: f32,
  ) -> WriterResult<()> {
    let chunk = &mut chunk_container.non_stream.annotations;
    let mut annotation = chunk
      .indirect(root_ref)
      .start::<pdf_writer::writers::Annotation>();

    self
      .annotation_type
      .serialize_type(sc, &mut annotation, page_height)?;

    annotation.flags(AnnotationFlags::PRINT);

    if let Some(struct_parent) = self.struct_parent {
      annotation.struct_parent(struct_parent);
    }

    if let Some(alt_text) = &self.alt {
      annotation.contents(TextStr(alt_text));
    }

    if self.alt.as_ref().is_none_or(String::is_empty) {
      sc.register_validation_error(ValidationError::MissingAnnotationAltText(self.location));
    }

    annotation.finish();

    Ok(())
  }
}

/// A type of annotation.
pub enum AnnotationType {
  /// A link annotation.
  Link(LinkAnnotation),
}

impl AnnotationType {
  fn serialize_type(
    &self,
    sc: &mut SerializeContext,
    annotation: &mut pdf_writer::writers::Annotation,
    page_height: f32,
  ) -> WriterResult<()> {
    match self {
      AnnotationType::Link(l) => l.serialize_type(sc, annotation, page_height),
    }
  }
}

/// An annotation target.
pub enum Target {
  /// A destination within the document.
  Destination(Destination),
  /// A link to open.
  Action(LinkAction),
}

/// A link annotation.
pub struct LinkAnnotation {
  pub(crate) rect: Rect<f32>,
  pub(crate) target: Target,
}

impl LinkAnnotation {
  /// Create a new link annotation.
  ///
  /// `rect`: The bounding box of the link annotation that it should cover on the page.
  /// `target`: The target of the link annotation.
  pub fn new(rect: Rect<f32>, target: Target) -> Self {
    Self { rect, target }
  }

  fn serialize_type(
    &self,
    sc: &mut SerializeContext,
    annotation: &mut pdf_writer::writers::Annotation,
    page_height: f32,
  ) -> WriterResult<()> {
    annotation.subtype(pdf_writer::types::AnnotationType::Link);

    let actual_rect = self
      .rect
      .transformed(page_root_transform(page_height))
      .unwrap_or(self.rect);
    annotation.rect(pdf_rect(actual_rect));
    annotation.border(0.0, 0.0, 0.0, None);

    match &self.target {
      Target::Destination(destination) => {
        destination.serialize(sc, annotation.insert(Name(b"Dest")))
      }
      Target::Action(action) => {
        action.serialize(annotation.action());

        Ok(())
      }
    }
  }
}
