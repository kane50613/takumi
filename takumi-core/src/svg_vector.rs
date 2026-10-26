//! The drawing ops every backend draws an SVG image from.

use std::sync::Arc;

#[cfg(feature = "svg-raster")]
use crate::svg::filter::SvgFilter;
use crate::{
  geometry::{PathCommand, Point, Rect},
  resources::{image_buffer::ImageBuffer, image_decoder::decode_image},
  style::{Affine, BlendMode, Color, ImageScalingAlgorithm},
};

/// One flattened SVG drawing instruction, in SVG canvas coordinates.
#[derive(Debug, Clone)]
pub enum SvgOp {
  /// Applies an affine transform to nested ops.
  PushTransform(Affine),
  /// Clips nested ops to a path.
  PushClip {
    /// Clip outline.
    path: Vec<PathCommand>,
    /// `true` for the even-odd fill rule, `false` for nonzero.
    evenodd: bool,
  },
  /// Blends nested ops onto the backdrop with a blend mode.
  PushBlend(BlendMode),
  /// Composites nested ops as one isolated group, so their blend modes stop at its backdrop.
  PushIsolation {
    /// Conservative bounds of the nested paint in the group's coordinate space.
    bounds: Option<Rect<f32>>,
  },
  /// Applies uniform opacity to nested ops as one isolated group.
  PushOpacity(f32),
  /// Masks nested ops with the rendering of `ops` (an alpha or luminance soft mask).
  PushMask {
    /// Mask content, flattened recursively.
    ops: Vec<SvgOp>,
    /// `true` to mask by luminance, `false` by alpha.
    luminance: bool,
  },
  /// Filters nested ops as one layer, in the coordinate space the layer is pushed in.
  PushFilter(SvgFilters),
  /// Closes the innermost open `Push*` layer.
  Pop,
  /// Fills and/or strokes a path.
  Draw {
    /// Path outline in the current coordinate space.
    path: Vec<PathCommand>,
    /// Fill paint, if any.
    fill: Option<SvgFill>,
    /// Stroke paint, if any.
    stroke: Option<SvgStrokeStyle>,
    /// `false` for `shape-rendering: crispEdges` and `optimizeSpeed`.
    anti_alias: bool,
  },
  /// Draws a bitmap over `(0, 0, width, height)` in the current coordinate space.
  Image {
    /// Bitmap pixels.
    data: SvgImageData,
    /// Width in user units.
    width: f32,
    /// Height in user units.
    height: f32,
    /// How the bitmap resamples.
    rendering: ImageScalingAlgorithm,
  },
}

/// The pixels of a bitmap inside an SVG.
#[derive(Debug, Clone)]
pub enum SvgImageData {
  /// PNG, JPEG or WebP bytes, as the SVG carries them.
  Encoded(Arc<Vec<u8>>),
  /// Decoded pixels, for a GIF's first frame or a bitmap glyph.
  Bitmap(ImageBuffer),
}

impl SvgImageData {
  /// The decoded pixels, or `None` when the bytes do not decode.
  pub fn decode(&self) -> Option<ImageBuffer> {
    match self {
      Self::Encoded(bytes) => decode_image(bytes).ok(),
      Self::Bitmap(buffer) => Some(buffer.clone()),
    }
  }
}

/// The SVG filters of one group, applied to its layer.
#[derive(Debug, Clone)]
pub struct SvgFilters(#[cfg(feature = "svg-raster")] Vec<Arc<SvgFilter>>);

#[cfg(feature = "svg-raster")]
impl SvgFilters {
  pub(crate) fn new(filters: Vec<Arc<SvgFilter>>) -> Self {
    Self(filters)
  }

  /// The union of the filter regions in the group's coordinate space.
  pub fn region(&self) -> Option<tiny_skia::Rect> {
    self
      .0
      .iter()
      .map(|filter| filter.region())
      .reduce(|union, region| union.union(&region))
      .and_then(|region| {
        tiny_skia::Rect::from_ltrb(region.left, region.top, region.right, region.bottom)
      })
  }

  /// Applies each filter in order to `layer`, drawn under `transform`.
  pub fn apply(&self, transform: tiny_skia::Transform, layer: &mut tiny_skia::Pixmap) {
    for filter in &self.0 {
      filter.apply(transform, layer);
    }
  }
}

/// Fill style for [`SvgOp::Draw`].
#[derive(Debug, Clone)]
pub struct SvgFill {
  /// Fill paint.
  pub paint: SvgOpPaint,
  /// Fill opacity in `0..=1`.
  pub opacity: f32,
  /// `true` for the even-odd fill rule, `false` for nonzero.
  pub evenodd: bool,
}

/// Stroke style for [`SvgOp::Draw`].
#[derive(Debug, Clone)]
pub struct SvgStrokeStyle {
  /// Stroke paint.
  pub paint: SvgOpPaint,
  /// Stroke opacity in `0..=1`.
  pub opacity: f32,
  /// Stroke width in user units.
  pub width: f32,
  /// Miter limit.
  pub miter_limit: f32,
  /// Line cap.
  pub cap: SvgLineCap,
  /// Line join.
  pub join: SvgLineJoin,
  /// Dash pattern as `(array, offset)`.
  pub dash: Option<(Vec<f32>, f32)>,
}

/// Line cap for [`SvgStrokeStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgLineCap {
  /// Flat edge at the endpoint.
  Butt,
  /// Semicircle past the endpoint.
  Round,
  /// Half-square past the endpoint.
  Square,
}

/// Line join for [`SvgStrokeStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgLineJoin {
  /// Sharp corner, which becomes a bevel past the miter limit.
  Miter,
  /// Rounded corner.
  Round,
  /// Beveled corner.
  Bevel,
}

/// Paint for fills and strokes.
#[derive(Debug, Clone)]
pub enum SvgOpPaint {
  /// Opaque sRGB color.
  Color(Color),
  /// Linear gradient from `start` to `end` in user space.
  Linear {
    /// Gradient line start.
    start: Point<f32>,
    /// Gradient line end.
    end: Point<f32>,
    /// Shared gradient parameters.
    gradient: SvgGradient,
  },
  /// Radial gradient with a focal circle.
  Radial {
    /// Center.
    center: Point<f32>,
    /// Radius.
    radius: f32,
    /// Focal point.
    focal: Point<f32>,
    /// Focal radius.
    focal_radius: f32,
    /// Shared gradient parameters.
    gradient: SvgGradient,
  },
  /// Tiling pattern: `ops` draw one `width` x `height` tile placed by `transform`.
  Pattern {
    /// Tile content, flattened recursively.
    ops: Vec<SvgOp>,
    /// Pattern space transform.
    transform: Affine,
    /// Tile width.
    width: f32,
    /// Tile height.
    height: f32,
  },
}

/// Parameters shared by both gradient kinds.
#[derive(Debug, Clone)]
pub struct SvgGradient {
  /// Extra gradient transform.
  pub transform: Affine,
  /// How the gradient extends past its bounds.
  pub spread: SvgSpreadMethod,
  /// Color stops, offsets ascending in `0..=1`.
  pub stops: Vec<SvgGradientStop>,
}

/// Gradient spread method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgSpreadMethod {
  /// Clamp to the edge stops.
  Pad,
  /// Mirror-repeat.
  Reflect,
  /// Repeat.
  Repeat,
}

/// One gradient color stop.
#[derive(Debug, Clone)]
pub struct SvgGradientStop {
  /// Offset in `0..=1`.
  pub offset: f32,
  /// Opaque sRGB color.
  pub color: Color,
  /// Stop opacity in `0..=1`.
  pub opacity: f32,
}
