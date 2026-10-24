//! The filters of an SVG document as Blink builds them from `filter` elements, after Blink's
//! `FilterEffect` graph: each primitive has its inputs, its subregion, and its color space, all
//! resolved to user space, so applying a filter never reads the document again.

use crate::{
  filter::ColorMatrixChain,
  geometry::Rect,
  resources::image::SvgOp,
  style::{BlendMode, Color, TransferTable},
};

/// A filter: the region it paints within and the primitives that run in order.
#[derive(Debug)]
pub(crate) struct SvgFilter {
  pub(in crate::svg) region: Rect<f32>,
  pub(in crate::svg) primitives: Vec<Primitive>,
}

impl SvgFilter {
  /// The region the filter paints within, in the user space of the element it filters.
  pub(crate) fn region(&self) -> Rect<f32> {
    self.region
  }
}

#[derive(Debug)]
pub(in crate::svg) struct Primitive {
  pub(in crate::svg) subregion: Rect<f32>,
  pub(in crate::svg) color_space: ColorSpace,
  /// The name later primitives read this one's result by.
  pub(in crate::svg) result: String,
  pub(in crate::svg) kind: Kind,
}

/// The space a primitive computes in, from `color-interpolation-filters`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::svg) enum ColorSpace {
  Srgb,
  LinearRgb,
}

#[derive(Debug)]
pub(in crate::svg) enum Kind {
  Blend {
    input1: Input,
    input2: Input,
    mode: BlendMode,
  },
  ColorMatrix {
    input: Input,
    chain: ColorMatrixChain,
  },
  ComponentTransfer {
    input: Input,
    /// The tables of the red, green, blue, and alpha channels, none for one left as it is.
    tables: Box<[Option<TransferTable>; 4]>,
  },
  Composite {
    input1: Input,
    input2: Input,
    operator: CompositeOperator,
  },
  ConvolveMatrix(ConvolveMatrix),
  DiffuseLighting(DiffuseLighting),
  DisplacementMap(DisplacementMap),
  DropShadow(DropShadow),
  Flood {
    color: Color,
    opacity: f32,
  },
  GaussianBlur {
    input: Input,
    std_dev_x: f32,
    std_dev_y: f32,
  },
  /// An image or an element, drawn as the ops of its content.
  Image(Vec<SvgOp>),
  Merge {
    inputs: Vec<Input>,
  },
  Morphology {
    input: Input,
    operator: MorphologyOperator,
    radius_x: f32,
    radius_y: f32,
  },
  Offset {
    input: Input,
    dx: f32,
    dy: f32,
  },
  SpecularLighting(SpecularLighting),
  Tile {
    input: Input,
  },
  Turbulence(Turbulence),
}

/// What a primitive reads: the filtered graphic, its alpha, or the result of a primitive.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::svg) enum Input {
  SourceGraphic,
  SourceAlpha,
  Reference(String),
}

impl Input {
  /// Whether the input is the result named `name`.
  pub(super) fn names(&self, name: &str) -> bool {
    matches!(self, Self::Reference(reference) if reference == name)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::svg) enum CompositeOperator {
  Over,
  In,
  Out,
  Atop,
  Xor,
  Arithmetic { k1: f32, k2: f32, k3: f32, k4: f32 },
}

#[derive(Debug)]
pub(in crate::svg) struct ConvolveMatrix {
  pub(in crate::svg) input: Input,
  pub(in crate::svg) kernel: Kernel,
  pub(in crate::svg) divisor: f32,
  pub(in crate::svg) bias: f32,
  pub(in crate::svg) edge_mode: EdgeMode,
  pub(in crate::svg) preserve_alpha: bool,
}

/// The kernel of a convolution: `columns` by `rows` entries, with the target among them.
#[derive(Debug)]
pub(in crate::svg) struct Kernel {
  pub(in crate::svg) target_x: u32,
  pub(in crate::svg) target_y: u32,
  pub(in crate::svg) columns: u32,
  pub(in crate::svg) rows: u32,
  pub(in crate::svg) data: Vec<f32>,
}

impl Kernel {
  /// The kernel, none unless the entries fill it and the target is inside it.
  pub(in crate::svg) fn new(
    target_x: u32,
    target_y: u32,
    columns: u32,
    rows: u32,
    data: Vec<f32>,
  ) -> Option<Self> {
    let len = columns.checked_mul(rows)?;

    (len as usize == data.len() && target_x < columns && target_y < rows).then_some(Self {
      target_x,
      target_y,
      columns,
      rows,
      data,
    })
  }

  /// The entry at the position, which must be inside the kernel.
  pub(in crate::svg) fn get(&self, x: u32, y: u32) -> f32 {
    self.data[(y * self.columns + x) as usize]
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::svg) enum EdgeMode {
  None,
  Duplicate,
  Wrap,
}

#[derive(Debug)]
pub(in crate::svg) struct DisplacementMap {
  pub(in crate::svg) input1: Input,
  pub(in crate::svg) input2: Input,
  pub(in crate::svg) scale: f32,
  pub(in crate::svg) x_channel: ColorChannel,
  pub(in crate::svg) y_channel: ColorChannel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::svg) enum ColorChannel {
  R,
  G,
  B,
  A,
}

/// `feDropShadow`, which is `feGaussianBlur`, `feOffset`, and `feFlood` joined.
#[derive(Debug)]
pub(in crate::svg) struct DropShadow {
  pub(in crate::svg) input: Input,
  pub(in crate::svg) dx: f32,
  pub(in crate::svg) dy: f32,
  pub(in crate::svg) std_dev_x: f32,
  pub(in crate::svg) std_dev_y: f32,
  pub(in crate::svg) color: Color,
  pub(in crate::svg) opacity: f32,
}

#[derive(Debug)]
pub(in crate::svg) struct DiffuseLighting {
  pub(in crate::svg) input: Input,
  pub(in crate::svg) surface_scale: f32,
  pub(in crate::svg) diffuse_constant: f32,
  pub(in crate::svg) lighting_color: Color,
  pub(in crate::svg) light_source: LightSource,
}

#[derive(Debug)]
pub(in crate::svg) struct SpecularLighting {
  pub(in crate::svg) input: Input,
  pub(in crate::svg) surface_scale: f32,
  pub(in crate::svg) specular_constant: f32,
  /// In `1..=128`.
  pub(in crate::svg) specular_exponent: f32,
  pub(in crate::svg) lighting_color: Color,
  pub(in crate::svg) light_source: LightSource,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg) enum LightSource {
  Distant {
    /// The direction on the XY plane, clockwise from the x axis, in degrees.
    azimuth: f32,
    /// The direction from the XY plane towards the z axis, in degrees.
    elevation: f32,
  },
  Point {
    x: f32,
    y: f32,
    z: f32,
  },
  Spot(SpotLight),
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg) struct SpotLight {
  pub(in crate::svg) x: f32,
  pub(in crate::svg) y: f32,
  pub(in crate::svg) z: f32,
  pub(in crate::svg) points_at_x: f32,
  pub(in crate::svg) points_at_y: f32,
  pub(in crate::svg) points_at_z: f32,
  pub(in crate::svg) specular_exponent: f32,
  pub(in crate::svg) limiting_cone_angle: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::svg) enum MorphologyOperator {
  Erode,
  Dilate,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg) struct Turbulence {
  pub(in crate::svg) base_frequency_x: f32,
  pub(in crate::svg) base_frequency_y: f32,
  pub(in crate::svg) num_octaves: u32,
  pub(in crate::svg) seed: i32,
  pub(in crate::svg) stitch_tiles: bool,
  pub(in crate::svg) fractal_noise: bool,
}
