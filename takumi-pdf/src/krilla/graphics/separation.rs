use pdf_writer::writers::ExponentialFunction;
use pdf_writer::{Finish, Name, Ref, Writer};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::color::separation::SeparationSpace;
use crate::krilla::color::{DEVICE_CMYK, DEVICE_GRAY, DEVICE_RGB};
use crate::krilla::resource::{self, Resource, Resourceable};
use crate::krilla::serialize::{Cacheable, MaybeDeviceColorSpace, SerializeContext};

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct SeparationColorSpace {
  space: SeparationSpace,
}

impl SeparationColorSpace {
  pub fn new(space: SeparationSpace) -> Self {
    Self { space }
  }
}

impl Cacheable for SeparationColorSpace {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    // Delegate fallback color space registration to existing logic
    let fallback_cs = self.space.fallback.color_space(sc);
    let fallback_cs_resource = sc.register_colorspace(chunk_container, fallback_cs.into());

    // Get fallback color components for tint function
    let fallback_color = crate::krilla::color::Color::from(self.space.fallback).to_pdf_color();
    let num_components = fallback_color.len();

    // Determine C0 based on whether the fallback is subtractive or additive
    // - Additive (RGB, Luma): tint 0.0 = white = [1.0, 1.0, 1.0]
    // - Subtractive (CMYK): tint 0.0 = white = [0.0, 0.0, 0.0, 0.0]
    let c0_value = if self.space.fallback.is_subtractive() {
      0.0
    } else {
      1.0
    };

    if let Err(validation_error) = sc.validation_store().validate_separation(&self.space) {
      sc.register_validation_error(validation_error);
    }

    let chunk = &mut chunk_container.non_stream.color_spaces;

    // Write Separation color space array: [/Separation name alternateSpace tintTransform]
    let mut array = chunk.indirect(root_ref).array();
    array.item(Name(b"Separation"));

    // Colorant name
    array.item(self.space.colorant.to_pdf());

    // Fallback color space - write as name for device CS, or ref for others
    match fallback_cs_resource {
      MaybeDeviceColorSpace::DeviceRgb => array.item(Name(DEVICE_RGB.as_bytes())),
      MaybeDeviceColorSpace::DeviceGray => array.item(Name(DEVICE_GRAY.as_bytes())),
      MaybeDeviceColorSpace::DeviceCMYK => array.item(Name(DEVICE_CMYK.as_bytes())),
      MaybeDeviceColorSpace::ColorSpace(cs) => {
        // Use get_ref() to extract the underlying Ref
        array.item(cs.get_ref())
      }
    };

    // Write Type 2 (Exponential) function for tint transform
    // Maps tint [0.0-1.0] from white (no ink) to fallback color (full ink)
    ExponentialFunction::start(array.push())
      .domain([0.0, 1.0])
      .range([0.0, 1.0].repeat(num_components))
      // C0: white/no ink (tint = 0.0) - value depends on color space
      .c0(vec![c0_value; num_components])
      // C1: fallback color (tint = 1.0)
      .c1(fallback_color)
      .n(1.0);

    array.finish();
  }
}

impl Resourceable for SeparationColorSpace {
  type Resource = resource::ColorSpace;
}
