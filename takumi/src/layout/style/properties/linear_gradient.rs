use cssparser::{Parser, Token};
use image::{GenericImageView, Rgba};
use std::ops::{Deref, Neg};

use super::gradient_utils::{
  GradientOverlayTile, adaptive_lut_size, apply_dither, build_color_lut_with_interpolation,
  resolve_stops_along_axis,
};
use crate::layout::style::{
  Color, ColorInterpolationMethod, CssToken, FromCss, Length, MakeComputed, ParseResult,
  declare_enum_from_css_impl, properties::ColorInput, tw::TailwindPropertyParser,
};
use crate::rendering::{BufferPool, RenderContext, Sizing};

/// Represents a linear gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
  /// The angle of the gradient.
  pub angle: Angle,
  /// The color interpolation method used between stops.
  pub interpolation: ColorInterpolationMethod,
  /// The steps of the gradient.
  pub stops: Box<[GradientStop]>,
}

impl MakeComputed for LinearGradient {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.stops.make_computed(sizing);
  }
}

impl GenericImageView for LinearGradientTile {
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

    let projection = self.projection_at(x as f32, y as f32);
    let lut_idx = self.lut_index_for_projection_with_len(projection, lut_samples.len());

    Rgba(apply_dither(&lut_samples[lut_idx], x, y))
  }
}

/// Precomputed drawing context for repeated sampling of a `LinearGradient`.
#[derive(Debug, Clone)]
pub(crate) struct LinearGradientTile {
  /// Target width in pixels.
  pub width: u32,
  /// Target height in pixels.
  pub height: u32,
  /// Direction vector X component derived from angle.
  pub dir_x: f32,
  /// Direction vector Y component derived from angle.
  pub dir_y: f32,
  /// Full axis length along gradient direction in pixels.
  pub axis_length: f32,
  /// Projection bias for `x * dir_x + y * dir_y + projection_bias`.
  pub projection_bias: f32,
  /// Scale converting axis-space position in pixels into LUT index space.
  pub position_to_lut_scale: f32,
  /// Pre-computed color lookup table for fast gradient sampling.
  /// Maps normalized position [0.0, 1.0] to color.
  pub color_lut: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LinearGradientRowState {
  projection: f32,
  lut_len: usize,
}

impl LinearGradientTile {
  #[inline(always)]
  pub(crate) fn lut_samples(&self) -> &[[f32; 4]] {
    bytemuck::cast_slice(&self.color_lut)
  }

  #[inline(always)]
  pub(crate) fn projection_at(&self, x: f32, y: f32) -> f32 {
    x * self.dir_x + y * self.dir_y + self.projection_bias
  }

  #[inline(always)]
  pub(crate) fn lut_index_for_projection_with_len(&self, projection: f32, lut_len: usize) -> usize {
    if lut_len <= 1 {
      return 0;
    }

    let position_px = projection.clamp(0.0, self.axis_length);
    ((position_px * self.position_to_lut_scale).round() as usize).min(lut_len - 1)
  }

  /// Builds a drawing context from a gradient and a target viewport.
  pub fn new(
    gradient: &LinearGradient,
    width: u32,
    height: u32,
    context: &RenderContext,
    buffer_pool: &mut BufferPool,
  ) -> Self {
    let rad = gradient.angle.0.to_radians();
    let (dir_x, dir_y) = (rad.sin(), -rad.cos());

    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let max_extent = ((width as f32 * dir_x.abs()) + (height as f32 * dir_y.abs())) / 2.0;
    let axis_length = 2.0 * max_extent;
    let projection_bias = max_extent - cx * dir_x - cy * dir_y;

    let resolved_stops = resolve_stops_along_axis(&gradient.stops, axis_length.max(1e-6), context);

    // Pre-compute color lookup table with adaptive size.
    let lut_size = adaptive_lut_size(axis_length);
    let color_lut = build_color_lut_with_interpolation(
      &resolved_stops,
      axis_length,
      lut_size,
      buffer_pool,
      gradient.interpolation.color_space,
      gradient.interpolation.hue_direction,
    );
    let lut_len = color_lut.len() / 16;
    let position_to_lut_scale = if axis_length.abs() <= f32::EPSILON || lut_len <= 1 {
      0.0
    } else {
      (lut_len - 1) as f32 / axis_length
    };

    LinearGradientTile {
      width,
      height,
      dir_x,
      dir_y,
      axis_length,
      projection_bias,
      position_to_lut_scale,
      color_lut,
    }
  }
}

impl GradientOverlayTile for LinearGradientTile {
  type RowState = LinearGradientRowState;

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
    LinearGradientRowState {
      projection: self.projection_at(src_x_start as f32, src_y as f32),
      lut_len,
    }
  }

