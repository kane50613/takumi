//! Resolved CSS values turned into the document's shapes, paints, and filters.

use std::{
  array,
  f32::consts::{FRAC_PI_2, TAU},
  mem,
};

use super::document::{
  ColorStop, CornerRadii, FillRuleName, ImageSource, Paint, PaintFilter, PaintPoint, PaintRect,
  Sampling, Shape,
};
use crate::{
  context::RenderContext,
  filter::{ColorMatrix, ColorMatrixChain},
  geometry::{Point, Rect, Size},
  layout::{
    background_image_geometry::{BackgroundLayer, OriginBox},
    node::resolve_image,
  },
  paint::SrgbStop,
  painter::{FillShape, UNBOUNDED},
  path_data::path_data,
  shadow::SizedShadow,
  style::{
    Affine, BackgroundImage, BlendMode, Color, ColorInterpolationMethod, ConicGradient, FillRule,
    Filter, ImageScalingAlgorithm, LinearGradient, RadialGradient, ResolvedGradientStop, ToCss,
  },
};

impl From<Point<f32>> for PaintPoint {
  fn from(point: Point<f32>) -> Self {
    Self {
      x: point.x,
      y: point.y,
    }
  }
}

impl From<Rect<f32>> for PaintRect {
  fn from(edges: Rect<f32>) -> Self {
    Self {
      x: edges.left,
      y: edges.top,
      width: edges.right - edges.left,
      height: edges.bottom - edges.top,
    }
  }
}

impl PaintRect {
  /// A `size` rectangle at `origin`.
  pub(super) fn sized(origin: Point<f32>, size: Size<f32>) -> Self {
    Self {
      x: origin.x,
      y: origin.y,
      width: size.width,
      height: size.height,
    }
  }

  /// The rectangle a `width` × `height` box covers once `transform` maps it.
  pub(super) fn bounding(width: f32, height: f32, transform: Affine) -> Self {
    let corners = [(0.0, 0.0), (width, 0.0), (0.0, height), (width, height)]
      .map(|(x, y)| transform.transform_point(x, y));
    let [left, top, right, bottom] = corners.iter().fold(
      [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
      ],
      |[left, top, right, bottom], &(x, y)| [left.min(x), top.min(y), right.max(x), bottom.max(y)],
    );

    Self::from(Rect {
      left,
      top,
      right,
      bottom,
    })
  }
}

impl From<FillRule> for FillRuleName {
  fn from(rule: FillRule) -> Self {
    match rule {
      FillRule::NonZero => Self::Nonzero,
      FillRule::EvenOdd => Self::Evenodd,
    }
  }
}

impl Shape {
  /// `shape` under `transform`: a rectangle while it stays one, SVG path data otherwise.
  pub(super) fn of(shape: &FillShape, transform: Affine) -> Self {
    if transform.only_translation() {
      let at = Point {
        x: transform.x,
        y: transform.y,
      };

      match shape {
        FillShape::Rect(size) => {
          return Self::Rect {
            rect: PaintRect::sized(at, *size),
          };
        }
        FillShape::RoundedRect {
          border,
          size,
          offset,
        } if border.shape.0.iter().all(|shape| shape.is_round()) => {
          let rect = PaintRect::sized(at + *offset, *size);

          if border.is_zero() {
            return Self::Rect { rect };
          }

          let [top_left, top_right, bottom_right, bottom_left] = border
            .scaled_corner_radii(*size)
            .0
            .map(|radius| PaintPoint {
              x: radius.x,
              y: radius.y,
            });

          return Self::RoundedRect {
            rect,
            radii: CornerRadii {
              top_left,
              top_right,
              bottom_right,
              bottom_left,
            },
          };
        }
        _ => {}
      }
    }

    Self::Path {
      d: path_data(&shape.to_commands(), transform),
      fill_rule: shape.rule().into(),
    }
  }

  /// Everything outside `shape` under `transform`, out to [`UNBOUNDED`].
  pub(super) fn outside(shape: &FillShape, transform: Affine) -> Self {
    let everywhere = FillShape::Rect(Size {
      width: 2.0 * UNBOUNDED,
      height: 2.0 * UNBOUNDED,
    });
    let far = Affine::translation(-UNBOUNDED, -UNBOUNDED);

    Self::Path {
      d: path_data(&everywhere.to_commands(), far) + &path_data(&shape.to_commands(), transform),
      fill_rule: FillRuleName::Evenodd,
    }
  }
}

impl ColorStop {
  fn of(stop: SrgbStop) -> Self {
    Self {
      offset: stop.offset,
      color: stop.color.0,
    }
  }
}

/// A gradient's stops over one period, unrolled across `0..length` of its line.
struct Period {
  /// Stops within the period, offsets from 0 to 1.
  stops: Vec<SrgbStop>,
  /// Where the first period starts on the line.
  start: f32,
  /// The period's length.
  length: f32,
}

