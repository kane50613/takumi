use cssparser::Parser;
use image::{GenericImageView, Rgba};

use super::gradient_utils::{
  GradientOverlayTile, adaptive_lut_size, apply_dither, build_color_lut_with_interpolation,
  resolve_stops_along_axis,
};
use crate::{
  layout::style::{
    BackgroundPosition, ColorInterpolationMethod, CssToken, FromCss, GradientStop, GradientStops,
    Length, MakeComputed, ParseResult, declare_enum_from_css_impl,
  },
  rendering::{BufferPool, RenderContext, Sizing},
};

/// Represents a radial gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
  /// The radial gradient shape
  pub shape: RadialShape,
  /// The sizing mode for the gradient
  pub size: RadialSize,
  /// Center position
  pub center: BackgroundPosition,
  /// The color interpolation method used between stops.
  pub interpolation: ColorInterpolationMethod,
  /// Gradient stops
  pub stops: Box<[GradientStop]>,
}

impl MakeComputed for RadialGradient {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.center.make_computed(sizing);
    self.stops.make_computed(sizing);
  }
}

/// Supported shapes for radial gradients
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RadialShape {
  /// A circle shape where radii are equal
  Circle,
  /// An ellipse shape with independent x/y radii
  #[default]
  Ellipse,
}

declare_enum_from_css_impl!(
  RadialShape,
  "circle" => RadialShape::Circle,
  "ellipse" => RadialShape::Ellipse,
);

/// Supported size keywords for radial gradients
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RadialSize {
  /// The gradient end stops at the nearest side from the center
  ClosestSide,
  /// The gradient end stops at the farthest side from the center
  FarthestSide,
  /// The gradient end stops at the nearest corner from the center
  ClosestCorner,
  /// The gradient end stops at the farthest corner from the center
  #[default]
  FarthestCorner,
}

declare_enum_from_css_impl!(
  RadialSize,
  "closest-side" => RadialSize::ClosestSide,
  "farthest-side" => RadialSize::FarthestSide,
  "closest-corner" => RadialSize::ClosestCorner,
  "farthest-corner" => RadialSize::FarthestCorner,
);

/// Precomputed drawing context for repeated sampling of a `RadialGradient`.
#[derive(Debug, Clone)]
pub(crate) struct RadialGradientTile {
  /// Target width in pixels.
  pub width: u32,
  /// Target height in pixels.
  pub height: u32,
  /// Center X coordinate in pixels
  pub cx: f32,
  /// Center Y coordinate in pixels
  pub cy: f32,
  /// Radius X in pixels (for circle, equals radius_y)
  pub radius_x: f32,
  /// Radius Y in pixels (for circle, equals radius_x)
  pub radius_y: f32,
  /// Reciprocal of `radius_x` for sampling.
  pub inv_radius_x: f32,
  /// Reciprocal of `radius_y` for sampling.
  pub inv_radius_y: f32,
  /// Scale converting normalized distance to LUT index.
  pub distance_to_lut_scale: f32,
  /// Pre-computed color lookup table for fast gradient sampling.
  /// Maps normalized distance [0.0, 1.0] from center to color.
  pub color_lut: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RadialGradientRowState {
  dx: f32,
  dy2: f32,
  lut_len: usize,
  max_lut_index: usize,
}

impl GenericImageView for RadialGradientTile {
  type Pixel = Rgba<u8>;

  fn dimensions(&self) -> (u32, u32) {
    (self.width, self.height)
  }

  fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
    let lut_samples = self.lut_samples();
    if lut_samples.is_empty() {
      return Rgba([0, 0, 0, 0]);
    }

    if lut_samples.len() == 1 {
      return Rgba(apply_dither(&lut_samples[0], x, y));
    }

    let dx = (x as f32 - self.cx) / self.radius_x.max(1e-6);
    let dy = (y as f32 - self.cy) / self.radius_y.max(1e-6);
    let normalized_distance = (dx * dx + dy * dy).sqrt();
    let lut_idx =
      self.lut_index_for_normalized_distance_with_len(normalized_distance, lut_samples.len());

