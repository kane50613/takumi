//! Blending.

use takumi_core::style::BlendMode;

/// The PDF blend mode `mode` writes as, normal for the modes PDF lacks.
pub(crate) fn pdf_blend_mode(mode: BlendMode) -> pdf_writer::types::BlendMode {
  use pdf_writer::types::BlendMode as Pdf;

  match mode {
    BlendMode::Multiply => Pdf::Multiply,
    BlendMode::Screen => Pdf::Screen,
    BlendMode::Overlay => Pdf::Overlay,
    BlendMode::Darken => Pdf::Darken,
    BlendMode::Lighten => Pdf::Lighten,
    BlendMode::ColorDodge => Pdf::ColorDodge,
    BlendMode::ColorBurn => Pdf::ColorBurn,
    BlendMode::HardLight => Pdf::HardLight,
    BlendMode::SoftLight => Pdf::SoftLight,
    BlendMode::Difference => Pdf::Difference,
    BlendMode::Exclusion => Pdf::Exclusion,
    BlendMode::Hue => Pdf::Hue,
    BlendMode::Saturation => Pdf::Saturation,
    BlendMode::Color => Pdf::Color,
    BlendMode::Luminosity => Pdf::Luminosity,
    _ => Pdf::Normal,
  }
}
