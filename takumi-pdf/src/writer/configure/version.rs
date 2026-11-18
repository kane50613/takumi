use pdf_writer::Pdf;
use std::sync::LazyLock;
use xmp_writer::XmpWriter;

#[cfg(feature = "images")]
use crate::writer::graphics::icc::ICCMetadata;
use crate::writer::graphics::icc::ICCProfile;
#[cfg(feature = "images")]
use crate::writer::image::BitsPerComponent;

/// The version of a PDF document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum PdfVersion {
  /// PDF 1.4.
  Pdf14,
  /// PDF 1.5.
  Pdf15,
  /// PDF 1.6.
  Pdf16,
  /// PDF 1.7.
  #[default]
  Pdf17,
  /// PDF 2.0.
  Pdf20,
}

impl PdfVersion {
  /// The minimum supported PDF version.
  pub const MIN: Self = Self::Pdf14;
  /// The maximum supported PDF version.
  pub const MAX: Self = Self::Pdf20;

  pub(crate) fn write_xmp(&self, xmp: &mut XmpWriter) {
    match self {
      PdfVersion::Pdf14 => xmp.pdf_version("1.4"),
      PdfVersion::Pdf15 => xmp.pdf_version("1.5"),
      PdfVersion::Pdf16 => xmp.pdf_version("1.6"),
      PdfVersion::Pdf17 => xmp.pdf_version("1.7"),
      PdfVersion::Pdf20 => xmp.pdf_version("2.0"),
    };
  }

  /// Get a string representation of the PDF version.
  pub fn as_str(self) -> &'static str {
    match self {
      PdfVersion::Pdf14 => "PDF 1.4",
      PdfVersion::Pdf15 => "PDF 1.5",
      PdfVersion::Pdf16 => "PDF 1.6",
      PdfVersion::Pdf17 => "PDF 1.7",
      PdfVersion::Pdf20 => "PDF 2.0",
    }
  }

  pub(crate) fn rgb_icc(&self) -> ICCProfile<3> {
    match self {
      PdfVersion::Pdf14 => SRGB_V2_ICC.clone(),
      PdfVersion::Pdf15 => SRGB_V2_ICC.clone(),
      PdfVersion::Pdf16 => SRGB_V2_ICC.clone(),
      PdfVersion::Pdf17 => SRGB_V4_ICC.clone(),
      PdfVersion::Pdf20 => SRGB_V4_ICC.clone(),
    }
  }

  pub(crate) fn grey_icc(&self) -> ICCProfile<1> {
    match self {
      PdfVersion::Pdf14 => GREY_V2_ICC.clone(),
      PdfVersion::Pdf15 => GREY_V2_ICC.clone(),
      PdfVersion::Pdf16 => GREY_V2_ICC.clone(),
      PdfVersion::Pdf17 => GREY_V4_ICC.clone(),
      PdfVersion::Pdf20 => GREY_V4_ICC.clone(),
    }
  }

  #[cfg(feature = "images")]
  pub(crate) fn supports_icc(&self, metadata: &ICCMetadata) -> bool {
    let max_version = match self {
      PdfVersion::Pdf14 => (2, 3),
      PdfVersion::Pdf15 => (4, 0),
      PdfVersion::Pdf16 => (4, 1),
      PdfVersion::Pdf17 => (4, 2),
      PdfVersion::Pdf20 => (4, 3),
    };

    (metadata.major, metadata.minor) <= max_version
  }

  #[cfg(feature = "images")]
  pub(crate) fn supports_bit_depth(&self, bits_per_component: BitsPerComponent) -> bool {
    match bits_per_component {
      BitsPerComponent::Eight => true,
      BitsPerComponent::Sixteen => *self >= PdfVersion::Pdf15,
    }
  }

  /// Whether the version supports the `/AF` key.
  ///
  /// Note that support for the `/AF` key may instead be provided by the
  /// PDF/A-3 standard for 1.x series files.
  pub(crate) fn specifies_associated_files(&self) -> bool {
    match self {
      PdfVersion::Pdf14 | PdfVersion::Pdf15 | PdfVersion::Pdf16 | PdfVersion::Pdf17 => false,
      PdfVersion::Pdf20 => true,
    }
  }

  pub(crate) fn set_version(&self, pdf: &mut Pdf) {
    match self {
      PdfVersion::Pdf14 => pdf.set_version(1, 4),
      PdfVersion::Pdf15 => pdf.set_version(1, 5),
      PdfVersion::Pdf16 => pdf.set_version(1, 6),
      PdfVersion::Pdf17 => pdf.set_version(1, 7),
      PdfVersion::Pdf20 => pdf.set_version(2, 0),
    };
  }

  pub(crate) fn deprecates_proc_sets(&self) -> bool {
    *self >= PdfVersion::Pdf20
  }
}