  #[inline(always)]
  fn next_lut_index(&self, row_state: &mut Self::RowState) -> usize {
    let lut_idx = self.lut_index_for_projection_with_len(row_state.projection, row_state.lut_len);
    row_state.projection += self.dir_x;
    lut_idx
  }
}

/// Represents a gradient stop position.
/// If a percentage or number (0.0-1.0) is provided, it is treated as a percentage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopPosition(pub Length);

impl MakeComputed for StopPosition {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.0.make_computed(sizing);
  }
}

/// Represents a gradient stop.
#[derive(Debug, Clone, PartialEq)]
pub enum GradientStop {
  /// A color gradient stop.
  ColorHint {
    /// The color of the gradient stop.
    color: ColorInput,
    /// The position of the gradient stop.
    hint: Option<StopPosition>,
  },
  /// A numeric gradient stop.
  Hint(StopPosition),
}

impl MakeComputed for GradientStop {
  fn make_computed(&mut self, sizing: &Sizing) {
    match self {
      GradientStop::ColorHint { hint, .. } => hint.make_computed(sizing),
      GradientStop::Hint(hint) => hint.make_computed(sizing),
    }
  }
}

/// A list of gradient color stops, handling CSS double-stop syntax.
pub type GradientStops = Vec<GradientStop>;

impl<'i> FromCss<'i> for GradientStops {
  fn valid_tokens() -> &'static [CssToken] {
    GradientStop::valid_tokens()
  }

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut stops = Vec::new();
    loop {
      if let Ok(hint) = input.try_parse(StopPosition::from_css) {
        stops.push(GradientStop::Hint(hint));
      } else {
        let color = ColorInput::from_css(input)?;
        let first_position = input.try_parse(StopPosition::from_css).ok();
        let second_position = if first_position.is_some() {
          input.try_parse(StopPosition::from_css).ok()
        } else {
          None
        };

        match (first_position, second_position) {
          (Some(first_position), Some(second_position)) => {
            stops.push(GradientStop::ColorHint {
              color,
              hint: Some(first_position),
            });
            stops.push(GradientStop::ColorHint {
              color,
              hint: Some(second_position),
            });
          }
          (first_position, None) | (first_position, Some(_)) => {
            stops.push(GradientStop::ColorHint {
              color,
              hint: first_position,
            });
          }
        }
      }

      if input.try_parse(Parser::expect_comma).is_err() {
        break;
      }
    }

    Ok(stops)
  }
}

/// Represents a resolved gradient stop with a position.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedGradientStop {
  /// The color of the gradient stop.
  pub color: Color,
  /// The position of the gradient stop in pixels from the start of the axis.
  pub position: f32,
}

impl<'i> FromCss<'i> for StopPosition {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, StopPosition> {
    if let Ok(num) = input.try_parse(Parser::expect_number) {
      return Ok(StopPosition(Length::Percentage(
        num.clamp(0.0, 1.0) * 100.0,
      )));
    }

    if let Ok(unit_value) = input.try_parse(Parser::expect_percentage) {
      return Ok(StopPosition(Length::Percentage(unit_value * 100.0)));
    }

    let Ok(length) = input.try_parse(Length::from_css) else {
      return Err(Self::unexpected_token_error(
        input.current_source_location(),
        input.next()?,
      ));
    };

    Ok(StopPosition(length))
  }

  fn valid_tokens() -> &'static [CssToken] {
    Length::<true>::valid_tokens()
  }
}

impl<'i> FromCss<'i> for GradientStop {
  /// Parses a gradient hint from the input.
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, GradientStop> {
    if let Ok(hint) = input.try_parse(StopPosition::from_css) {
      return Ok(GradientStop::Hint(hint));
    };

    let color = ColorInput::from_css(input)?;
    let hint = input.try_parse(StopPosition::from_css).ok();

    Ok(GradientStop::ColorHint { color, hint })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("color"), CssToken::Token("length")]
  }
}