    Rgba(apply_dither(&lut_samples[lut_idx], x, y))
  }
}

impl RadialGradientTile {
  #[inline(always)]
  pub(crate) fn lut_samples(&self) -> &[[f32; 4]] {
    bytemuck::cast_slice(&self.color_lut)
  }

  #[inline(always)]
  pub(crate) fn lut_index_for_normalized_distance_with_len(
    &self,
    normalized_distance: f32,
    lut_len: usize,
  ) -> usize {
    if lut_len <= 1 {
      return 0;
    }

    ((normalized_distance.clamp(0.0, 1.0) * self.distance_to_lut_scale).round() as usize)
      .min(lut_len - 1)
  }

  /// Builds a drawing context from a gradient and a target viewport.
  pub fn new(
    gradient: &RadialGradient,
    width: u32,
    height: u32,
    context: &RenderContext,
    buffer_pool: &mut BufferPool,
  ) -> Self {
    let cx = Length::from(gradient.center.0.x).to_px(&context.sizing, width as f32);
    let cy = Length::from(gradient.center.0.y).to_px(&context.sizing, height as f32);

    // Distances to sides and corners
    let dx_left = cx;
    let dx_right = width as f32 - cx;
    let dy_top = cy;
    let dy_bottom = height as f32 - cy;

    let (radius_x, radius_y) = match (gradient.shape, gradient.size) {
      (RadialShape::Ellipse, RadialSize::FarthestCorner) => {
        // ellipse radii to farthest corner: take farthest side per axis
        (dx_left.max(dx_right), dy_top.max(dy_bottom))
      }
      (RadialShape::Circle, RadialSize::FarthestCorner) => {
        // distance to farthest corner
        let candidates = [
          (cx, cy),
          (cx, dy_bottom),
          (dx_right, cy),
          (dx_right, dy_bottom),
        ];
        let r = candidates
          .iter()
          .map(|(dx, dy)| (dx * dx + dy * dy).sqrt())
          .fold(0.0_f32, f32::max);
        (r, r)
      }
      // Fallbacks for other size keywords: approximate using sides
      (RadialShape::Ellipse, RadialSize::FarthestSide) => {
        (dx_left.max(dx_right), dy_top.max(dy_bottom))
      }
      (RadialShape::Ellipse, RadialSize::ClosestSide) => {
        (dx_left.min(dx_right), dy_top.min(dy_bottom))
      }
      (RadialShape::Circle, RadialSize::FarthestSide) => {
        let r = dx_left.max(dx_right).max(dy_top.max(dy_bottom));
        (r, r)
      }
      (RadialShape::Circle, RadialSize::ClosestSide) => {
        let r = dx_left.min(dx_right).min(dy_top.min(dy_bottom));
        (r, r)
      }
      (RadialShape::Ellipse, RadialSize::ClosestCorner) => {
        let f_rx = dx_left.max(dx_right);
        let f_ry = dy_top.max(dy_bottom);
        let corners = [
          (dx_left, dy_top),
          (dx_right, dy_top),
          (dx_left, dy_bottom),
          (dx_right, dy_bottom),
        ];
        let distances = corners.map(|(dx, dy)| (dx * dx + dy * dy).sqrt());
        let dist_to_closest_corner = distances.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let dist_to_farthest_corner = distances.iter().fold(0.0f32, |a, &b| a.max(b));
        let ratio = dist_to_closest_corner / dist_to_farthest_corner.max(1e-6);
        (f_rx * ratio, f_ry * ratio)
      }
      (RadialShape::Circle, RadialSize::ClosestCorner) => {
        let candidates = [
          (cx, cy),
          (cx, dy_bottom),
          (dx_right, cy),
          (dx_right, dy_bottom),
        ];
        let r = candidates
          .iter()
          .map(|(dx, dy)| (dx * dx + dy * dy).sqrt())
          .fold(f32::INFINITY, f32::min);
        (r, r)
      }
    };

    let radius_scale = radius_x.max(radius_y);
    let resolved_stops = resolve_stops_along_axis(&gradient.stops, radius_scale.max(1e-6), context);

    // Pre-compute color lookup table with adaptive size.
    let lut_size = adaptive_lut_size(radius_scale);
    let color_lut = build_color_lut_with_interpolation(
      &resolved_stops,
      radius_scale,
      lut_size,
      buffer_pool,
      gradient.interpolation.color_space,
      gradient.interpolation.hue_direction,
    );
    let lut_len = color_lut.len() / 16;
    let inv_radius_x = radius_x.max(1e-6).recip();
    let inv_radius_y = radius_y.max(1e-6).recip();
    let distance_to_lut_scale = if lut_len <= 1 {
      0.0
    } else {
      (lut_len - 1) as f32
    };

    RadialGradientTile {
      width,
      height,
      cx,
      cy,
      radius_x,
      radius_y,
      inv_radius_x,
      inv_radius_y,
      distance_to_lut_scale,
      color_lut,
    }
  }
}

