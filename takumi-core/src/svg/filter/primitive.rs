//! An SVG document's filters as Blink's `FilterEffect` graph, resolved to user space.

use crate::{
  filter::ColorMatrixChain,
  geometry::Rect,
  resources::image::SvgOp,
  style::{BlendMode, Color, TransferTable},
};

/// A filter: the region it paints within and the primitives that run in order.
#[derive(Debug)]
pub struct SvgFilter {
  /// The region the filter paints within, in the user space of the element it filters.
  pub region: Rect<f32>,
  /// The primitives, in the order they run.
  pub primitives: Vec<Primitive>,
}

impl SvgFilter {
  /// The region the filter paints within, in the user space of the element it filters.
  pub fn region(&self) -> Rect<f32> {
    self.region
  }

  /// The bytes the filter retains, for cache budgeting. Approximate: counts its primitives, their
  /// names and kernels, transfer tables, and image ops, not allocator slack.
  pub fn estimated_bytes(&self) -> usize {
    size_of::<Self>()
      + self
        .primitives
        .iter()
        .map(Primitive::estimated_bytes)
        .sum::<usize>()
  }
}

impl Primitive {
  fn estimated_bytes(&self) -> usize {
    size_of::<Self>() + self.result.len() + self.kind.estimated_bytes()
  }
}

impl Kind {
  /// The inputs the primitive reads, in the order it takes them.
  pub fn inputs(&self) -> Vec<&Input> {
    match self {
      Self::Blend { input1, input2, .. } | Self::Composite { input1, input2, .. } => {
        vec![input1, input2]
      }
      Self::DisplacementMap(fe) => vec![&fe.input1, &fe.input2],
      Self::Merge { inputs } => inputs.iter().collect(),
      Self::ColorMatrix { input, .. }
      | Self::ComponentTransfer { input, .. }
      | Self::GaussianBlur { input, .. }
      | Self::Morphology { input, .. }
      | Self::Offset { input, .. }
      | Self::Tile { input } => vec![input],
      Self::ConvolveMatrix(fe) => vec![&fe.input],
      Self::DiffuseLighting(fe) => vec![&fe.input],
      Self::SpecularLighting(fe) => vec![&fe.input],
      Self::DropShadow(fe) => vec![&fe.input],
      Self::Flood { .. } | Self::Image(_) | Self::Turbulence(_) => Vec::new(),
    }
  }

  /// The bytes the primitive's kind owns outside itself.
  fn estimated_bytes(&self) -> usize {
    match self {
      Self::Blend { input1, input2, .. } | Self::Composite { input1, input2, .. } => {
        input1.estimated_bytes() + input2.estimated_bytes()
      }
      Self::ColorMatrix { input, .. }
      | Self::GaussianBlur { input, .. }
      | Self::Morphology { input, .. }
      | Self::Offset { input, .. }
      | Self::Tile { input } => input.estimated_bytes(),
      Self::ComponentTransfer { input, tables } => {
        input.estimated_bytes() + size_of_val(tables.as_ref())
      }
      Self::ConvolveMatrix(convolve) => {
        convolve.input.estimated_bytes() + size_of_val(convolve.kernel.data.as_slice())
      }
      Self::DisplacementMap(map) => map.input1.estimated_bytes() + map.input2.estimated_bytes(),
      Self::DropShadow(shadow) => shadow.input.estimated_bytes(),
      Self::DiffuseLighting(lighting) => lighting.input.estimated_bytes(),
      Self::SpecularLighting(lighting) => lighting.input.estimated_bytes(),
      Self::Image(ops) => SvgOp::estimated_bytes(ops),
      Self::Merge { inputs } => inputs
        .iter()
        .map(|input| size_of::<Input>() + input.estimated_bytes())
        .sum(),
      _ => 0,
    }
  }
}

/// One filter primitive element, such as `feGaussianBlur`.
#[derive(Debug)]
pub struct Primitive {
  /// The region the primitive's result covers, in user space.
  pub subregion: Rect<f32>,
  /// The space the primitive computes in.
  pub color_space: ColorSpace,
  /// The name later primitives read this one's result by.
  pub result: String,
  /// What the primitive does.
  pub kind: Kind,
}

/// The space a primitive computes in, from `color-interpolation-filters`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
  /// `sRGB`.
  Srgb,
  /// `linearRGB`.
  LinearRgb,
}

