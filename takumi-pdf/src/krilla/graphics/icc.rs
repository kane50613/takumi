//! Using ICC profiles.

use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;

use pdf_writer::{Finish, Name, Ref};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::resource;
use crate::krilla::resource::Resourceable;
use crate::krilla::serialize::{Cacheable, SerializeContext};
use crate::krilla::stream::{FilterStreamBuilder, deflate_encode};
use crate::krilla::util::Prehashed;

/// An ICC profile.
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct ICCProfile<const C: u8>(Arc<Prehashed<Repr>>);

impl<const C: u8> ICCProfile<C> {
  /// Create a new ICC profile.
  ///
  /// Returns `None` if the metadata of the profile couldn't be read or if the
  /// number of channels of the underlying data does not correspond to the one
  /// indicated in the constant parameter.
  pub fn new(data: &[u8]) -> Option<Self> {
    let metadata = ICCMetadata::from_data(data)?;

    if metadata.color_space.num_components() != C {
      return None;
    }

    Some(Self(Arc::new(Prehashed::new(Repr {
      data: deflate_encode(data),
      metadata,
    }))))
  }

  pub(crate) fn metadata(&self) -> &ICCMetadata {
    &self.0.metadata
  }
}

impl<const C: u8> Cacheable for ICCProfile<C> {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let mut chunk = sc.new_chunk();
    let icc_stream =
      FilterStreamBuilder::new_from_deflated(&self.0.deref().data).finish(&sc.serialize_settings());

    let mut icc_profile = chunk.icc_profile(root_ref, icc_stream.encoded_data());
    icc_profile.n(C as i32).range([0.0, 1.0].repeat(C as usize));
    icc_stream.write_filters(icc_profile.deref_mut().deref_mut());
    icc_profile.finish();
    chunk_container.streams.icc_profiles.push(chunk);
  }
}

#[cfg(feature = "raster-images")]
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum GenericICCProfile {
  Luma(ICCProfile<1>),
  Rgb(ICCProfile<3>),
  Cmyk(ICCProfile<4>),
}

#[cfg(feature = "raster-images")]
impl GenericICCProfile {
  pub(crate) fn metadata(&self) -> &ICCMetadata {
    match self {
      GenericICCProfile::Luma(l) => l.metadata(),
      GenericICCProfile::Rgb(r) => r.metadata(),
      GenericICCProfile::Cmyk(c) => c.metadata(),
    }
  }
}

#[cfg(feature = "raster-images")]
impl Cacheable for GenericICCProfile {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    match self {
      GenericICCProfile::Luma(l) => l.serialize(sc, chunk_container, root_ref),
      GenericICCProfile::Rgb(r) => r.serialize(sc, chunk_container, root_ref),
      GenericICCProfile::Cmyk(c) => c.serialize(sc, chunk_container, root_ref),
    }
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct ICCBasedColorSpace<const C: u8>(pub(crate) ICCProfile<C>);

impl<const C: u8> Cacheable for ICCBasedColorSpace<C> {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let icc_ref = sc.register_cacheable(chunk_container, self.0.clone());

    let chunk = &mut chunk_container.non_stream.color_spaces;

    let mut array = chunk.indirect(root_ref).array();
    array.item(Name(b"ICCBased"));
    array.item(icc_ref);
    array.finish();
  }
}

impl<const C: u8> Resourceable for ICCBasedColorSpace<C> {
  type Resource = resource::ColorSpace;
}

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub(crate) enum ICCColorSpace {
  Xyz,
  Lab,
  Luv,
  Ycbr,
  Yxy,
  Lms,
  Rgb,
  Gray,
  Hsv,
  Hls,
  Cmyk,
  Cmy,
  OneClr,
  ThreeClr,
  FourClr,
  // There are more, but those should be the most important
  // ones.
}

impl ICCColorSpace {
  pub(crate) fn num_components(&self) -> u8 {
    match self {
      ICCColorSpace::Xyz => 3,
      ICCColorSpace::Lab => 3,
      ICCColorSpace::Luv => 3,
      ICCColorSpace::Ycbr => 3,
      ICCColorSpace::Yxy => 3,
      ICCColorSpace::Lms => 3,
      ICCColorSpace::Rgb => 3,
      ICCColorSpace::Gray => 1,
      ICCColorSpace::Hsv => 3,
      ICCColorSpace::Hls => 3,
      ICCColorSpace::Cmyk => 4,
      ICCColorSpace::Cmy => 3,
      ICCColorSpace::OneClr => 1,
      ICCColorSpace::ThreeClr => 3,
      ICCColorSpace::FourClr => 4,
    }
  }
}

impl TryFrom<u32> for ICCColorSpace {
  type Error = ();

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    match value {
      0x58595A20 => Ok(ICCColorSpace::Xyz),
      0x4C616220 => Ok(ICCColorSpace::Lab),
      0x4C757620 => Ok(ICCColorSpace::Luv),
      0x59436272 => Ok(ICCColorSpace::Ycbr),
      0x59787920 => Ok(ICCColorSpace::Yxy),
      0x4C4D5320 => Ok(ICCColorSpace::Lms),
      0x52474220 => Ok(ICCColorSpace::Rgb),
      0x47524159 => Ok(ICCColorSpace::Gray),
      0x48535620 => Ok(ICCColorSpace::Hsv),
      0x484C5320 => Ok(ICCColorSpace::Hls),
      0x434D594B => Ok(ICCColorSpace::Cmyk),
      0x434D5920 => Ok(ICCColorSpace::Cmy),
      0x31434C52 => Ok(ICCColorSpace::OneClr),
      0x33434C52 => Ok(ICCColorSpace::ThreeClr),
      0x34434C52 => Ok(ICCColorSpace::FourClr),
      _ => Err(()),
    }
  }
}

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub(crate) struct ICCMetadata {
  pub(crate) major: u8,
  pub(crate) minor: u8,
  pub(crate) color_space: ICCColorSpace,
}

impl ICCMetadata {
  pub(crate) fn from_data(data: &[u8]) -> Option<Self> {
    let major = *data.get(8)?;
    let minor = *data.get(9)? >> 4;
    let color_space = {
      let marker = u32::from_be_bytes(data.get(16..20)?.try_into().ok()?);
      ICCColorSpace::try_from(marker).ok()?
    };
    Some(Self {
      major,
      minor,
      color_space,
    })
  }
}

#[derive(Clone, Hash, Debug)]
struct Repr {
  data: Vec<u8>,
  metadata: ICCMetadata,
}