/// Represents an angle value in degrees.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Angle(f32);

impl MakeComputed for Angle {}

impl From<Angle> for zeno::Angle {
  fn from(angle: Angle) -> Self {
    zeno::Angle::from_degrees(angle.0)
  }
}

impl TailwindPropertyParser for Angle {
  fn parse_tw(token: &str) -> Option<Self> {
    match token.to_ascii_lowercase().as_str() {
      "none" => return Some(Angle::zero()),
      "to-t" => return Some(Angle::new(0.0)),
      "to-tr" => return Some(Angle::new(45.0)),
      "to-r" => return Some(Angle::new(90.0)),
      "to-br" => return Some(Angle::new(135.0)),
      "to-b" => return Some(Angle::new(180.0)),
      "to-bl" => return Some(Angle::new(225.0)),
      "to-l" => return Some(Angle::new(270.0)),
      "to-tl" => return Some(Angle::new(315.0)),
      _ => {}
    }

    let angle = token.parse::<f32>().ok()?;

    Some(Angle::new(angle))
  }
}

impl Neg for Angle {
  type Output = Self;

  fn neg(self) -> Self::Output {
    Angle::new(-self.0)
  }
}

impl Deref for Angle {
  type Target = f32;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl Angle {
  /// Returns a zero angle.
  pub const fn zero() -> Self {
    Angle(0.0)
  }

  /// Creates a new angle value, normalizing it to the range [0, 360).
  pub fn new(value: f32) -> Self {
    Angle(value.rem_euclid(360.0))
  }
}

/// Represents a horizontal keyword.
pub enum HorizontalKeyword {
  /// The left keyword.
  Left,
  /// The right keyword.
  Right,
}

/// Represents a vertical keyword.
pub enum VerticalKeyword {
  /// The top keyword.
  Top,
  /// The bottom keyword.
  Bottom,
}

declare_enum_from_css_impl!(
  HorizontalKeyword,
  "left" => HorizontalKeyword::Left,
  "right" => HorizontalKeyword::Right,
);

declare_enum_from_css_impl!(
  VerticalKeyword,
  "top" => VerticalKeyword::Top,
  "bottom" => VerticalKeyword::Bottom,
);

impl HorizontalKeyword {
  /// Returns the angle in degrees.
  pub fn degrees(&self) -> f32 {
    match self {
      HorizontalKeyword::Left => 270.0, // "to left" = 270deg
      HorizontalKeyword::Right => 90.0, // "to right" = 90deg
    }
  }

  /// Returns the mixed angle in degrees.
  pub fn vertical_mixed_degrees(&self) -> f32 {
    match self {
      HorizontalKeyword::Left => -45.0, // For diagonals with left
      HorizontalKeyword::Right => 45.0, // For diagonals with right
    }
  }
}

impl VerticalKeyword {
  /// Returns the angle in degrees.
  pub fn degrees(&self) -> f32 {
    match self {
      VerticalKeyword::Top => 0.0,
      VerticalKeyword::Bottom => 180.0,
    }
  }
}

impl<'i> FromCss<'i> for LinearGradient {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, LinearGradient> {
    input.expect_function_matching("linear-gradient")?;

    input.parse_nested_block(|input| {
      let mut angle = Angle::new(180.0);
      let mut interpolation = ColorInterpolationMethod::default();

      loop {
        if let Ok(parsed_angle) = input.try_parse(Angle::from_css) {
          angle = parsed_angle;
          continue;
        }

        if let Ok(parsed_interpolation) = input.try_parse(ColorInterpolationMethod::from_css) {
          interpolation = parsed_interpolation;
          continue;
        }

        break;
      }

      input.try_parse(Parser::expect_comma).ok();

      Ok(LinearGradient {
        angle,
        interpolation,
        stops: GradientStops::from_css(input)?.into_boxed_slice(),
      })
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("linear-gradient()")]
  }
}

impl Angle {
  /// Calculates the angle from horizontal and vertical keywords.
  pub fn degrees_from_keywords(
    horizontal: Option<HorizontalKeyword>,
    vertical: Option<VerticalKeyword>,
  ) -> Angle {
    match (horizontal, vertical) {
      (None, None) => Angle::new(180.0),
      (Some(horizontal), None) => Angle::new(horizontal.degrees()),
      (None, Some(vertical)) => Angle::new(vertical.degrees()),
      (Some(horizontal), Some(VerticalKeyword::Top)) => {
        Angle::new(horizontal.vertical_mixed_degrees())
      }
      (Some(horizontal), Some(VerticalKeyword::Bottom)) => {
        Angle::new(180.0 - horizontal.vertical_mixed_degrees())
      }
    }
  }
}

impl<'i> FromCss<'i> for Angle {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Angle> {
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Angle::zero());
    }

