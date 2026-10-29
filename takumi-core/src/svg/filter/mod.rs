//! SVG filter primitives and, under `svg-raster`, the pixel code from resvg that runs them.

#[cfg(feature = "svg-raster")]
mod apply;
#[cfg(feature = "svg-raster")]
mod color_filter;
#[cfg(feature = "svg-raster")]
mod composite;
#[cfg(feature = "svg-raster")]
mod convolve_matrix;
#[cfg(feature = "svg-raster")]
mod displacement_map;
#[cfg(feature = "svg-raster")]
mod lighting;
#[cfg(feature = "svg-raster")]
mod morphology;
mod primitive;
#[cfg(feature = "svg-raster")]
mod turbulence;

pub(crate) use self::primitive::SvgFilter;
pub(in crate::svg) use self::primitive::{
  ColorChannel, ColorSpace, CompositeOperator, ConvolveMatrix, DiffuseLighting, DisplacementMap,
  DropShadow, EdgeMode, Input, Kernel, Kind, LightSource, MorphologyOperator, Primitive,
  SpecularLighting, SpotLight, Turbulence,
};

/// Skia's `SkRasterPipeline` reads a byte as a float by this product rather than a division.
#[cfg(feature = "svg-raster")]
const BYTE_TO_FLOAT: f32 = 1.0 / 255.0;