impl Period {
  /// The period a repeating gradient's `resolved` stops span, their colours sampled in
  /// `interpolation`; `None` when the stops span nothing.
  fn of(
    resolved: &[ResolvedGradientStop],
    interpolation: ColorInterpolationMethod,
  ) -> Option<Self> {
    let start = resolved.first()?.position;
    let length = resolved.last()?.position - start;

    if length <= 1e-3 {
      return None;
    }

    let shifted: Vec<ResolvedGradientStop> = resolved
      .iter()
      .map(|stop| ResolvedGradientStop {
        position: stop.position - start,
        ..*stop
      })
      .collect();

    Some(Self {
      stops: SrgbStop::sampled(&shifted, length, interpolation),
      start,
      length,
    })
  }

  /// The periods covering `0..line`, as stops from 0 to 1 along it.
  fn unroll(&self, line: f32) -> Vec<ColorStop> {
    let first = ((0.0 - self.start) / self.length).floor() as i32;
    let last = ((line - self.start) / self.length).ceil() as i32;
    let unrolled: Vec<SrgbStop> = (first..last)
      .flat_map(|period| {
        self.stops.iter().map(move |stop| SrgbStop {
          offset: (self.start + (period as f32 + stop.offset) * self.length) / line,
          ..*stop
        })
      })
      .collect();

    compact(clamp_to_unit(&unrolled))
  }
}

/// `stops` cut to offsets 0 to 1, with the colours at each end interpolated in.
fn clamp_to_unit(stops: &[SrgbStop]) -> Vec<SrgbStop> {
  let mut clamped = Vec::with_capacity(stops.len());

  for (index, stop) in stops.iter().enumerate() {
    let previous = index.checked_sub(1).and_then(|index| stops.get(index));

    for edge in [0.0, 1.0] {
      if let Some(previous) = previous
        && previous.offset < edge
        && stop.offset > edge
      {
        clamped.push(SrgbStop {
          offset: edge,
          color: mix(previous, stop, edge),
        });
      }
    }
    if (0.0..=1.0).contains(&stop.offset) {
      clamped.push(*stop);
    }
  }
  clamped
}

/// The colour at `offset` between `from` and `to`, interpolated in straight sRGB.
fn mix(from: &SrgbStop, to: &SrgbStop, offset: f32) -> Color {
  let t = (offset - from.offset) / (to.offset - from.offset).max(1e-6);

  Color(array::from_fn(|channel| {
    let [from, to] = [from.color.0[channel], to.color.0[channel]].map(f32::from);

    (from + (to - from) * t).round() as u8
  }))
}

/// `stops` without the ones inside a run of the same colour, which change nothing.
fn compact(stops: Vec<SrgbStop>) -> Vec<ColorStop> {
  let kept = |index: usize| {
    let color = stops[index].color;
    let same = |other: Option<&SrgbStop>| other.is_some_and(|other| other.color == color);

    !(index > 0 && same(stops.get(index - 1)) && same(stops.get(index + 1)))
  };

  (0..stops.len())
    .filter(|&index| kept(index))
    .map(|index| ColorStop::of(stops[index]))
    .collect()
}

impl Paint {
  /// One `width` × `height` tile of `image`, in the tile's space, or `None` when it paints
  /// nothing.
  pub(super) fn tile(
    image: &BackgroundImage,
    width: u32,
    height: u32,
    context: &RenderContext,
  ) -> Option<Self> {
    match image {
      BackgroundImage::None => None,
      BackgroundImage::Linear(gradient) => Self::linear(gradient, width, height, context),
      BackgroundImage::Radial(gradient) => Self::radial(gradient, width, height, context),
      BackgroundImage::Conic(gradient) => Self::conic(gradient, width, height, context),
      BackgroundImage::Url(src) => {
        let (width, height) = resolve_image(src, context).ok()?.size(&context.sizing);

        Some(Self::Image {
          image: ImageSource {
            src: src.to_string(),
            width,
            height,
          },
          sampling: Sampling::of(context.style.image_rendering),
        })
      }
    }
  }

  fn linear(
    gradient: &LinearGradient,
    width: u32,
    height: u32,
    context: &RenderContext,
  ) -> Option<Self> {
    let geometry = gradient.resolve_geometry(width, height, &context.sizing, context.current_color);
    let resolved = geometry.stops();

    resolved.first()?;

    let line = geometry.axis_length;
    let half = line / 2.0;
    let at = |t: f32| PaintPoint {
      x: width as f32 / 2.0 + (t - half) * geometry.dir_x,
      y: height as f32 / 2.0 + (t - half) * geometry.dir_y,
    };
    let stops = match gradient
      .repeating
      .then(|| Period::of(resolved, gradient.interpolation))
      .flatten()
    {
      Some(period) => period.unroll(line),
      None => compact(SrgbStop::sampled(resolved, line, gradient.interpolation)),
    };

    Some(Self::LinearGradient {
      start: at(0.0),
      end: at(line),
      stops,
    })
  }