    let is_direction_keyword = input
      .try_parse(|input| input.expect_ident_matching("to"))
      .is_ok();

    if is_direction_keyword {
      if let Ok(vertical) = input.try_parse(VerticalKeyword::from_css) {
        if let Ok(horizontal) = input.try_parse(HorizontalKeyword::from_css) {
          return Ok(Angle::degrees_from_keywords(
            Some(horizontal),
            Some(vertical),
          ));
        }

        return Ok(Angle::new(vertical.degrees()));
      }

      if let Ok(horizontal) = input.try_parse(HorizontalKeyword::from_css) {
        return Ok(Angle::new(horizontal.degrees()));
      }

      return Err(input.new_error_for_next_token());
    }

    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Number { value, .. } => Ok(Angle::new(*value)),
      Token::Dimension { value, unit, .. } => match unit.as_ref() {
        "deg" => Ok(Angle::new(*value)),
        "grad" => Ok(Angle::new(*value / 400.0 * 360.0)),
        "turn" => Ok(Angle::new(*value * 360.0)),
        "rad" => Ok(Angle::new(value.to_degrees())),
        _ => Err(Self::unexpected_token_error(location, token)),
      },
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("angle"),
      CssToken::Keyword("to"),
      CssToken::Keyword("none"),
    ]
  }
}

#[cfg(test)]
mod tests {
  use color::{ColorSpaceTag, HueDirection};

  use crate::GlobalContext;

  use super::*;

