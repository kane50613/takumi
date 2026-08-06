//! Blending.

/// How to blend source and backdrop.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[allow(missing_docs)]
pub enum BlendMode {
  Normal,
  Multiply,
  Screen,
  Overlay,
  Darken,
  Lighten,
  ColorDodge,
  ColorBurn,
  HardLight,
  SoftLight,
  Difference,
  Exclusion,
  Hue,
  Saturation,
  Color,
  Luminosity,
}

impl BlendMode {
  pub(crate) fn to_pdf(self) -> pdf_writer::types::BlendMode {
    match self {
      BlendMode::Normal => pdf_writer::types::BlendMode::Normal,
      BlendMode::Multiply => pdf_writer::types::BlendMode::Multiply,
      BlendMode::Screen => pdf_writer::types::BlendMode::Screen,
      BlendMode::Overlay => pdf_writer::types::BlendMode::Overlay,
      BlendMode::Darken => pdf_writer::types::BlendMode::Darken,
      BlendMode::Lighten => pdf_writer::types::BlendMode::Lighten,
      BlendMode::ColorDodge => pdf_writer::types::BlendMode::ColorDodge,
      BlendMode::ColorBurn => pdf_writer::types::BlendMode::ColorBurn,
      BlendMode::HardLight => pdf_writer::types::BlendMode::HardLight,
      BlendMode::SoftLight => pdf_writer::types::BlendMode::SoftLight,
      BlendMode::Difference => pdf_writer::types::BlendMode::Difference,
      BlendMode::Exclusion => pdf_writer::types::BlendMode::Exclusion,
      BlendMode::Hue => pdf_writer::types::BlendMode::Hue,
      BlendMode::Saturation => pdf_writer::types::BlendMode::Saturation,
      BlendMode::Color => pdf_writer::types::BlendMode::Color,
      BlendMode::Luminosity => pdf_writer::types::BlendMode::Luminosity,
    }
  }
}
