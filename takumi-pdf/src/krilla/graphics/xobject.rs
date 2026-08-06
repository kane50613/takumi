use std::ops::DerefMut;
use std::sync::Arc;

use pdf_writer::{Finish, Name, Ref};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::ValidationError;
use crate::krilla::geom::Rect;
use crate::krilla::graphics::color::{DEVICE_RGB, rgb};
use crate::krilla::resource;
use crate::krilla::resource::{Resource, Resourceable};
use crate::krilla::serialize::{Cacheable, MaybeDeviceColorSpace, SerializeContext};
use crate::krilla::stream::{FilterStreamBuilder, Stream};
use crate::krilla::util::{NameExt, Prehashed};

#[derive(Debug, Hash, Eq, PartialEq)]
struct Repr {
  stream: Stream,
  isolated: bool,
  transparency_group_color_space: bool,
  custom_bbox: Option<Rect>,
}

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub(crate) struct XObject(Arc<Prehashed<Repr>>);

impl XObject {
  pub(crate) fn new(
    stream: Stream,
    isolated: bool,
    mut transparency_group_color_space: bool,
    custom_bbox: Option<Rect>,
  ) -> Self {
    // In case a mask was invoked in the content stream, we _always_ create
    // a new transparency group. Please see <https://github.com/typst/typst/issues/5509>.
    // Just to provide a brief explanation: I have not found any mention
    // of a transparency group being required when using a soft mask in the
    // PDF spec. However, Apple Preview (and only them!) have a weird bug
    // where if you transform an XObject with a soft mask but not transparency
    // group, the transform gets applied twice to the mask, resulting in
    // rendering issues. Using a transparency group in this case seems to
    // fix the issue.
    // TODO: Apply group transparency to page as well.
    transparency_group_color_space |= stream.uses_mask;

    Self(Arc::new(Prehashed::new(Repr {
      stream,
      isolated,
      transparency_group_color_space,
      custom_bbox,
    })))
  }

  pub(crate) fn is_empty(&self) -> bool {
    self.0.stream.is_empty()
  }

  pub(crate) fn bbox(&self) -> Rect {
    self.0.custom_bbox.unwrap_or(self.0.stream.bbox)
  }
}

impl Cacheable for XObject {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let mut chunk = sc.new_chunk();

    for validation_error in &self.0.stream.validation_errors {
      sc.register_validation_error(validation_error.clone());
    }

    let use_transparency_group = self.0.isolated || self.0.transparency_group_color_space;

    if use_transparency_group {
      sc.register_validation_error(ValidationError::Transparency(sc.location));
    }

    let serialize_settings = sc.serialize_settings();

    // "Ordinarily, the CS entry may be present only for isolated transparency groups (those
    // for which I is true), and even then it is optional. However, this entry shall be present in
    // the group attributes dictionary for any transparency group XObject that has no parent
    // group or page from which to inherit."
    //
    // So while we don't technically always need to write it, let's just play it save
    // and always add a group color space in case we have a transparency group.
    let transparency_group_cs = if use_transparency_group {
      Some(sc.register_colorspace(
        chunk_container,
        rgb::color_space(serialize_settings.no_device_cs).into(),
      ))
    } else {
      None
    };

    let x_object_stream =
      FilterStreamBuilder::new_from_content_stream(&self.0.stream.content, &serialize_settings)
        .finish(&serialize_settings);
    let mut x_object = chunk.form_xobject(root_ref, x_object_stream.encoded_data());
    x_object_stream.write_filters(x_object.deref_mut().deref_mut());

    self.0.stream.resource_dictionary.to_pdf_resources(
      &mut x_object,
      sc,
      &mut chunk_container.non_stream.resource_dictionaries,
    );
    x_object.bbox(
      self
        .0
        .custom_bbox
        .unwrap_or(self.0.stream.bbox)
        .to_pdf_rect(),
    );

    if use_transparency_group {
      let mut group = x_object.group();
      let transparency = group.transparency();

      if self.0.isolated {
        transparency.isolated(self.0.isolated);
      }

      if let Some(transparency_group_cs) = transparency_group_cs {
        let pdf_cs = transparency.insert(Name(b"CS"));

        match transparency_group_cs {
          MaybeDeviceColorSpace::DeviceRgb => pdf_cs.primitive(DEVICE_RGB.to_pdf_name()),
          // Can only be SRGB
          MaybeDeviceColorSpace::ColorSpace(cs) => pdf_cs.primitive(cs.get_ref()),
          _ => unreachable!(),
        }
      }

      transparency.finish();
      group.finish();
    }

    x_object.finish();
    chunk_container.streams.x_objects.push(chunk);
  }
}

impl Resourceable for XObject {
  type Resource = resource::XObject;
}
