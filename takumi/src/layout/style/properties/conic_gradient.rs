use std::f32::consts::TAU;

use cssparser::Parser;
use image::{GenericImageView, Rgba};

use super::gradient_utils::{
  GradientOverlayTile, adaptive_lut_size, apply_dither, build_color_lut_with_interpolation,
  resolve_stops_along_axis,
};
use crate::{
  layout::style::{
    Angle, BackgroundPosition, ColorInterpolationMethod, CssToken, FromCss, GradientStop,
    GradientStops, Length, MakeComputed, ObjectPosition, ParseResult,
  },
  rendering::{BufferPool, RenderContext, Sizing},
};

/// Represents a CSS conic-gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct ConicGradient {
  /// The starting angle of the gradient (default 0deg = from top).
  pub from_angle: Angle,
  /// Center position (default 50% 50%).
  pub center: ObjectPosition,
  /// The color interpolation method used between stops.
  pub interpolation: ColorInterpolationMethod,
  /// Gradient color stops.
  pub stops: Box<[GradientStop]>,
}

impl MakeComputed for ConicGradient {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.center.make_computed(sizing);
    self.stops.make_computed(sizing);
  }
}

/// Precomputed data for repeated sampling of a `ConicGradient`.
#[derive(Debug, Clone)]
pub(crate) struct ConicGradientTile {
  /// Target width in pixels.
  pub width: u32,
  /// Target height in pixels.
  pub height: u32,
  /// Center X coordinate in pixels.
  pub cx: f32,
  /// Center Y coordinate in pixels.
  pub cy: f32,
  /// Starting angle in radians (CSS 0deg = from top, clockwise).
  pub start_rad: f32,
  /// Scale converting an adjusted angle in radians to LUT index.
  pub angle_to_lut_scale: f32,
  /// Pre-computed color lookup table for fast gradient sampling.
  /// Maps normalized angle [0.0, 1.0] (fraction of full turn) to color.
  pub color_lut: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ConicGradientRowState {
  dx: f32,
  dy: f32,
  lut_len: usize,
}

impl GenericImageView for ConicGradientTile {
  type Pixel = Rgba<u8>;

  fn dimensions(&self) -> (u32, u32) {
    (self.width, self.height)
  }

  fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
    let lut_samples: &[[f32; 4]] = bytemuck::cast_slice(&self.color_lut);
    if lut_samples.is_empty() {
      return Rgba([0, 0, 0, 0]);
    }

    if lut_samples.len() == 1 {
      return Rgba(apply_dither(&lut_samples[0], x, y));
    }

    let dx = x as f32 - self.cx;
    let dy = y as f32 - self.cy;
    if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
      return Rgba(apply_dither(&lut_samples[0], x, y));
    }

    let angle_from_top = libm::atan2f(dx, -dy);
    let adjusted = (angle_from_top - self.start_rad).rem_euclid(TAU);
    let lut_idx = self.lut_index_for_adjusted_angle_with_len(adjusted, lut_samples.len());

    Rgba(apply_dither(&lut_samples[lut_idx], x, y))
  }
}

impl ConicGradientTile {
  #[inline(always)]
  pub(crate) fn lut_index_for_adjusted_angle_with_len(
    &self,
    adjusted_angle: f32,
    lut_len: usize,
  ) -> usize {
    if lut_len <= 1 {
      return 0;
    }

    ((adjusted_angle * self.angle_to_lut_scale).floor() as usize).min(lut_len - 1)
  }

  /// Builds a drawing context from a conic gradient and a target viewport.
  pub fn new(
    gradient: &ConicGradient,
    width: u32,
    height: u32,
    context: &RenderContext,
    buffer_pool: &mut BufferPool,
  ) -> Self {
    let cx = Length::from(gradient.center.0.x).to_px(&context.sizing, width as f32);
    let cy = Length::from(gradient.center.0.y).to_px(&context.sizing, height as f32);

    let start_rad = gradient.from_angle.to_radians();

    // Resolve stop percentages against one full turn (360deg).
    let resolved_stops = resolve_stops_along_axis(&gradient.stops, 360.0, context);

    // Keep LUT sizing deterministic across platforms by deriving from integer tile dimensions.
    // 8 samples per pixel of the larger dimension provides enough angular density for conic edges.
    let angular_axis = width.max(height).max(1) as f32 * 8.0;
    let lut_size = adaptive_lut_size(angular_axis);
    let color_lut = build_color_lut_with_interpolation(
      &resolved_stops,
      360.0,
      lut_size,
      buffer_pool,
      gradient.interpolation.color_space,
      gradient.interpolation.hue_direction,
    );
    let lut_len = color_lut.len() / 16;
    let angle_to_lut_scale = if lut_len == 0 {
      0.0
    } else {
      lut_len as f32 / TAU
    };

    ConicGradientTile {
      width,
      height,
      cx,
      cy,
      start_rad,
      angle_to_lut_scale,
      color_lut,
    }
  }
}