/// The operation of a [`Primitive`], one variant per filter primitive element.
#[derive(Debug)]
pub enum Kind {
  /// `feBlend`.
  Blend {
    /// The top layer, `in`.
    input1: Input,
    /// The bottom layer, `in2`.
    input2: Input,
    /// The blend mode.
    mode: BlendMode,
  },
  /// `feColorMatrix`.
  ColorMatrix {
    /// The input.
    input: Input,
    /// The matrices applied in order.
    chain: ColorMatrixChain,
  },
  /// `feComponentTransfer`.
  ComponentTransfer {
    /// The input.
    input: Input,
    /// The tables of the red, green, blue, and alpha channels, none for one left as it is.
    tables: Box<[Option<TransferTable>; 4]>,
  },
  /// `feComposite`.
  Composite {
    /// The source, `in`.
    input1: Input,
    /// The destination, `in2`.
    input2: Input,
    /// How the two combine.
    operator: CompositeOperator,
  },
  /// `feConvolveMatrix`.
  ConvolveMatrix(ConvolveMatrix),
  /// `feDiffuseLighting`.
  DiffuseLighting(DiffuseLighting),
  /// `feDisplacementMap`.
  DisplacementMap(DisplacementMap),
  /// `feDropShadow`.
  DropShadow(DropShadow),
  /// `feFlood`.
  Flood {
    /// The flood colour at full alpha, since `opacity` holds the authored alpha.
    color: Color,
    /// The authored colour's alpha times `flood-opacity`, in `0..=1`.
    opacity: f32,
  },
  /// `feGaussianBlur`.
  GaussianBlur {
    /// The input.
    input: Input,
    /// The horizontal standard deviation in user units.
    std_dev_x: f32,
    /// The vertical standard deviation in user units.
    std_dev_y: f32,
  },
  /// An image or an element, drawn as the ops of its content.
  Image(Vec<SvgOp>),
  /// `feMerge`.
  Merge {
    /// The layers, bottom first.
    inputs: Vec<Input>,
  },
  /// `feMorphology`.
  Morphology {
    /// The input.
    input: Input,
    /// Whether it thins or thickens.
    operator: MorphologyOperator,
    /// The horizontal radius in user units.
    radius_x: f32,
    /// The vertical radius in user units.
    radius_y: f32,
  },
  /// `feOffset`.
  Offset {
    /// The input.
    input: Input,
    /// The horizontal shift in user units.
    dx: f32,
    /// The vertical shift in user units.
    dy: f32,
  },
  /// `feSpecularLighting`.
  SpecularLighting(SpecularLighting),
  /// `feTile`.
  Tile {
    /// The tile, read within its subregion.
    input: Input,
  },
  /// `feTurbulence`.
  Turbulence(Turbulence),
}

/// What a primitive reads: the filtered graphic, its alpha, or the result of a primitive.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
  /// `SourceGraphic`.
  SourceGraphic,
  /// `SourceAlpha`.
  SourceAlpha,
  /// The result of the primitive with this name.
  Reference(String),
}

impl Input {
  /// The bytes of the result name it reads.
  fn estimated_bytes(&self) -> usize {
    match self {
      Self::Reference(name) => name.len(),
      _ => 0,
    }
  }

  /// Whether the input is the result named `name`.
  pub fn names(&self, name: &str) -> bool {
    matches!(self, Self::Reference(reference) if reference == name)
  }
}

/// An `feComposite` operator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompositeOperator {
  /// `over`.
  Over,
  /// `in`.
  In,
  /// `out`.
  Out,
  /// `atop`.
  Atop,
  /// `xor`.
  Xor,
  /// `arithmetic`, `k1 * i1 * i2 + k2 * i1 + k3 * i2 + k4`.
  Arithmetic {
    /// The weight of the product.
    k1: f32,
    /// The weight of the source.
    k2: f32,
    /// The weight of the destination.
    k3: f32,
    /// The constant.
    k4: f32,
  },
}

/// `feConvolveMatrix`.
#[derive(Debug)]
pub struct ConvolveMatrix {
  /// The input.
  pub input: Input,
  /// The kernel.
  pub kernel: Kernel,
  /// What the weighted sum is divided by.
  pub divisor: f32,
  /// What is added after the division.
  pub bias: f32,
  /// How pixels past the input's edge read.
  pub edge_mode: EdgeMode,
  /// Whether alpha passes through unconvolved.
  pub preserve_alpha: bool,
}

/// The kernel of a convolution: `columns` by `rows` entries, with the target among them.
#[derive(Debug)]
pub struct Kernel {
  target_x: u32,
  target_y: u32,
  columns: u32,
  rows: u32,
  data: Vec<f32>,
}

