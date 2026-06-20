/// Node Tree
pub mod node;

/// Backend-agnostic border geometry shared across rasterization backends.
pub mod border;
pub mod inline;
pub(crate) mod matching;
pub mod tree;

/// CSS-like styling system with colors, units, and properties.
///
/// Re-exported from the `takumi-css` crate, which holds the cold CSS
/// parsing/cascade layer so it can be size-optimized independently.
pub mod style {
  pub use takumi_css::style::*;
}
pub use takumi_css::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport};