impl GradientOverlayTile for ConicGradientTile {
  type RowState = ConicGradientRowState;

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
    bytemuck::cast_slice(&self.color_lut)
  }

  #[inline(always)]
  fn begin_row(&self, src_x_start: u32, src_y: u32, lut_len: usize) -> Self::RowState {
    ConicGradientRowState {
      dx: src_x_start as f32 - self.cx,
      dy: src_y as f32 - self.cy,
      lut_len,
    }
  }

  #[inline(always)]
  fn next_lut_index(&self, row_state: &mut Self::RowState) -> usize {
    let lut_idx = if row_state.dx.abs() <= f32::EPSILON && row_state.dy.abs() <= f32::EPSILON {
      0
    } else {
      let angle_from_top = libm::atan2f(row_state.dx, -row_state.dy);
      let adjusted_angle = (angle_from_top - self.start_rad).rem_euclid(TAU);
      self.lut_index_for_adjusted_angle_with_len(adjusted_angle, row_state.lut_len)
    };
    row_state.dx += 1.0;
    lut_idx
  }
}

impl<'i> FromCss<'i> for ConicGradient {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, ConicGradient> {
    input.expect_function_matching("conic-gradient")?;

    input.parse_nested_block(|input| {
      let mut from_angle: Option<Angle> = None;
      let mut center: Option<ObjectPosition> = None;
      let mut interpolation = ColorInterpolationMethod::default();

      // Parse optional "from <angle>" and/or "at <position>" before the comma
      loop {
        // Try "from <angle>"
        if input.try_parse(|i| i.expect_ident_matching("from")).is_ok() {
          from_angle = Some(Angle::from_css(input)?);
          continue;
        }

        // Try "at <position>"
        if input.try_parse(|i| i.expect_ident_matching("at")).is_ok() {
          center = Some(BackgroundPosition::from_css(input)?);
          continue;
        }

        if let Ok(parsed_interpolation) = input.try_parse(ColorInterpolationMethod::from_css) {
          interpolation = parsed_interpolation;
          continue;
        }

        // Consume the comma separator if present
        input.try_parse(Parser::expect_comma).ok();
        break;
      }

      let stops = GradientStops::from_css(input)?;

      Ok(ConicGradient {
        from_angle: from_angle.unwrap_or(Angle::zero()),
        center: center.unwrap_or_default(),
        interpolation,
        stops: stops.into_boxed_slice(),
      })
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("conic-gradient()")]
  }
}

#[cfg(test)]
mod tests {
  use color::{ColorSpaceTag, HueDirection};

  use super::*;
  use crate::layout::style::{Color, Length, SpacePair, StopPosition};
  use crate::{GlobalContext, rendering::RenderContext};

  #[test]
  fn test_parse_conic_gradient_basic() {
    let gradient = ConicGradient::from_str("conic-gradient(#ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(ConicGradient {
        from_angle: Angle::zero(),
        center: ObjectPosition::default(),
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
  fn test_parse_conic_gradient_with_interpolation_color_space() {
    assert_eq!(
      ConicGradient::from_str("conic-gradient(in oklab, red, blue)"),
      Ok(ConicGradient {
        from_angle: Angle::zero(),
        center: ObjectPosition::default(),
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
  fn test_parse_conic_gradient_with_stops() {
    assert_eq!(
      ConicGradient::from_str("conic-gradient(#ff0000 0%, #00ff00 50%, #0000ff 100%)"),
      Ok(ConicGradient {
        from_angle: Angle::zero(),
        center: ObjectPosition::default(),
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
  fn test_parse_conic_gradient_with_double_position_color_stop() {
    assert_eq!(
      ConicGradient::from_str("conic-gradient(red 10% 20%, blue)"),
      Ok(ConicGradient {
        from_angle: Angle::zero(),
        center: ObjectPosition::default(),
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
  fn test_parse_conic_gradient_complex() {
    let gradient = ConicGradient::from_str("conic-gradient(from 90deg at 25% 75%, red, blue)");

    assert_eq!(
      gradient,
      Ok(ConicGradient {
        from_angle: Angle::new(90.0),
        center: BackgroundPosition::<false>(SpacePair::from_pair(
          Length::Percentage(25.0).into(),
          Length::Percentage(75.0).into()
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
  fn test_conic_gradient_top_pixel_is_first_color() {
    let gradient = ConicGradient {
      from_angle: Angle::zero(),
      center: ObjectPosition::default(),
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    let context = GlobalContext::default();
    let render_context = RenderContext::new_test(&context, (100, 100).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = ConicGradientTile::new(&gradient, 100, 100, &render_context, &mut buffer_pool);

    // Top center (50, 0) should be red (start of gradient)
    let color_top = tile.get_pixel(50, 0);
    assert_eq!(color_top, Rgba([255, 0, 0, 255]));
  }

  #[test]
  fn test_conic_gradient_hard_stops() {
    // Simulate the card cost gradient: 3 colors with hard stops
    let gradient = ConicGradient {
      from_angle: Angle::zero(),
      center: ObjectPosition::default(),
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(33.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 255, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(33.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 255, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(66.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(66.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    let context = GlobalContext::default();
    let render_context = RenderContext::new_test(&context, (100, 100).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = ConicGradientTile::new(&gradient, 100, 100, &render_context, &mut buffer_pool);

    // Top-center should be red
    let top = tile.get_pixel(50, 0);
    assert_eq!(top, Rgba([255, 0, 0, 255]));

    // Bottom should be green (roughly 180deg = 50% of turn, within the 33%–66% green zone)
    let bottom = tile.get_pixel(50, 99);
    assert_eq!(bottom, Rgba([0, 255, 0, 255]));
  }
}
