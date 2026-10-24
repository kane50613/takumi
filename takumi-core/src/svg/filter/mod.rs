//! SVG filters: the primitives of a filter as the layout builds them, and, under `svg-raster`, the
//! pixel code that runs them on a layer. The pixel code is resvg's, moved here and retargeted to
//! these types, except where takumi already has the math.

#[cfg(feature = "svg-raster")]
mod apply;
#[cfg(feature = "svg-raster")]
mod color_space;
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
