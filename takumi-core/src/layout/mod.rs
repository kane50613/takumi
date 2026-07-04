/// Node Tree
pub mod node;

/// Backend-agnostic border geometry shared across rasterization backends.
pub mod border;
/// Inline-level layout: text shaping, line breaking, and text fitting.
pub mod inline;
/// Layout tree: render nodes and their computed layout results.
pub mod tree;

/// CSS-like styling system with colors, units, and properties.
pub mod style {
  pub use crate::style::*;
}
pub use crate::viewport::{DEFAULT_DEVICE_PIXEL_RATIO, Viewport};
