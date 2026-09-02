use std::{f32::consts::TAU, fmt};

use cssparser::{Parser, Token, match_ignore_ascii_case};
use tiny_skia::PremultipliedColorU8;
use typed_builder::TypedBuilder;

use super::gradient_utils::{
  ColorLut, GradientOverlayTile, LutAxis, gradient_tile_accessors, parse_gradient_stops,
  write_gradient_css,
};
use crate::{
  math,
  style::{
    Angle, Color, ColorInterpolationMethod, CssDescriptorKind, CssToken, FromCss, GradientStop,
    Length, MakeComputed, ParseResult, PositionValue, ResolvedGradientStop, SizingContext,
    StopPosition, ToCss, unexpected_token,
  },
};

const LUT_INDEX_BOUNDARY_EPSILON: f32 = 0.001;

/// Represents a CSS conic-gradient.
#[derive(Debug, Clone, PartialEq, TypedBuilder)]
pub struct ConicGradient {
  /// Whether the gradient repeats beyond the last stop.
  #[builder(default)]
  pub repeating: bool,
  /// The starting angle of the gradient (default 0deg = from top).
  #[builder(default)]
  pub from_angle: Angle,
  /// Center position (default 50% 50%).
  #[builder(default = PositionValue::center())]
  pub center: PositionValue,
  /// The color interpolation method used between stops.
  #[builder(default = ColorInterpolationMethod::LEGACY)]
  pub interpolation: ColorInterpolationMethod,
  /// Gradient color stops.
  #[builder(setter(into))]
  pub stops: Box<[GradientStop]>,
}

impl MakeComputed for ConicGradient {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.center.make_computed(sizing);
    self.stops.make_computed(sizing);
  }
}

/// Precomputed data for repeated sampling of a `ConicGradient`.
#[derive(Debug, Clone)]
pub struct ConicGradientTile {
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
  /// Starting angle as a fraction of a full turn.
  start_turns: f32,
  /// Scale converting adjusted turns to a LUT index on the non-repeating path.
  turns_to_lut_scale: f32,
  /// Whether this gradient repeats.
  pub repeating: bool,
  /// First resolved stop position in degrees, used as repeating origin.
  pub repeat_start_deg: f32,
  /// Repeat period in degrees.
  pub repeat_period_deg: f32,
  /// Scale converting an adjusted angle in radians to LUT index.
  pub angle_to_lut_scale: f32,
  /// Whether every LUT entry has alpha = 255.
  pub fully_opaque: bool,
  /// Pre-computed colour samples around the turn.
  pub lut: ColorLut,
}

/// Per-row sampling state for incremental angle stepping.
#[derive(Debug, Clone, Copy)]
pub struct ConicGradientRowState {
  dx: f32,
  dy: f32,
  lut_len: usize,
}

impl ConicGradientTile {
  fn visible_angle_samples(width: u32, height: u32, cx: f32, cy: f32) -> usize {
    let max_dx = cx.max(width as f32 - cx);
    let max_dy = cy.max(height as f32 - cy);
    (max_dx.hypot(max_dy) * TAU).ceil() as usize + 1
  }

  /// Adjusted angle at `(dx, dy)` as a fraction of a full turn, measured from the gradient's start
  /// angle (CSS 0deg = from top, clockwise).
  #[inline(always)]
  fn adjusted_turns(&self, dx: f32, dy: f32) -> f32 {
    let turns = math::xy_to_unit_angle(-dy, dx);
    let adjusted = turns - self.start_turns;
    if adjusted < 0.0 {
      adjusted + 1.0
    } else {
      adjusted
    }
  }

  #[inline(always)]
  fn stable_floor_index(scaled_position: f32, max_index: usize) -> usize {
    let nearest = scaled_position.round();
    let sample = if (scaled_position - nearest).abs() <= LUT_INDEX_BOUNDARY_EPSILON {
      nearest
    } else {
      scaled_position.floor()
    };
    (sample as usize).min(max_index)
  }

  #[inline(always)]
  fn stable_round_index(scaled_position: f32, max_index: usize) -> usize {
    let floor = scaled_position.floor();
    let fraction = scaled_position - floor;
    let sample = if fraction >= 0.5 - LUT_INDEX_BOUNDARY_EPSILON {
      floor + 1.0
    } else {
      floor
    };
    (sample as usize).min(max_index)
  }