impl Kernel {
  /// The kernel, none unless the entries fill it and the target is inside it.
  pub fn new(
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

  /// The column of the target.
  pub fn target_x(&self) -> u32 {
    self.target_x
  }

  /// The row of the target.
  pub fn target_y(&self) -> u32 {
    self.target_y
  }

  /// The number of columns.
  pub fn columns(&self) -> u32 {
    self.columns
  }

  /// The number of rows.
  pub fn rows(&self) -> u32 {
    self.rows
  }

  /// The entries, row by row.
  pub fn data(&self) -> &[f32] {
    &self.data
  }
}

/// How a convolution reads past the input's edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EdgeMode {
  /// As transparent black.
  None,
  /// As the nearest edge pixel.
  Duplicate,
  /// As the pixel on the opposite edge.
  Wrap,
}

/// `feDisplacementMap`.
#[derive(Debug)]
pub struct DisplacementMap {
  /// The image displaced, `in`.
  pub input1: Input,
  /// The map, `in2`.
  pub input2: Input,
  /// The coefficient of `channel - 0.5` that gives the displacement, in user units.
  pub scale: f32,
  /// The channel that moves pixels horizontally.
  pub x_channel: ColorChannel,
  /// The channel that moves pixels vertically.
  pub y_channel: ColorChannel,
}

/// A colour channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorChannel {
  /// Red.
  R,
  /// Green.
  G,
  /// Blue.
  B,
  /// Alpha.
  A,
}

/// `feDropShadow`, which is `feGaussianBlur`, `feOffset`, and `feFlood` joined.
#[derive(Debug)]
pub struct DropShadow {
  /// The input.
  pub input: Input,
  /// The horizontal offset in user units.
  pub dx: f32,
  /// The vertical offset in user units.
  pub dy: f32,
  /// The horizontal standard deviation in user units.
  pub std_dev_x: f32,
  /// The vertical standard deviation in user units.
  pub std_dev_y: f32,
  /// The shadow colour at full alpha, since `opacity` holds the authored alpha.
  pub color: Color,
  /// The authored colour's alpha times `flood-opacity`, in `0..=1`.
  pub opacity: f32,
}

/// `feDiffuseLighting`.
#[derive(Debug)]
pub struct DiffuseLighting {
  /// The input, whose alpha is the surface height.
  pub input: Input,
  /// The height of a full alpha.
  pub surface_scale: f32,
  /// The diffuse reflectance.
  pub diffuse_constant: f32,
  /// The light's colour.
  pub lighting_color: Color,
  /// The light.
  pub light_source: LightSource,
}

/// `feSpecularLighting`.
#[derive(Debug)]
pub struct SpecularLighting {
  /// The input, whose alpha is the surface height.
  pub input: Input,
  /// The height of a full alpha.
  pub surface_scale: f32,
  /// The specular reflectance.
  pub specular_constant: f32,
  /// The shininess, in `1..=128`.
  pub specular_exponent: f32,
  /// The light's colour.
  pub lighting_color: Color,
  /// The light.
  pub light_source: LightSource,
}

/// The light of a lighting primitive.
#[derive(Debug, Clone, Copy)]
pub enum LightSource {
  /// `feDistantLight`.
  Distant {
    /// The direction on the XY plane, clockwise from the x axis, in degrees.
    azimuth: f32,
    /// The direction from the XY plane towards the z axis, in degrees.
    elevation: f32,
  },
  /// `fePointLight`.
  Point {
    /// The x position in user units.
    x: f32,
    /// The y position in user units.
    y: f32,
    /// The z position in user units, from which the surface height is subtracted.
    z: f32,
  },
  /// `feSpotLight`.
  Spot(SpotLight),
}

/// `feSpotLight`.
#[derive(Debug, Clone, Copy)]
pub struct SpotLight {
  /// The x position in user units.
  pub x: f32,
  /// The y position in user units.
  pub y: f32,
  /// The z position in user units, from which the surface height is subtracted.
  pub z: f32,
  /// The x of the point it aims at.
  pub points_at_x: f32,
  /// The y of the point it aims at.
  pub points_at_y: f32,
  /// The z of the point it aims at.
  pub points_at_z: f32,
  /// How fast the light falls off away from its axis.
  pub specular_exponent: f32,
  /// The half-angle of the cone the light stays inside, in degrees.
  pub limiting_cone_angle: Option<f32>,
}

/// An `feMorphology` operator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MorphologyOperator {
  /// Thins the input.
  Erode,
  /// Thickens the input.
  Dilate,
}

/// `feTurbulence`.
#[derive(Debug, Clone, Copy)]
pub struct Turbulence {
  /// The horizontal base frequency.
  pub base_frequency_x: f32,
  /// The vertical base frequency.
  pub base_frequency_y: f32,
  /// The number of noise octaves.
  pub num_octaves: u32,
  /// The seed of the noise.
  pub seed: i32,
  /// Whether the noise tiles seamlessly across the subregion.
  pub stitch_tiles: bool,
  /// Whether it is fractal noise rather than turbulence.
  pub fractal_noise: bool,
}