  #[test]
  fn test_parse_linear_gradient() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to top right, #ff0000, #0000ff)"),
      Ok(LinearGradient {
        angle: Angle::new(45.0),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: None,
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: None,
          },
        ]
        .into(),
      })
    )
  }

  #[test]
  fn test_parse_angle() {
    assert_eq!(Angle::from_str("45deg"), Ok(Angle::new(45.0)));
  }

  #[test]
  fn test_parse_angle_grad() {
    // 200 grad = 200 * (π/200) = π radians = 180 degrees
    assert_eq!(Angle::from_str("200grad"), Ok(Angle::new(180.0)));
  }

  #[test]
  fn test_parse_angle_turn() {
    // 0.5 turn = 0.5 * 2π = π radians = 180 degrees
    assert_eq!(Angle::from_str("0.5turn"), Ok(Angle::new(180.0)));
  }

  #[test]
  fn test_parse_angle_rad() {
    // π radians = 180 degrees
    // Use approximate equality due to floating point precision
    assert!(Angle::from_str("3.14159rad").is_ok_and(|angle| (angle.0 - 180.0).abs() < 0.001));
  }

  #[test]
  fn test_parse_angle_number() {
    assert_eq!(Angle::from_str("90"), Ok(Angle::new(90.0)));
  }

  #[test]
  fn test_parse_direction_keywords_top() {
    assert_eq!(Angle::from_str("to top"), Ok(Angle::new(0.0)));
  }

  #[test]
  fn test_parse_direction_keywords_right() {
    assert_eq!(Angle::from_str("to right"), Ok(Angle::new(90.0)));
  }

  #[test]
  fn test_parse_direction_keywords_bottom() {
    assert_eq!(Angle::from_str("to bottom"), Ok(Angle::new(180.0)));
  }

  #[test]
  fn test_parse_direction_keywords_left() {
    assert_eq!(Angle::from_str("to left"), Ok(Angle::new(270.0)));
  }

  #[test]
  fn test_parse_direction_keywords_top_right() {
    assert_eq!(Angle::from_str("to top right"), Ok(Angle::new(45.0)));
  }

  #[test]
  fn test_parse_direction_keywords_bottom_left() {
    // 45 + 180 = 225 degrees
    assert_eq!(Angle::from_str("to bottom left"), Ok(Angle::new(225.0)));
  }

  #[test]
  fn test_parse_direction_keywords_top_left() {
    assert_eq!(Angle::from_str("to top left"), Ok(Angle::new(315.0)));
  }

  #[test]
  fn test_parse_direction_keywords_bottom_right() {
    assert_eq!(Angle::from_str("to bottom right"), Ok(Angle::new(135.0)));
  }

  #[test]
  fn test_parse_linear_gradient_with_angle() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(45deg, #ff0000, #0000ff)"),
      Ok(LinearGradient {
        angle: Angle::new(45.0),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: None,
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: None,
          },
        ]
        .into(),
      })
    )
  }

  #[test]
  fn test_parse_linear_gradient_with_interpolation_color_space() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(in oklab, #ff0000, #0000ff)"),
      Ok(LinearGradient {
        angle: Angle::new(180.0),
        interpolation: ColorInterpolationMethod {
          color_space: ColorSpaceTag::Oklab,
          hue_direction: HueDirection::Shorter,
        },
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: None,
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_with_interpolation_hue_direction() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to right in oklch longer hue, red, blue)"),
      Ok(LinearGradient {
        angle: Angle::new(90.0),
        interpolation: ColorInterpolationMethod {
          color_space: ColorSpaceTag::Oklch,
          hue_direction: HueDirection::Longer,
        },
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0xff0000)),
            hint: None,
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0x0000ff)),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_with_stops() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to right, #ff0000 0%, #0000ff 100%)"),
      Ok(LinearGradient {
        angle: Angle::new(90.0), // "to right" = 90deg
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: Some(StopPosition(Length::Percentage(0.0))),
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: Some(StopPosition(Length::Percentage(100.0))),
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_with_double_position_color_stop() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to right, red 10% 20%, blue)"),
      Ok(LinearGradient {
        angle: Angle::new(90.0),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0xff0000)),
            hint: Some(StopPosition(Length::Percentage(10.0))),
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0xff0000)),
            hint: Some(StopPosition(Length::Percentage(20.0))),
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0x0000ff)),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_with_hint() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to right, #ff0000, 50%, #0000ff)"),
      Ok(LinearGradient {
        angle: Angle::new(90.0), // "to right" = 90deg
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: None,
          },
          GradientStop::Hint(StopPosition(Length::Percentage(50.0))),
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_single_color() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to bottom, #ff0000)"),
      Ok(LinearGradient {
        angle: Angle::new(180.0),
        interpolation: ColorInterpolationMethod::default(),
        stops: [GradientStop::ColorHint {
          color: ColorInput::Value(Color([255, 0, 0, 255])),
          hint: None,
        }]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_default_angle() {
    // Default angle is 180 degrees (to bottom)
    assert_eq!(
      LinearGradient::from_str("linear-gradient(#ff0000, #0000ff)"),
      Ok(LinearGradient {
        angle: Angle::new(180.0),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0xff0000)),
            hint: None,
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0x0000ff)),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_gradient_hint_color() {
    assert_eq!(
      GradientStop::from_str("#ff0000"),
      Ok(GradientStop::ColorHint {
        color: ColorInput::Value(Color([255, 0, 0, 255])),
        hint: None,
      })
    );
  }

  #[test]
  fn test_parse_gradient_hint_numeric() {
    assert_eq!(
      GradientStop::from_str("50%"),
      Ok(GradientStop::Hint(StopPosition(Length::Percentage(50.0))))
    );
  }

  #[test]
  fn test_angle_degrees_from_keywords() {
    // None, None
    assert_eq!(Angle::degrees_from_keywords(None, None), Angle::new(180.0));

    // Some horizontal, None
    assert_eq!(
      Angle::degrees_from_keywords(Some(HorizontalKeyword::Left), None),
      Angle::new(270.0) // "to left" = 270deg
    );
    assert_eq!(
      Angle::degrees_from_keywords(Some(HorizontalKeyword::Right), None),
      Angle::new(90.0) // "to right" = 90deg
    );

    // None, Some vertical
    assert_eq!(
      Angle::degrees_from_keywords(None, Some(VerticalKeyword::Top)),
      Angle::new(0.0)
    );
    assert_eq!(
      Angle::degrees_from_keywords(None, Some(VerticalKeyword::Bottom)),
      Angle::new(180.0)
    );

    // Some horizontal, Some vertical
    assert_eq!(
      Angle::degrees_from_keywords(Some(HorizontalKeyword::Left), Some(VerticalKeyword::Top)),
      Angle::new(315.0)
    );
    assert_eq!(
      Angle::degrees_from_keywords(Some(HorizontalKeyword::Right), Some(VerticalKeyword::Top)),
      Angle::new(45.0)
    );
    assert_eq!(
      Angle::degrees_from_keywords(Some(HorizontalKeyword::Left), Some(VerticalKeyword::Bottom)),
      Angle::new(225.0)
    );
    assert_eq!(
      Angle::degrees_from_keywords(
        Some(HorizontalKeyword::Right),
        Some(VerticalKeyword::Bottom)
      ),
      Angle::new(135.0)
    );
  }

  #[test]
  fn test_parse_linear_gradient_mixed_hints_and_colors() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(45deg, #ff0000, 25%, #00ff00, 75%, #0000ff)"),
      Ok(LinearGradient {
        angle: Angle::new(45.0),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color([255, 0, 0, 255]).into(),
            hint: None,
          },
          GradientStop::Hint(StopPosition(Length::Percentage(25.0))),
          GradientStop::ColorHint {
            color: Color([0, 255, 0, 255]).into(),
            hint: None,
          },
          GradientStop::Hint(StopPosition(Length::Percentage(75.0))),
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
  fn test_linear_gradient_at_simple() {
    let gradient = LinearGradient {
      angle: Angle::new(180.0), // "to bottom" (default) - Top to bottom
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(), // Red
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(), // Blue
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    // Test at the top (should be red)
    let context = GlobalContext::default();
    let dummy_context = RenderContext::new_test(&context, (100, 100).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = LinearGradientTile::new(&gradient, 100, 100, &dummy_context, &mut buffer_pool);

    let color_top = tile.get_pixel(50, 0);
    assert_eq!(color_top, Rgba([255, 0, 0, 255]));

    // Test at the bottom (should be blue)
    let color_bottom = tile.get_pixel(50, 100);
    assert_eq!(color_bottom, Rgba([0, 0, 255, 255]));

    // Test in the middle (should be purple)
    let color_middle = tile.get_pixel(50, 50);
    // Middle should be roughly purple (red + blue)
    assert_eq!(color_middle, Rgba([127, 0, 127, 255]));
  }

  #[test]
  fn test_linear_gradient_at_horizontal() {
    let gradient = LinearGradient {
      angle: Angle::new(90.0), // "to right" - Left to right
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(), // Red
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(), // Blue
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    // Test at the left (should be red)
    let context = GlobalContext::default();
    let dummy_context = RenderContext::new_test(&context, (100, 100).into());

    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = LinearGradientTile::new(&gradient, 100, 100, &dummy_context, &mut buffer_pool);
    let color_left = tile.get_pixel(0, 50);
    assert_eq!(color_left, Rgba([255, 0, 0, 255]));

    // Test at the right (should be blue)
    let color_right = tile.get_pixel(100, 50);
    assert_eq!(color_right, Rgba([0, 0, 255, 255]));
  }

  #[test]
  fn test_linear_gradient_at_single_color() {
    let gradient = LinearGradient {
      angle: Angle::new(0.0),
      interpolation: ColorInterpolationMethod::default(),
      stops: [GradientStop::ColorHint {
        color: Color([255, 0, 0, 255]).into(), // Red
        hint: None,
      }]
      .into(),
    };

    // Should always return the same color
    let context = GlobalContext::default();
    let dummy_context = RenderContext::new_test(&context, (100, 100).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = LinearGradientTile::new(&gradient, 100, 100, &dummy_context, &mut buffer_pool);
    let color = tile.get_pixel(50, 50);
    assert_eq!(color, Rgba([255, 0, 0, 255]));
  }

  #[test]
  fn test_linear_gradient_at_no_steps() {
    let gradient = LinearGradient {
      angle: Angle::new(0.0),
      interpolation: ColorInterpolationMethod::default(),
      stops: [].into(),
    };

    // Should return transparent
    let context = GlobalContext::default();
    let dummy_context = RenderContext::new_test(&context, (100, 100).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = LinearGradientTile::new(&gradient, 100, 100, &dummy_context, &mut buffer_pool);
    let color = tile.get_pixel(50, 50);
    assert_eq!(color, Rgba([0, 0, 0, 0]));
  }

  #[test]
  fn test_linear_gradient_px_stops_crisp_line() -> ParseResult<'static, ()> {
    let gradient =
      LinearGradient::from_str("linear-gradient(to right, grey 1px, transparent 1px)")?;

    let context = GlobalContext::default();
    let dummy_context = RenderContext::new_test(&context, (40, 40).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = LinearGradientTile::new(&gradient, 40, 40, &dummy_context, &mut buffer_pool);

    // grey at 0,0
    let c0 = tile.get_pixel(0, 0);
    assert_eq!(c0, Rgba([128, 128, 128, 255]));

    // transparent at 1,0
    let c1 = tile.get_pixel(1, 0);
    assert_eq!(c1, Rgba([0, 0, 0, 0]));

    // transparent till the end
    let c2 = tile.get_pixel(40, 0);
    assert_eq!(c2, Rgba([0, 0, 0, 0]));

    Ok(())
  }

  #[test]
  fn test_linear_gradient_vertical_px_stops_top_pixel() -> ParseResult<'static, ()> {
    let gradient =
      LinearGradient::from_str("linear-gradient(to bottom, grey 1px, transparent 1px)")?;

    let context = GlobalContext::default();
    let dummy_context = RenderContext::new_test(&context, (40, 40).into());
    let mut buffer_pool = crate::rendering::BufferPool::default();
    let tile = LinearGradientTile::new(&gradient, 40, 40, &dummy_context, &mut buffer_pool);

    // color at top-left (0, 0) should be grey (1px hard stop)
    assert_eq!(tile.get_pixel(0, 0), Rgba([128, 128, 128, 255]));

    Ok(())
  }

  #[test]
  fn test_stop_position_parsing_fraction_number() {
    assert_eq!(
      StopPosition::from_str("0.25"),
      Ok(StopPosition(Length::Percentage(25.0)))
    );
  }

  #[test]
  fn test_stop_position_parsing_percentage() {
    assert_eq!(
      StopPosition::from_str("75%"),
      Ok(StopPosition(Length::Percentage(75.0)))
    );
  }

  #[test]
  fn test_stop_position_parsing_length_px() {
    assert_eq!(
      StopPosition::from_str("12px"),
      Ok(StopPosition(Length::Px(12.0)))
    );
  }

  #[test]
  fn test_stop_position_value_css_roundtrip() {
    assert_eq!(
      StopPosition::from_str("50%"),
      Ok(StopPosition(Length::Percentage(50.0)))
    );

    assert_eq!(
      StopPosition::from_str("8px"),
      Ok(StopPosition(Length::Px(8.0)))
    );
  }

  #[test]
  fn resolve_stops_percentage_and_px_linear() {
    let gradient = LinearGradient {
      angle: Angle::new(0.0),
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
    let ctx = RenderContext::new_test(&context, (200, 100).into());

    let resolved = resolve_stops_along_axis(
      &gradient.stops,
      ctx.sizing.viewport.width.unwrap_or_default() as f32,
      &ctx,
    );
    assert_eq!(resolved.len(), 3);
    assert!((resolved[0].position - 0.0).abs() < 1e-3);
    assert!((resolved[1].position - 100.0).abs() < 1e-3);
    assert!((resolved[2].position - 100.0).abs() < 1e-3);
  }

  #[test]
  fn resolve_stops_equal_positions_allowed_linear() {
    let gradient = LinearGradient {
      angle: Angle::new(0.0),
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
      ]
      .into(),
    };
    let context = GlobalContext::default();
    let ctx = RenderContext::new_test(&context, (200, 100).into());

    let resolved = resolve_stops_along_axis(
      &gradient.stops,
      ctx.sizing.viewport.width.unwrap_or_default() as f32,
      &ctx,
    );
    assert_eq!(resolved.len(), 2);
    assert!((resolved[0].position - 0.0).abs() < 1e-3);
    assert!((resolved[1].position - 0.0).abs() < 1e-3);
  }
}