impl GradientOverlayTile for RadialGradientTile {
  type RowState = RadialGradientRowState;

  #[inline(always)]
  fn width(&self) -> u32 {
    self.width
  }

  #[inline(always)]
  fn height(&self) -> u32 {
    self.height
  }

  #[inline(always)]
  fn lut_samples(&self) -> &[[f32; 4]] {
    self.lut_samples()
  }

  #[inline(always)]
  fn begin_row(&self, src_x_start: u32, src_y: u32, lut_len: usize) -> Self::RowState {
    let dy = (src_y as f32 - self.cy) * self.inv_radius_y;
    RadialGradientRowState {
      dx: (src_x_start as f32 - self.cx) * self.inv_radius_x,
      dy2: dy * dy,
      lut_len,
      max_lut_index: lut_len.saturating_sub(1),
    }
  }

  #[inline(always)]
  fn next_lut_index(&self, row_state: &mut Self::RowState) -> usize {
    if row_state.dy2 >= 1.0 {
      return row_state.max_lut_index;
    }

    let normalized_distance = (row_state.dx * row_state.dx + row_state.dy2).sqrt();
    let lut_idx =
      self.lut_index_for_normalized_distance_with_len(normalized_distance, row_state.lut_len);
    row_state.dx += self.inv_radius_x;
    lut_idx
  }
}

impl<'i> FromCss<'i> for RadialGradient {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, RadialGradient> {
    input.expect_function_matching("radial-gradient")?;

    input.parse_nested_block(|input| {
      let mut shape = RadialShape::Ellipse;
      let mut size = RadialSize::FarthestCorner;
      let mut center = BackgroundPosition::default();
      let mut interpolation = ColorInterpolationMethod::default();

      loop {
        if let Ok(s) = input.try_parse(RadialShape::from_css) {
          shape = s;
          continue;
        }

        if let Ok(s) = input.try_parse(RadialSize::from_css) {
          size = s;
          continue;
        }

        if input.try_parse(|i| i.expect_ident_matching("at")).is_ok() {
          center = BackgroundPosition::from_css(input)?;
          continue;
        }

        if let Ok(parsed_interpolation) = input.try_parse(ColorInterpolationMethod::from_css) {
          interpolation = parsed_interpolation;
          continue;
        }

        input.try_parse(Parser::expect_comma).ok();

        break;
      }

      let stops = GradientStops::from_css(input)?;

      Ok(RadialGradient {
        shape,
        size,
        center,
        interpolation,
        stops: stops.into_boxed_slice(),
      })
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("radial-gradient()")]
  }
}

#[cfg(test)]
mod tests {
  use color::{ColorSpaceTag, HueDirection};

  use super::*;
  use crate::layout::style::{
    Color, Length, PositionComponent, PositionKeywordX, PositionKeywordY, SpacePair, StopPosition,
  };
  use crate::{GlobalContext, rendering::RenderContext};

