use serde::{Deserialize, Serialize};

/// Gradient dithering for static image exports and raw buffers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DitheringAlgorithm {
  /// Disable gradient dithering.
  #[default]
  None,
  /// Dither gradient fills with an ordered 8x8 Bayer pattern.
  OrderedBayer,
  /// Alias of [`Self::OrderedBayer`]: the whole-image error-diffusion pass it
  /// once named reduced every pixel to 128 levels and is gone.
  #[deprecated(note = "alias of OrderedBayer; the whole-image pass is gone")]
  FloydSteinberg,
}
