/// Node Tree
pub mod node;

pub(crate) mod inline;
pub(crate) mod matching;
pub(crate) mod tree;

/// CSS-like styling system with colors, units, and properties.
///
/// Re-exported from the `takumi-css` crate, which holds the cold CSS
/// parsing/cascade layer so it can be size-optimized independently.
pub mod style {
  pub use takumi_css::style::*;

  /// Deprecated alias for [`FontSynthesisValue`]; removed in takumi 2.0.
  #[deprecated(note = "renamed to `FontSynthesisValue`; removed in takumi 2.0")]
  pub type FontSynthesic = takumi_css::style::FontSynthesisValue;
}
pub use takumi_css::{DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport, ViewportSize};
