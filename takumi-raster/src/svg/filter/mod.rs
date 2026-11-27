//! Runs an SVG's filter primitives. Most primitives port Skia's and Blink's filter code, and
//! [`apply`] keeps the structure of resvg's `filter/mod.rs`.

use takumi_core::svg_filter::*;

mod apply;
mod color_filter;
mod composite;
mod convolve_matrix;
mod displacement_map;
mod lighting;
mod morphology;
mod turbulence;

pub(crate) use apply::{apply, apply_to_layer};

#[cfg(test)]
mod tests;

/// Skia's `SkRasterPipeline` reads a byte as a float by this product rather than a division.
const BYTE_TO_FLOAT: f32 = 1.0 / 255.0;
