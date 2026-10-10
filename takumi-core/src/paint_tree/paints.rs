//! Resolved CSS values turned into the document's shapes, paints, and filters.

use std::{
  f32::consts::{FRAC_PI_2, TAU},
  mem,
};

use super::document::{
  ColorStop, CornerRadii, FillRuleName, ImageSource, Paint, PaintFilter, PaintPoint, PaintRect,
  Sampling, Shape, Spread,
};
use crate::{
  context::RenderContext,
  filter::{ColorMatrix, ColorMatrixChain},
  geometry::{Point, Rect, Size},
  layout::{background_image_geometry::BackgroundLayer, node::resolve_image},
  paint::SrgbStop,
  painter::{FillShape, UNBOUNDED},
  path_data::path_data,
  shadow::SizedShadow,
  style::properties::gradient_utils::LutAxis,
  style::{
    Affine, BackgroundImage, BlendMode, ColorInterpolationMethod, ConicGradient, FillRule, Filter,
    ImageScalingAlgorithm, LinearGradient, RadialGradient, ResolvedGradientStop, ToCss,
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

/// How a gradient's stops lay over its line, after Blink's `NormalizeAndAddStops`.
enum GradientStops {
  /// Once over the whole line, padded past its ends.
  Once(Vec<ColorStop>),
  /// Over one period, repeated both ways along the line.
  Repeating {
    stops: Vec<ColorStop>,
    /// Where the period starts on the line.
    start: f32,
    /// The period's length.
    length: f32,
  },
}

impl GradientStops {
  /// `resolved` stops over a `line`, their colours sampled in `interpolation`, periods found as
  /// the raster backend's [`LutAxis`] finds them.
  fn of(
    resolved: &[ResolvedGradientStop],
    line: f32,
    repeating: bool,
    interpolation: ColorInterpolationMethod,
  ) -> Option<Self> {
    resolved.first()?;

    let axis = LutAxis::new(repeating, resolved.iter().cloned().collect(), line);
    let stops = compact(SrgbStop::sampled(&axis.stops, axis.length, interpolation));

    Some(if axis.repeating {
      Self::Repeating {
        stops,
        start: axis.repeat_start,
        length: axis.repeat_period,
      }
    } else {
      Self::Once(stops)
    })
  }
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
    width: f32,
    height: f32,
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
          sampling: Sampling::of(context.style.rare_inherited_data.image_rendering),
        })
      }
    }
  }

  fn linear(
    gradient: &LinearGradient,
    width: f32,
    height: f32,
    context: &RenderContext,
  ) -> Option<Self> {
    let geometry = gradient.resolve_geometry(width, height, &context.sizing, context.current_color);
    let line = geometry.axis_length;
    let half = line / 2.0;
    let at = |t: f32| PaintPoint {
      x: width / 2.0 + (t - half) * geometry.dir_x,
      y: height / 2.0 + (t - half) * geometry.dir_y,
    };

    Some(
      match GradientStops::of(
        geometry.stops(),
        line,
        gradient.repeating,
        gradient.interpolation,
      )? {
        GradientStops::Once(stops) => Self::LinearGradient {
          start: at(0.0),
          end: at(line),
          stops,
          spread: Spread::Pad,
        },
        GradientStops::Repeating {
          stops,
          start,
          length,
        } => Self::LinearGradient {
          start: at(start),
          end: at(start + length),
          stops,
          spread: Spread::Repeat,
        },
      },
    )
  }

  fn radial(
    gradient: &RadialGradient,
    width: f32,
    height: f32,
    context: &RenderContext,
  ) -> Option<Self> {
    let geometry = gradient.resolve_geometry(width, height, &context.sizing, context.current_color);
    let scale = geometry.radius_scale.max(1e-6);
    let [radius_x, radius_y] = [geometry.inv_radius_x, geometry.inv_radius_y].map(f32::recip);
    let center = PaintPoint {
      x: geometry.cx,
      y: geometry.cy,
    };

    Some(
      match GradientStops::of(
        geometry.stops(),
        scale,
        gradient.repeating,
        gradient.interpolation,
      )? {
        GradientStops::Once(stops) => Self::RadialGradient {
          center,
          radius_x,
          radius_y,
          start: 0.0,
          stops,
          spread: Spread::Pad,
        },
        GradientStops::Repeating {
          stops,
          start,
          length,
        } => {
          // Blink's `AdjustGradientRadiiForOffsetRange`: a period starting inside the centre
          // moves out by whole periods, which repeating hides.
          let [inner, outer] = [start, start + length].map(|position| position / scale);
          let span = outer - inner;
          let shift = if inner < 0.0 {
            span * (-inner / span).ceil()
          } else {
            0.0
          };
          let [inner, outer] = [inner + shift, outer + shift];

          Self::RadialGradient {
            center,
            radius_x: radius_x * outer,
            radius_y: radius_y * outer,
            start: inner / outer,
            stops,
            spread: Spread::Repeat,
          }
        }
      },
    )
  }

  fn conic(
    gradient: &ConicGradient,
    width: f32,
    height: f32,
    context: &RenderContext,
  ) -> Option<Self> {
    let resolved = gradient.resolve_stops(&context.sizing, context.current_color);
    let (x, y) = gradient.resolve_center(width, height, &context.sizing);
    let center = PaintPoint { x, y };
    let from = gradient.from_angle.to_radians() - FRAC_PI_2;

    Some(
      match GradientStops::of(
        &resolved,
        TAU.to_degrees(),
        gradient.repeating,
        gradient.interpolation,
      )? {
        GradientStops::Once(stops) => Self::ConicGradient {
          center,
          start_angle: from,
          end_angle: from + TAU,
          stops,
          spread: Spread::Pad,
        },
        GradientStops::Repeating {
          stops,
          start,
          length,
        } => Self::ConicGradient {
          center,
          start_angle: from + start.to_radians(),
          end_angle: from + (start + length).to_radians(),
          stops,
          spread: Spread::Repeat,
        },
      },
    )
  }

  /// The paint moved by `offset`, its tiles with it.
  pub(super) fn shifted(self, offset: Point<f32>) -> Self {
    match self {
      Self::Pattern {
        tile,
        tile_width,
        tile_height,
        x,
        y,
        area,
      } => Self::Pattern {
        tile,
        tile_width,
        tile_height,
        x: x.into_iter().map(|x| x + offset.x).collect(),
        y: y.into_iter().map(|y| y + offset.y).collect(),
        area: PaintRect {
          x: area.x + offset.x,
          y: area.y + offset.y,
          ..area
        },
      },
      paint => paint,
    }
  }

  /// Background or mask `layers`, each with its `background-blend-mode`.
  pub(super) fn layers(
    layers: &[BackgroundLayer<'_>],
    context: &RenderContext,
  ) -> Vec<(Self, Option<String>)> {
    layers
      .iter()
      .filter_map(|layer| {
        let tiling = layer.tiling;
        let tile = Self::tile(layer.image, tiling.tile.width, tiling.tile.height, context)?;
        let (x, y) = tiling.origins();
        let blend_mode =
          (layer.blend_mode != BlendMode::Normal).then(|| layer.blend_mode.to_css_string());

        Some((
          Self::Pattern {
            tile: Box::new(tile),
            tile_width: tiling.tile.width,
            tile_height: tiling.tile.height,
            x: x.into_vec(),
            y: y.into_vec(),
            area: tiling.dest.into(),
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
