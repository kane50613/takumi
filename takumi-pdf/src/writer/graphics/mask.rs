//! Alpha and luminosity masks.

use pdf_writer::{Finish, Name, Ref};
use takumi_core::{
  geometry::Rect,
  style::{Affine, MaskType},
};

use crate::writer::chunk_container::ChunkContainer;
use crate::writer::graphics::shading_function::{GradientProperties, ShadingFunction};
use crate::writer::graphics::xobject::XObject;
use crate::writer::resource;
use crate::writer::resource::Resourceable;
use crate::writer::serialize::{Cacheable, SerializeContext};
use crate::writer::stream::Stream;
use crate::writer::stream::StreamBuilder;

/// A mask. Can be a luminance mask or an alpha mask.
///
/// IMPORTANT: Note that you must only use a mask in the document that you created it with!
/// If you use it in a different document, you will end up with an invalid PDF file.
// TODO: Maybe we can enfore the above somehow?
#[derive(PartialEq, Eq, Debug, Hash)]
pub struct Mask {
  /// The stream of the mask.
  stream: Stream,
  /// The type of the mask.
  mask_type: MaskType,
  /// A custom bbox of the mask. The only reason we need this is that for gradients with
  /// transparencies, we create a custom mask where we call the shading operator. In this case,
  /// we want to manually set the bbox of the underlying XObject to match the shape that the
  /// gradient is being applied to.
  custom_bbox: Option<Rect<f32>>,
}

impl Mask {
  /// Create a new mask. `stream` contains the content description
  /// of the mask, and `mask_type` indicates the type of mask.
  pub fn new(stream: Stream, mask_type: MaskType) -> Self {
    Self {
      stream,
      mask_type,
      custom_bbox: None,
    }
  }

  /// Create a new mask for a shading to encode the opacity channels.
  pub(crate) fn new_from_shading(
    gradient_properties: GradientProperties,
    shading_transform: Affine,
    bbox: Rect<f32>,
    serializer_context: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) -> Option<Self> {
    match &gradient_properties {
      GradientProperties::RadialAxialGradient(rag) => {
        if rag.stops.iter().all(|s| s.opacity.get() == 1.0) {
          return None;
        }
      }
      GradientProperties::PostScriptGradient(psg) => {
        if psg.stops.iter().all(|s| s.opacity.get() == 1.0) {
          return None;
        }
      }
    }

    let shading_function = ShadingFunction::new(gradient_properties, true);

    let shading_stream = {
      let mut builder = StreamBuilder::new(serializer_context, chunk_container);
      let mut surface = builder.surface();
      surface.push_transform(&shading_transform);
      surface.draw_shading(&shading_function);
      surface.pop();
      surface.finish();
      builder.finish()
    };

    Some(Self {
      stream: shading_stream,
      mask_type: MaskType::Luminance,
      custom_bbox: Some(bbox),
    })
  }
}

/// The PDF name of `mask_type`.
pub(crate) fn mask_type_name(mask_type: MaskType) -> Name<'static> {
  match mask_type {
    MaskType::Alpha => Name(b"Alpha"),
    MaskType::Luminance => Name(b"Luminosity"),
  }
}

impl Cacheable for Mask {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let x_object = sc.register_cacheable(
      chunk_container,
      XObject::new(self.stream, false, true, self.custom_bbox),
    );

    let chunk = &mut chunk_container.non_stream.masks;
    let mut dict = chunk.indirect(root_ref).dict();
    dict.pair(Name(b"Type"), Name(b"Mask"));
    dict.pair(Name(b"S"), mask_type_name(self.mask_type));
    dict.pair(Name(b"G"), x_object);

    dict.finish();
  }
}

impl Resourceable for Mask {
  type Resource = resource::XObject;
}