  #[inline(always)]
  fn lut_index_for_adjusted_turns(&self, adjusted_turns: f32, lut_len: usize) -> usize {
    if lut_len <= 1 {
      return 0;
    }

    let max_index = lut_len - 1;
    if self.repeating && self.repeat_period_deg > 1e-6 {
      let degrees = adjusted_turns * 360.0;
      let wrapped = (degrees - self.repeat_start_deg).rem_euclid(self.repeat_period_deg);
      Self::stable_round_index(wrapped * self.angle_to_lut_scale, max_index)
    } else {
      Self::stable_floor_index(adjusted_turns * self.turns_to_lut_scale, max_index)
    }
  }

  /// Maps an adjusted angle in radians to a LUT index for a LUT of `lut_len`.
  #[inline(always)]
  pub fn lut_index_for_adjusted_angle_with_len(
    &self,
    adjusted_angle: f32,
    lut_len: usize,
  ) -> usize {
    if lut_len <= 1 {
      return 0;
    }

    let max_index = lut_len - 1;
    if self.repeating && self.repeat_period_deg > 1e-6 {
      let degrees = adjusted_angle / TAU * 360.0;
      let wrapped = (degrees - self.repeat_start_deg).rem_euclid(self.repeat_period_deg);
      Self::stable_round_index(wrapped * self.angle_to_lut_scale, max_index)
    } else {
      Self::stable_floor_index(adjusted_angle * self.angle_to_lut_scale, max_index)
    }
  }

  /// Builds a drawing context from a conic gradient and a target viewport.
  pub fn new(
    gradient: &ConicGradient,
    width: u32,
    height: u32,
    sizing: &SizingContext,
    current_color: Color,
    dither: bool,
  ) -> Self {
    let cx = Length::from(gradient.center.0.x).to_px(sizing, width as f32);
    let cy = Length::from(gradient.center.0.y).to_px(sizing, height as f32);

    let start_rad = gradient.from_angle.to_radians().rem_euclid(TAU);
    let start_turns = start_rad / TAU;

    let resolved_stops =
      ResolvedGradientStop::resolve(&gradient.stops, 360.0, sizing, current_color);
    let axis = LutAxis::new(gradient.repeating, resolved_stops, 360.0);
    let lut_size = axis.lut_size_covering(Self::visible_angle_samples(width, height, cx, cy));
    let lut = axis.lut(lut_size, gradient.interpolation, dither);
    let lut_len = lut.len();
    let angle_to_lut_scale = if axis.repeating && axis.repeat_period > 1e-6 && lut_len > 1 {
      (lut_len - 1) as f32 / axis.repeat_period
    } else if lut_len == 0 {
      0.0
    } else {
      lut_len as f32 / TAU
    };

    let fully_opaque = lut.colors().iter().all(|p| p.alpha() == u8::MAX);

    ConicGradientTile {
      width,
      height,
      cx,
      cy,
      start_rad,
      start_turns,
      turns_to_lut_scale: lut_len as f32,
      repeating: axis.repeating,
      repeat_start_deg: axis.repeat_start,
      repeat_period_deg: axis.repeat_period,
      angle_to_lut_scale,
      fully_opaque,
      lut,
    }
  }

  #[inline(always)]
  fn pixel_lut_index(&self, x: u32, y: u32) -> usize {
    let dx = x as f32 - self.cx;
    let dy = y as f32 - self.cy;
    if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
      return 0;
    }

    let adjusted = self.adjusted_turns(dx, dy);
    self.lut_index_for_adjusted_turns(adjusted, self.lut.len())
  }
}

impl GradientOverlayTile for ConicGradientTile {
  type RowState = ConicGradientRowState;

  gradient_tile_accessors!();

  #[inline(always)]
  fn sample_pixel(&self, x: u32, y: u32) -> PremultipliedColorU8 {
    match self.lut.len() {
      0 => PremultipliedColorU8::TRANSPARENT,
      1 => self.lut.sample(0),
      _ => self.lut.sample(self.pixel_lut_index(x, y)),
    }
  }

  #[inline(always)]
  fn sample_pixel_dithered(&self, x: u32, y: u32) -> PremultipliedColorU8 {
    if !self.lut.dither_active() {
      return self.sample_pixel(x, y);
    }

    self.lut.sample_dithered(self.pixel_lut_index(x, y), x, y)
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
      let adjusted_turns = self.adjusted_turns(row_state.dx, row_state.dy);
      self.lut_index_for_adjusted_turns(adjusted_turns, row_state.lut_len)
    };
    row_state.dx += 1.0;
    lut_idx
  }
}