/// The ICC v4 profile for the SRGB color space.
#[expect(
  clippy::unwrap_used,
  reason = "the bundled profile parses, as a test checks"
)]
static SRGB_V4_ICC: LazyLock<ICCProfile<3>> =
  LazyLock::new(|| ICCProfile::new(include_bytes!("../../../icc/sRGB-v4.icc")).unwrap());
/// The ICC v2 profile for the SRGB color space.
#[expect(
  clippy::unwrap_used,
  reason = "the bundled profile parses, as a test checks"
)]
static SRGB_V2_ICC: LazyLock<ICCProfile<3>> =
  LazyLock::new(|| ICCProfile::new(include_bytes!("../../../icc/sRGB-v2-magic.icc")).unwrap());
/// The ICC v4 profile for the sgray color space.
#[expect(
  clippy::unwrap_used,
  reason = "the bundled profile parses, as a test checks"
)]
static GREY_V4_ICC: LazyLock<ICCProfile<1>> =
  LazyLock::new(|| ICCProfile::new(include_bytes!("../../../icc/sGrey-v4.icc")).unwrap());
/// The ICC v2 profile for the sgray color space.
#[expect(
  clippy::unwrap_used,
  reason = "the bundled profile parses, as a test checks"
)]
static GREY_V2_ICC: LazyLock<ICCProfile<1>> =
  LazyLock::new(|| ICCProfile::new(include_bytes!("../../../icc/sGrey-v2-magic.icc")).unwrap());

#[cfg(test)]
mod bundled_profile_tests {
  use std::sync::LazyLock;

  use super::{GREY_V2_ICC, GREY_V4_ICC, SRGB_V2_ICC, SRGB_V4_ICC};

  #[test]
  fn every_bundled_profile_parses() {
    LazyLock::force(&SRGB_V4_ICC);
    LazyLock::force(&SRGB_V2_ICC);
    LazyLock::force(&GREY_V4_ICC);
    LazyLock::force(&GREY_V2_ICC);
  }
}

#[cfg(all(test, feature = "images"))]
mod tests {
  use super::PdfVersion;
  use crate::writer::graphics::icc::{ICCColorSpace, ICCMetadata};

  fn icc_metadata(major: u8, minor: u8) -> ICCMetadata {
    ICCMetadata {
      major,
      minor,
      color_space: ICCColorSpace::Cmyk,
    }
  }

  #[test]
  fn icc_v2_4_supported_from_pdf_1_5() {
    assert!(!PdfVersion::Pdf14.supports_icc(&icc_metadata(2, 4)));
    assert!(PdfVersion::Pdf15.supports_icc(&icc_metadata(2, 4)));
    assert!(PdfVersion::Pdf16.supports_icc(&icc_metadata(2, 4)));
    assert!(PdfVersion::Pdf17.supports_icc(&icc_metadata(2, 4)));
    assert!(PdfVersion::Pdf20.supports_icc(&icc_metadata(2, 4)));
  }

  #[test]
  fn icc_max_version_per_pdf_version() {
    assert!(PdfVersion::Pdf14.supports_icc(&icc_metadata(2, 3)));
    assert!(!PdfVersion::Pdf14.supports_icc(&icc_metadata(4, 0)));
    assert!(PdfVersion::Pdf15.supports_icc(&icc_metadata(4, 0)));
    assert!(!PdfVersion::Pdf15.supports_icc(&icc_metadata(4, 1)));
    assert!(PdfVersion::Pdf16.supports_icc(&icc_metadata(4, 1)));
    assert!(!PdfVersion::Pdf16.supports_icc(&icc_metadata(4, 2)));
    assert!(PdfVersion::Pdf17.supports_icc(&icc_metadata(4, 2)));
    assert!(!PdfVersion::Pdf17.supports_icc(&icc_metadata(4, 3)));
    assert!(PdfVersion::Pdf20.supports_icc(&icc_metadata(4, 3)));
    assert!(!PdfVersion::Pdf20.supports_icc(&icc_metadata(4, 4)));
  }
}