  fn radial(
    gradient: &RadialGradient,
    width: u32,
    height: u32,
    context: &RenderContext,
  ) -> Option<Self> {
    let geometry = gradient.resolve_geometry(width, height, &context.sizing, context.current_color);
    let resolved = geometry.stops();

    resolved.first()?;

    let scale = geometry.radius_scale.max(1e-6);
    let [radius_x, radius_y] = [geometry.inv_radius_x, geometry.inv_radius_y].map(f32::recip);
    let center = PaintPoint {
      x: geometry.cx,
      y: geometry.cy,
    };
    let (line, stops) = match gradient
      .repeating
      .then(|| Period::of(resolved, gradient.interpolation))
      .flatten()
    {
      Some(period) => {
        // How far along the gradient the tile's farthest corner sits.
        let reach = [(0.0, 0.0), (width as f32, 0.0), (0.0, height as f32)]
          .into_iter()
          .chain([(width as f32, height as f32)])
          .map(|(x, y)| ((x - center.x) / radius_x).hypot((y - center.y) / radius_y) * scale)
          .fold(scale, f32::max);

        (reach, period.unroll(reach))
      }
      None => (
        scale,
        compact(SrgbStop::sampled(resolved, scale, gradient.interpolation)),
      ),
    };

    Some(Self::RadialGradient {
      center,
      radius_x: radius_x * line / scale,
      radius_y: radius_y * line / scale,
      stops,
    })
  }

  fn conic(
    gradient: &ConicGradient,
    width: u32,
    height: u32,
    context: &RenderContext,
  ) -> Option<Self> {
    let resolved = gradient.resolve_stops(&context.sizing, context.current_color);
    let turn = TAU.to_degrees();

    resolved.first()?;

    let stops = match gradient
      .repeating
      .then(|| Period::of(&resolved, gradient.interpolation))
      .flatten()
    {
      Some(period) => period.unroll(turn),
      None => compact(SrgbStop::sampled(&resolved, turn, gradient.interpolation)),
    };
    let (x, y) = gradient.resolve_center(width as f32, height as f32, &context.sizing);

    Some(Self::ConicGradient {
      center: PaintPoint { x, y },
      start_angle: gradient.from_angle.to_radians() - FRAC_PI_2,
      stops,
    })
  }

  /// Background or mask `layers` over a border box of `size`, their positioning area at
  /// `origin`, each with its `background-blend-mode`.
  pub(super) fn layers(
    layers: &[BackgroundLayer<'_>],
    size: Size<f32>,
    origin: OriginBox,
    context: &RenderContext,
  ) -> Vec<(Self, Option<String>)> {
    layers
      .iter()
      .filter_map(|layer| {
        let tiles = layer.geometry.snap(size, origin.offset)?;
        let tile = Self::tile(layer.image, tiles.width, tiles.height, context)?;
        let blend_mode =
          (layer.blend_mode != BlendMode::Normal).then(|| layer.blend_mode.to_css_string());

        Some((
          Self::Pattern {
            tile: Box::new(tile),
            tile_width: tiles.width as f32,
            tile_height: tiles.height as f32,
            x: tiles.xs.iter().map(|&x| x as f32).collect(),
            y: tiles.ys.iter().map(|&y| y as f32).collect(),
          },
          blend_mode,
        ))
      })
      .collect()
  }
}

impl Sampling {
  /// How `image-rendering` samples.
  pub(super) fn of(algorithm: ImageScalingAlgorithm) -> Self {
    match algorithm {
      ImageScalingAlgorithm::Pixelated => Self::Pixelated,
      _ => Self::Smooth,
    }
  }
}

impl PaintFilter {
  /// `filters` resolved for a box of `size`, neighbouring colour matrices folded into one where
  /// they compose exactly.
  pub(super) fn chain(filters: &[Filter], size: Size<f32>, context: &RenderContext) -> Vec<Self> {
    let mut resolved = Vec::with_capacity(filters.len());
    let mut matrices = ColorMatrixChain::default();

    for filter in filters {
      if let Some(matrix) = ColorMatrix::from_filter(filter) {
        matrices.push(matrix);
        continue;
      }

      resolved.extend(Self::matrices(&mut matrices));
      resolved.push(match filter {
        Filter::Blur(radius) => Self::Blur {
          radius: radius.to_px(&context.sizing, 1.0),
        },
        Filter::DropShadow(shadow) => {
          let shadow =
            SizedShadow::from_text_shadow(*shadow, &context.sizing, context.current_color, size);

          Self::DropShadow {
            offset: PaintPoint {
              x: shadow.offset_x,
              y: shadow.offset_y,
            },
            blur: shadow.blur_radius,
            color: shadow.color.0,
          }
        }
        _ => Self::Unsupported {
          css: filter.to_css_string(),
        },
      });
    }
    resolved.extend(Self::matrices(&mut matrices));
    resolved
  }

  /// The colour matrices `chain` holds, leaving it empty.
  fn matrices(chain: &mut ColorMatrixChain) -> impl Iterator<Item = Self> {
    mem::take(&mut chain.0)
      .into_iter()
      .map(|matrix| Self::ColorMatrix {
        matrix: matrix.fe_color_matrix_values().to_vec(),
      })
  }
}