impl ConicGradient {
  /// Parses a conic gradient stop position: a percentage/number (fraction of a turn) or an angle.
  fn parse_stop_position<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, StopPosition> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Percentage { unit_value, .. } => {
        Ok(StopPosition(Length::Percentage(*unit_value * 100.0)))
      }
      Token::Number { value, .. } if (0.0..=1.0).contains(value) => {
        Ok(StopPosition(Length::Percentage(*value * 100.0)))
      }
      Token::Dimension { value, unit, .. } => {
        let degrees = match_ignore_ascii_case! { unit,
          "deg" => *value,
          "grad" => *value * 0.9,
          "rad" => value.to_degrees(),
          "turn" => *value * 360.0,
          _ => return Err(unexpected_token!(StopPosition, location, token)),
        };

        Ok(StopPosition(Length::Percentage(degrees / 360.0 * 100.0)))
      }
      _ => Err(unexpected_token!(StopPosition, location, token)),
    }
  }
}

impl<'i> FromCss<'i> for ConicGradient {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, ConicGradient> {
    let location = input.current_source_location();
    let name = input.expect_function()?.to_owned();
    let repeating = match_ignore_ascii_case! { &name,
      "conic-gradient" => false,
      "repeating-conic-gradient" => true,
      _ => return Err(unexpected_token!(location, &Token::Function(name))),
    };

    input.parse_nested_block(|input| {
      let mut from_angle: Option<Angle> = None;
      let mut center: Option<PositionValue> = None;
      let mut interpolation = None;

      // Parse optional "from <angle>" and/or "at <position>" before the comma
      loop {
        // Try "from <angle>"
        if input.try_parse(|i| i.expect_ident_matching("from")).is_ok() {
          from_angle = Some(Angle::from_css(input)?);
          continue;
        }

        // Try "at <position>"
        if input.try_parse(|i| i.expect_ident_matching("at")).is_ok() {
          center = Some(PositionValue::from_css(input)?);
          continue;
        }

        if let Ok(parsed_interpolation) = input.try_parse(ColorInterpolationMethod::from_css) {
          interpolation = Some(parsed_interpolation);
          continue;
        }

        // Consume the comma separator if present
        input.try_parse(Parser::expect_comma).ok();
        break;
      }

      let (stops, modern) = parse_gradient_stops(input, ConicGradient::parse_stop_position)?;

      Ok(ConicGradient {
        repeating,
        from_angle: from_angle.unwrap_or(Angle::zero()),
        center: center.unwrap_or_else(PositionValue::center),
        interpolation: interpolation.unwrap_or(ColorInterpolationMethod::gradient_default(modern)),
        stops: stops.into_boxed_slice(),
      })
    })
  }

  const VALID_TOKENS: &'static [CssToken] =
    &[CssToken::Descriptor(CssDescriptorKind::ConicGradientFn)];
}

impl ToCss for ConicGradient {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let name = if self.repeating {
      "repeating-conic-gradient"
    } else {
      "conic-gradient"
    };

    let mut params_buf = String::new();
    if self.from_angle != Angle::zero() {
      params_buf.push_str("from ");
      self.from_angle.to_css(&mut params_buf)?;
    }
    let mut center_buf = String::new();
    self.center.to_css(&mut center_buf)?;
    let is_center_default = center_buf == "center center" || center_buf == "50% 50%";
    if !is_center_default {
      if !params_buf.is_empty() {
        params_buf.push(' ');
      }
      params_buf.push_str("at ");
      params_buf.push_str(&center_buf);
    }

    write_gradient_css(dest, name, &params_buf, &self.interpolation, &self.stops)
  }
}

#[cfg(test)]
mod tests {
  use color::{ColorSpaceTag, HueDirection};
  use tiny_skia::ColorU8;

  use super::*;
  use crate::{
    style::{
      Color, FromCssStr, Length, SpacePair, StopPosition,
      properties::gradient_utils::red_blue_stops,
    },
    viewport::Viewport,
  };
  #[test]
  fn test_parse_conic_gradient_basic_variants() {
    fn stop(color: Color, pct: Option<f32>) -> GradientStop {
      GradientStop::ColorHint {
        color: color.into(),
        hint: pct.map(|p| StopPosition(Length::Percentage(p))),
      }
    }
    let red = Color::from_rgb(0xff0000);
    let green = Color::from_rgb(0x00ff00);
    let blue = Color::from_rgb(0x0000ff);

    for (input, stops) in [
      (
        "conic-gradient(#ff0000, #0000ff)",
        red_blue_stops(None, None).to_vec(),
      ),
      (
        "conic-gradient(#ff0000 0%, #00ff00 50%, #0000ff 100%)",
        vec![
          stop(red, Some(0.0)),
          stop(green, Some(50.0)),
          stop(blue, Some(100.0)),
        ],
      ),
      (
        "conic-gradient(red 0deg, lime 180deg, blue 1turn)",
        vec![
          stop(red, Some(0.0)),
          stop(green, Some(50.0)),
          stop(blue, Some(100.0)),
        ],
      ),
      (
        "conic-gradient(red 10% 20%, blue)",
        vec![
          stop(red, Some(10.0)),
          stop(red, Some(20.0)),
          stop(blue, None),
        ],
      ),
      (
        "conic-gradient(red 0deg 90deg, blue)",
        vec![
          stop(red, Some(0.0)),
          stop(red, Some(25.0)),
          stop(blue, None),
        ],
      ),
    ] {
      assert_eq!(
        ConicGradient::from_css_str(input),
        Ok(ConicGradient {
          repeating: false,
          from_angle: Angle::zero(),
          center: PositionValue::center(),
          interpolation: ColorInterpolationMethod::LEGACY,
          stops: stops.into(),
        }),
        "input: {input}",
      );
    }
  }

