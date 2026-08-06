//! Alpha and luminosity masks.

use pdf_writer::{Finish, Name, Ref};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::geom::{Rect, Transform};
use crate::krilla::graphics::shading_function::{GradientProperties, ShadingFunction};
use crate::krilla::graphics::xobject::XObject;
use crate::krilla::resource;
use crate::krilla::resource::Resourceable;
use crate::krilla::serialize::{Cacheable, SerializeContext};
use crate::krilla::stream::Stream;
use crate::krilla::stream::StreamBuilder;

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
  custom_bbox: Option<Rect>,
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
    shading_transform: Transform,
    bbox: Rect,
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
      mask_type: MaskType::Luminosity,
      custom_bbox: Some(bbox),
    })
  }
}

/// A mask type.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum MaskType {
  /// A luminosity mask.
  Luminosity,
  /// An alpha mask.
  Alpha,
}

impl MaskType {
  /// Return the PDF name of the mask type.
  pub(crate) fn to_name(self) -> Name<'static> {
    match self {
      MaskType::Alpha => Name(b"Alpha"),
      MaskType::Luminosity => Name(b"Luminosity"),
    }
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
    dict.pair(Name(b"S"), self.mask_type.to_name());
    dict.pair(Name(b"G"), x_object);

    dict.finish();
  }
}

impl Resourceable for Mask {
  type Resource = resource::XObject;
}
