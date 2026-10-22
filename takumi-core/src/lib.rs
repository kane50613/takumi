#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]
//! The layout engine shared by every takumi output. It holds the node tree, the CSS cascade,
//! layout, text shaping, and font and image resources. Its block, inline, flexbox, grid, and
//! table layout port Chromium's Blink.
//!
//! Each backend builds on these types: [`takumi-raster`](https://docs.rs/takumi-raster) for
//! images, [`takumi-svg`](https://docs.rs/takumi-svg) for SVG, and the PDF and paint-tree
//! builds behind the `takumi-pdf` and `takumi-paint` npm packages.
//!
//! Depend on the [`takumi`](https://docs.rs/takumi) crate instead. Its prelude re-exports the
//! stable subset of these types.

/// Style resolution and box/inline layout.
pub mod layout;

/// Gaussian blur of pixels, as Skia's raster blur engine runs it.
pub mod blur;
/// Render context threading style, sizing, and resources through layout.
pub mod context;
/// A CSS `filter` function as a colour matrix.
pub mod filter;
/// Font style resolved against a sizing context.
pub mod font_style;
/// Fonts and text shaped with them, after Blink.
pub(crate) mod fonts;
/// Box, text, and inset shadow resolution.
pub mod shadow;
/// Unicode facts about characters, after Blink.
pub(crate) mod text;

/// Error types.
pub mod error;
pub mod geometry;
/// `@keyframes` rules and animation timing.
pub mod keyframes;
pub(crate) mod layout_unit;
/// Selector matching against an abstract node tree.
pub(crate) mod matching;
/// Deterministic float math shared by the painting paths.
pub mod math;
pub mod paint_chunk;
pub mod paint_property;
#[cfg(feature = "paint-tree")]
pub mod paint_tree;
pub mod painter;
pub mod path_data;
/// Font and image resource management.
pub mod resources;
pub mod scene;
pub(crate) mod sort_key;
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
    gradient_utils::{ColorLut, GradientOverlayTile, SrgbStop},
    linear_gradient::{LinearGradientFastPathKind, LinearGradientGeometry, LinearGradientTile},
    radial_gradient::{RadialGradientGeometry, RadialGradientTile},
  };
}

use std::collections::HashSet;

pub use error::{Error, Result};
use xxhash_rust::xxh3::Xxh3DefaultBuilder;

pub use crate::resources::font::Fonts;

/// Type alias for HashSet using XXH3 hasher
pub(crate) type Xxh3HashSet<T> = HashSet<T, Xxh3DefaultBuilder>;