  #[test]
  fn test_parse_conic_gradient_with_interpolation_color_space() {
    assert_eq!(
      ConicGradient::from_css_str("conic-gradient(in oklab, red, blue)"),
      Ok(ConicGradient {
        repeating: false,
        from_angle: Angle::zero(),
        center: PositionValue::center(),
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
  fn test_parse_conic_gradient_complex() {
    let gradient = ConicGradient::from_css_str("conic-gradient(from 90deg at 25% 75%, red, blue)");

    assert_eq!(
      gradient,
      Ok(ConicGradient {
        repeating: false,
        from_angle: Angle::new(90.0),
        center: PositionValue(SpacePair::from_pair(
          Length::Percentage(25.0).into(),
          Length::Percentage(75.0).into()
        )),
        interpolation: ColorInterpolationMethod::LEGACY,
        stops: red_blue_stops(None, None).into(),
      })
    );
  }

  #[test]
  fn test_conic_gradient_top_pixel_is_first_color() {
    let gradient = ConicGradient::builder()
      .stops(red_blue_stops(
        Some(StopPosition(Length::Percentage(0.0))),
        Some(StopPosition(Length::Percentage(100.0))),
      ))
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let tile = ConicGradientTile::new(&gradient, 100, 100, &sizing, Color::black(), false);

    // Top center (50, 0) should be red (start of gradient)
    let color_top = tile.sample_pixel(50, 0).demultiply();
    assert_eq!(color_top, ColorU8::from_rgba(255, 0, 0, 255));
  }

  #[test]
  fn test_conic_gradient_hard_stops() {
    // Simulate the card cost gradient: 3 colors with hard stops
    let gradient = ConicGradient::builder()
      .stops([
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
      ])
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let tile = ConicGradientTile::new(&gradient, 100, 100, &sizing, Color::black(), false);

    // Top-center should be red
    let top = tile.sample_pixel(50, 0).demultiply();
    assert_eq!(top, ColorU8::from_rgba(255, 0, 0, 255));

    // Bottom should be green (roughly 180deg = 50% of turn, within the 33%–66% green zone)
    let bottom = tile.sample_pixel(50, 99).demultiply();
    assert_eq!(bottom, ColorU8::from_rgba(0, 255, 0, 255));
  }

  #[test]
  fn test_repeating_conic_gradient_quadrants() {
    let gradient = ConicGradient::builder()
      .repeating(true)
      .stops([
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(25.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(25.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(50.0))),
        },
      ])
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((40, 40)))
      .build();
    let tile = ConicGradientTile::new(&gradient, 40, 40, &sizing, Color::black(), false);

    assert_eq!(
      [
        tile.sample_pixel(25, 15).demultiply(),
        tile.sample_pixel(25, 25).demultiply(),
        tile.sample_pixel(15, 25).demultiply(),
        tile.sample_pixel(15, 15).demultiply(),
      ],
      [
        ColorU8::from_rgba(255, 0, 0, 255),
        ColorU8::from_rgba(0, 0, 255, 255),
        ColorU8::from_rgba(255, 0, 0, 255),
        ColorU8::from_rgba(0, 0, 255, 255),
      ]
    );
  }

  #[test]
  fn test_conic_gradient_lut_index_snaps_near_floor_boundary() {
    assert_eq!(ConicGradientTile::stable_floor_index(127.9995, 360), 128);
    assert_eq!(ConicGradientTile::stable_floor_index(127.998, 360), 127);
  }

  #[test]
  fn test_conic_gradient_lut_index_snaps_near_round_boundary() {
    assert_eq!(ConicGradientTile::stable_round_index(127.4995, 360), 128);
    assert_eq!(ConicGradientTile::stable_round_index(127.498, 360), 127);
  }
}
