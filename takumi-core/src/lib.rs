#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]
//! Backend-agnostic core for takumi: the node tree, CSS-driven style and layout,
//! and font/image resource management.
//!
//! The rendering backends [`takumi-raster`](https://docs.rs/takumi-raster) and
//! [`takumi-svg`](https://docs.rs/takumi-svg) build on these types. Depend on the
//! [`takumi`](https://docs.rs/takumi) umbrella crate instead of this one directly.

/// Style resolution and box/inline layout.
pub mod layout;

/// Render context threading style, sizing, and resources through layout.
pub mod context;
/// A CSS `filter` function as a colour matrix.
pub mod filter;
/// Font style resolved against a sizing context.
pub mod font_style;
/// Box, text, and inset shadow resolution.
pub mod shadow;
pub mod text_processing;

/// Error types.
pub mod error;
pub mod geometry;
/// `@keyframes` rules and animation timing.
pub mod keyframes;
/// Selector matching against an abstract node tree.
pub(crate) mod matching;
/// Deterministic float math shared by the painting paths.
pub mod math;
/// The seam between deciding what to paint and painting it.
pub mod painter;
/// Font and image resource management.
pub mod resources;
pub mod scene;
/// CSS value types, parsing, and the cascade.
pub mod style;
pub mod units;
/// Viewport dimensions and device-pixel-ratio resolution.
pub mod viewport;

/// Vendored resvg 0.48.1 (see `resvg/mod.rs` for provenance and stripped features).
#[cfg(feature = "svg")]
#[allow(
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::all,
  missing_docs,
  dead_code,
  deprecated
)]
mod resvg;
#[cfg(feature = "svg")]
mod svg_vector;

/// Backend painting helpers (gradient LUTs, tile positioning, transfer tables) shared with the
/// raster and SVG renderers.
pub mod paint {
  pub use crate::style::properties::{
    conic_gradient::ConicGradientTile,
    filter::compose_transfer_table,
    gradient_utils::{ColorLut, GradientOverlayTile},
    linear_gradient::{LinearGradientFastPathKind, LinearGradientTile},
    radial_gradient::RadialGradientTile,
  };
}

use std::collections::HashSet;

pub use error::{Error, Result};
use xxhash_rust::xxh3::Xxh3DefaultBuilder;

pub use crate::resources::font::Fonts;

/// Type alias for HashSet using XXH3 hasher
pub(crate) type Xxh3HashSet<T> = HashSet<T, Xxh3DefaultBuilder>;
