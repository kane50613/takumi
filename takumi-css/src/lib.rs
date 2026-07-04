#![deny(missing_docs)]
//! CSS parsing and computed-style layer for takumi.
//!
//! Holds the (cold) CSS parsing, cascade, value types, and selector matching so
//! they can be compiled independently from the hot rendering paths in `takumi`.
//! Matching is generic over a [`matching::MatchableNode`] the caller implements,
//! keeping this crate free of any node/render dependency and the `selectors`
//! crate out of takumi's public API.

/// Parse and cascade error types.
pub mod error;
/// `@keyframes` rules and animation timing.
pub mod keyframes;
/// Selector matching against an abstract node tree.
pub mod matching;
/// Backend painting helpers (gradient LUTs, tile positioning, transfer tables)
/// shared with the raster and SVG renderers. Deliberately kept out of `style`
/// (and thus `takumi`'s prelude) since they are rendering-backend internals, not
/// part of the CSS value surface.
pub mod paint {
  pub use crate::style::properties::{
    background_repeat::{
      collect_repeat_tile_positions, collect_spaced_tile_positions,
      collect_stretched_tile_positions,
    },
    conic_gradient::{ConicGradientRowState, ConicGradientTile},
    filter::compose_transfer_table,
    gradient_utils::{
      GradientOverlayTile, build_color_lut_with_interpolation,
      overlay_gradient_tile_fast_normal_unconstrained, resolve_stops_along_axis,
    },
    linear_gradient::{
      LinearGradientFastPath, LinearGradientFastPathData, LinearGradientFastPathKind,
      LinearGradientRowState, LinearGradientTile,
    },
    radial_gradient::{RadialGradientRowState, RadialGradientTile},
  };
}
/// CSS value types, parsing, and the cascade.
pub mod style;
mod viewport;

// Public surface re-exported at the crate root (e.g. `takumi_css::Display`).
pub use style::*;
pub use viewport::*;
