//! SVG filter primitives, which a raster backend runs.

mod primitive;

pub use self::primitive::SvgFilter;
pub use self::primitive::{
  ColorChannel, ColorSpace, CompositeOperator, ConvolveMatrix, DiffuseLighting, DisplacementMap,
  DropShadow, EdgeMode, Input, Kernel, Kind, LightSource, MorphologyOperator, Primitive,
  SpecularLighting, SpotLight, Turbulence,
};