  #[test]
  fn test_parse_radial_gradient_basic() {
    let gradient = RadialGradient::from_str("radial-gradient(#ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(RadialGradient {
        shape: RadialShape::Ellipse,
        size: RadialSize::FarthestCorner,
        center: BackgroundPosition::default(),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color([255, 0, 0, 255]).into(),
            hint: None,
          },
          GradientStop::ColorHint {
            color: Color([0, 0, 255, 255]).into(),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_radial_gradient_with_interpolation_color_space() {
    assert_eq!(
      RadialGradient::from_str("radial-gradient(in oklab, red, blue)"),
      Ok(RadialGradient {
        shape: RadialShape::Ellipse,
        size: RadialSize::FarthestCorner,
        center: BackgroundPosition::default(),
        interpolation: ColorInterpolationMethod {
          color_space: ColorSpaceTag::Oklab,
          hue_direction: HueDirection::Shorter,
        },
        stops: [
          GradientStop::ColorHint {
            color: Color::from_rgb(0xff0000).into(),
            hint: None,
          },
          GradientStop::ColorHint {
            color: Color::from_rgb(0x0000ff).into(),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_radial_gradient_circle_farthest_side() {
    let gradient =
      RadialGradient::from_str("radial-gradient(circle farthest-side, #ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(RadialGradient {
        shape: RadialShape::Circle,
        size: RadialSize::FarthestSide,
        center: BackgroundPosition::default(),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color([255, 0, 0, 255]).into(),
            hint: None,
          },
          GradientStop::ColorHint {
            color: Color([0, 0, 255, 255]).into(),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_radial_gradient_ellipse_at_left_top() {
    let gradient =
      RadialGradient::from_str("radial-gradient(ellipse at left top, #ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(RadialGradient {
        shape: RadialShape::Ellipse,
        size: RadialSize::FarthestCorner,
        center: BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Left),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        )),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color([255, 0, 0, 255]).into(),
            hint: None,
          },
          GradientStop::ColorHint {
            color: Color([0, 0, 255, 255]).into(),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_radial_gradient_size_then_position() {
    let gradient =
      RadialGradient::from_str("radial-gradient(farthest-corner at 25% 70%, #ffffff, #000000)");

    assert_eq!(
      gradient,
      Ok(RadialGradient {
        shape: RadialShape::Ellipse,
        size: RadialSize::FarthestCorner,
        center: BackgroundPosition(SpacePair::from_pair(
          Length::Percentage(25.0).into(),
          Length::Percentage(70.0).into(),
        )),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color::white().into(),
            hint: None,
          },
          GradientStop::ColorHint {
            color: Color::black().into(),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_radial_gradient_circle_farthest_side_with_stops() {
    let gradient = RadialGradient::from_str(
      "radial-gradient(circle at 25px 25px, lightgray 2%, transparent 0%)",
    );

    assert_eq!(
      gradient,
      Ok(RadialGradient {
        shape: RadialShape::Circle,
        size: RadialSize::FarthestCorner,
        center: BackgroundPosition(SpacePair::from_single(PositionComponent::Length(
          Length::Px(25.0),
        ))),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color([211, 211, 211, 255]).into(),
            hint: Some(StopPosition(Length::Percentage(2.0))),
          },
          GradientStop::ColorHint {
            color: Color::transparent().into(),
            hint: Some(StopPosition(Length::Percentage(0.0))),
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_radial_gradient_with_stop_positions() {
    let gradient =
      RadialGradient::from_str("radial-gradient(circle, #ff0000 0%, #00ff00 50%, #0000ff 100%)");

    assert_eq!(
      gradient,
      Ok(RadialGradient {
        shape: RadialShape::Circle,
        size: RadialSize::FarthestCorner,
        center: BackgroundPosition::default(),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color([255, 0, 0, 255]).into(),
            hint: Some(StopPosition(Length::Percentage(0.0))),
          },
          GradientStop::ColorHint {
            color: Color([0, 255, 0, 255]).into(),
            hint: Some(StopPosition(Length::Percentage(50.0))),
          },
          GradientStop::ColorHint {
            color: Color([0, 0, 255, 255]).into(),
            hint: Some(StopPosition(Length::Percentage(100.0))),
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_radial_gradient_with_double_position_color_stop() {
    assert_eq!(
      RadialGradient::from_str("radial-gradient(circle, red 10% 20%, blue)"),
      Ok(RadialGradient {
        shape: RadialShape::Circle,
        size: RadialSize::FarthestCorner,
        center: BackgroundPosition::default(),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color::from_rgb(0xff0000).into(),
            hint: Some(StopPosition(Length::Percentage(10.0))),
          },
          GradientStop::ColorHint {
            color: Color::from_rgb(0xff0000).into(),
            hint: Some(StopPosition(Length::Percentage(20.0))),
          },
          GradientStop::ColorHint {
            color: Color::from_rgb(0x0000ff).into(),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn resolve_stops_percentage_and_px_radial() {
    let gradient = RadialGradient {
      shape: RadialShape::Ellipse,
      size: RadialSize::FarthestCorner,
      center: BackgroundPosition::default(),
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Percentage(50.0))),
        },
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Px(100.0))),
        },
      ]
      .into(),
    };

    let context = GlobalContext::default();
    let render_context = RenderContext::new_test(&context, (200, 100).into());
    let resolved = resolve_stops_along_axis(
      &gradient.stops,
      render_context.sizing.viewport.width.unwrap_or_default() as f32,
      &render_context,
    );

    assert_eq!(resolved.len(), 3);
    assert!((resolved[0].position - 0.0).abs() < 1e-3);
    assert_eq!(resolved[1].position, resolved[2].position);
  }

  #[test]
  fn resolve_stops_equal_positions_distributed_radial() {
    let gradient = RadialGradient {
      shape: RadialShape::Ellipse,
      size: RadialSize::FarthestCorner,
      center: BackgroundPosition::default(),
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Px(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Px(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Px(0.0))),
        },
      ]
      .into(),
    };

    let context = GlobalContext::default();
    let render_context = RenderContext::new_test(&context, (200, 100).into());
    let resolved = resolve_stops_along_axis(
      &gradient.stops,
      render_context.sizing.viewport.width.unwrap_or_default() as f32,
      &render_context,
    );

    assert_eq!(resolved.len(), 3);
    assert!(resolved[0].position >= 0.0);
    assert!(resolved[1].position >= resolved[0].position);
    assert!(resolved[2].position >= resolved[1].position);
  }

  #[test]
  fn test_radial_gradient_at() {
    let gradient = RadialGradient {
      shape: RadialShape::Circle,
      size: RadialSize::FarthestCorner,
      center: BackgroundPosition::default(), // default is center (50%, 50%)
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(), // Red at center
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(), // Blue at edge
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    let context = GlobalContext::default();
    let dummy_context = RenderContext::new_test(&context, (100, 100).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = RadialGradientTile::new(&gradient, 100, 100, &dummy_context, &mut buffer_pool);

    // Center (50, 50) should be red
    let color_center = tile.get_pixel(50, 50);
    assert_eq!(color_center, Rgba([255, 0, 0, 255]));

    // Far outside (200, 200) should be clamped to blue
    let color_far = tile.get_pixel(200, 200);
    assert_eq!(color_far, Rgba([0, 0, 255, 255]));
  }

  #[test]
  fn test_radial_gradient_ellipse_closest_corner() {
    let gradient = RadialGradient {
      shape: RadialShape::Ellipse,
      size: RadialSize::ClosestCorner,
      center: BackgroundPosition(SpacePair::from_pair(
        Length::Px(20.0).into(),
        Length::Px(20.0).into(),
      )),
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::white().into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    let context = GlobalContext::default();
    let dummy_context = RenderContext::new_test(&context, (100, 100).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = RadialGradientTile::new(&gradient, 100, 100, &dummy_context, &mut buffer_pool);

    // dx_left=20, dx_right=80, dy_top=20, dy_bottom=80
    // f_rx = 80, f_ry = 80
    // d_closest = sqrt(20^2 + 20^2)
    // d_farthest = sqrt(80^2 + 80^2)
    // ratio = d_closest / d_farthest = 20/80 = 0.25
    // radius_x = 80 * 0.25 = 20
    assert!((tile.radius_x - 20.0).abs() < 1e-3);
    assert!((tile.radius_y - 20.0).abs() < 1e-3);
  }
}
