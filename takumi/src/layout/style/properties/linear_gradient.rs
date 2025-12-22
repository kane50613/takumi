use cssparser::{Parser, Token, match_ignore_ascii_case};
use image::RgbaImage;
use smallvec::SmallVec;
use std::ops::{Deref, Neg};

use super::gradient_utils::{color_from_stops, resolve_stops_along_axis};
use crate::{
  layout::style::{
    Color, ColorInput, FromCss, LengthUnit, ParseResult, tw::TailwindPropertyParser,
  },
  rendering::RenderContext,
};

/// A trait for gradients that can be sampled at a specific point.
/// This trait is used to avoid trait objects in the rendering pipeline.
pub(crate) trait Gradient: Send + Sync {
  /// The type of the draw context.
  type DrawContext: Send + Sync;

  /// Returns the color at a specific point in the gradient.
  fn at(&self, x: u32, y: u32, ctx: &Self::DrawContext) -> Color;

  /// Creates a draw context for the gradient.
  fn to_draw_context(&self, width: f32, height: f32, context: &RenderContext) -> Self::DrawContext;

  /// Creates an image of the gradient.
  fn to_image(&self, width: u32, height: u32, context: &RenderContext) -> RgbaImage {
    let ctx = self.to_draw_context(width as f32, height as f32, context);

    #[cfg(feature = "rayon")]
    {
      RgbaImage::from_par_fn(width, height, |x, y| self.at(x, y, &ctx).into())
    }
    #[cfg(not(feature = "rayon"))]
    RgbaImage::from_fn(width, height, |x, y| self.at(x, y, &ctx).into())
  }
}

/// Represents a linear gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
  /// The angle of the gradient.
  pub angle: Angle,
  /// The steps of the gradient.
  pub stops: SmallVec<[GradientStop; 4]>,
}

impl Gradient for LinearGradient {
  type DrawContext = LinearGradientDrawContext;

  fn at(&self, x: u32, y: u32, ctx: &Self::DrawContext) -> Color {
    let stops_len = self.stops.len();

    // Fast-paths
    if stops_len == 0 {
      return Color([0, 0, 0, 0]);
    }
    if stops_len == 1 {
      return ctx.resolved_stops[0].color;
    }

    let dx = x as f32 - ctx.cx;
    let dy = y as f32 - ctx.cy;
    let projection = dx * ctx.dir_x + dy * ctx.dir_y;
    let position_px = (projection + ctx.max_extent).clamp(0.0, ctx.axis_length);

    color_from_stops(position_px, &ctx.resolved_stops)
  }

  fn to_draw_context(&self, width: f32, height: f32, context: &RenderContext) -> Self::DrawContext {
    LinearGradientDrawContext::new(self, width, height, context)
  }
}

impl LinearGradient {
  /// Resolves gradient steps into color stops with positions expressed in pixels along the gradient axis.
  /// Supports non-px length units when a `RenderContext` is provided.
  pub(crate) fn resolve_stops_for_axis_size(
    &self,
    axis_size_px: f32,
    context: &RenderContext,
  ) -> SmallVec<[ResolvedGradientStop; 4]> {
    resolve_stops_along_axis(&self.stops, axis_size_px, context)
  }
}

/// Precomputed drawing context for repeated sampling of a `LinearGradient`.
#[derive(Debug, Clone)]
pub struct LinearGradientDrawContext {
  /// Target width in pixels.
  pub width: f32,
  /// Target height in pixels.
  pub height: f32,
  /// Direction vector X component derived from angle.
  pub dir_x: f32,
  /// Direction vector Y component derived from angle.
  pub dir_y: f32,
  /// Center X coordinate.
  pub cx: f32,
  /// Center Y coordinate.
  pub cy: f32,
  /// Half of axis length along gradient direction in pixels.
  pub max_extent: f32,
  /// Full axis length along gradient direction in pixels.
  pub axis_length: f32,
  /// Resolved and ordered color stops (positions in pixels).
  pub resolved_stops: SmallVec<[ResolvedGradientStop; 4]>,
}

impl LinearGradientDrawContext {
  /// Builds a drawing context from a gradient and a target viewport.
  pub fn new(gradient: &LinearGradient, width: f32, height: f32, context: &RenderContext) -> Self {
    let rad = gradient.angle.0.to_radians();
    let (dir_x, dir_y) = (rad.sin(), -rad.cos());

    let cx = width / 2.0;
    let cy = height / 2.0;
    let max_extent = ((width * dir_x.abs()) + (height * dir_y.abs())) / 2.0;
    let axis_length = 2.0 * max_extent;

    let resolved_stops = gradient.resolve_stops_for_axis_size(axis_length.max(1e-6), context);

    LinearGradientDrawContext {
      width,
      height,
      dir_x,
      dir_y,
      cx,
      cy,
      max_extent,
      axis_length,
      resolved_stops,
    }
  }
}

/// Represents a gradient stop position.
/// If a percentage or number (0.0-1.0) is provided, it is treated as a percentage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopPosition(pub LengthUnit);

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
      return Ok(StopPosition(LengthUnit::Percentage(
        num.clamp(0.0, 1.0) * 100.0,
      )));
    }

    if let Ok(unit_value) = input.try_parse(Parser::expect_percentage) {
      return Ok(StopPosition(LengthUnit::Percentage(unit_value * 100.0)));
    }

    let Ok(length) = input.try_parse(LengthUnit::from_css) else {
      return Err(input.new_error_for_next_token());
    };

    Ok(StopPosition(length))
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
}

/// Represents an angle value in degrees.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Angle(f32);

impl From<Angle> for zeno::Angle {
  fn from(angle: Angle) -> Self {
    zeno::Angle::from_degrees(angle.0)
  }
}

impl TailwindPropertyParser for Angle {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("none") {
      return Some(Angle::zero());
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

impl<'i> FromCss<'i> for HorizontalKeyword {
  /// Parses a horizontal keyword.
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, HorizontalKeyword> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! {&ident,
      "left" => Ok(HorizontalKeyword::Left),
      "right" => Ok(HorizontalKeyword::Right),
      _ => Err(location.new_basic_unexpected_token_error(Token::Ident(ident.clone())).into()),
    }
  }
}

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

impl<'i> FromCss<'i> for VerticalKeyword {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! {&ident,
      "top" => Ok(VerticalKeyword::Top),
      "bottom" => Ok(VerticalKeyword::Bottom),
      _ => Err(location.new_basic_unexpected_token_error(Token::Ident(ident.clone())).into()),
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
      let angle = if let Ok(angle) = input.try_parse(Angle::from_css) {
        input.try_parse(Parser::expect_comma).ok();

        angle
      } else {
        Angle::new(180.0)
      };

      let mut stops = SmallVec::new();

      stops.push(GradientStop::from_css(input)?);

      while input.try_parse(Parser::expect_comma).is_ok() {
        stops.push(GradientStop::from_css(input)?);
      }

      Ok(LinearGradient { angle, stops })
    })
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
        _ => Err(
          location
            .new_basic_unexpected_token_error(token.clone())
            .into(),
        ),
      },
      _ => Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      ),
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::GlobalContext;
  use smallvec::smallvec;

  use super::*;

  #[test]
  fn test_parse_linear_gradient() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to top right, #ff0000, #0000ff)"),
      Ok(LinearGradient {
        angle: Angle::new(45.0),
        stops: smallvec![
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: None,
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: None,
          },
        ]
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
        stops: smallvec![
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: None,
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: None,
          },
        ]
      })
    )
  }

  #[test]
  fn test_parse_linear_gradient_with_stops() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to right, #ff0000 0%, #0000ff 100%)"),
      Ok(LinearGradient {
        angle: Angle::new(90.0), // "to right" = 90deg
        stops: smallvec![
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: Some(StopPosition(LengthUnit::Percentage(0.0))),
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: Some(StopPosition(LengthUnit::Percentage(100.0))),
          },
        ]
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_with_hint() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to right, #ff0000, 50%, #0000ff)"),
      Ok(LinearGradient {
        angle: Angle::new(90.0), // "to right" = 90deg
        stops: smallvec![
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([255, 0, 0, 255])),
            hint: None,
          },
          GradientStop::Hint(StopPosition(LengthUnit::Percentage(50.0))),
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 0, 255, 255])),
            hint: None,
          },
        ]
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_single_color() {
    assert_eq!(
      LinearGradient::from_str("linear-gradient(to bottom, #ff0000)"),
      Ok(LinearGradient {
        angle: Angle::new(180.0),
        stops: smallvec![GradientStop::ColorHint {
          color: ColorInput::Value(Color([255, 0, 0, 255])),
          hint: None,
        },]
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
        stops: smallvec![
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0xff0000)),
            hint: None,
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color::from_rgb(0x0000ff)),
            hint: None,
          },
        ]
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
      Ok(GradientStop::Hint(StopPosition(LengthUnit::Percentage(
        50.0
      ))))
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
        stops: smallvec![
          GradientStop::ColorHint {
            color: Color([255, 0, 0, 255]).into(),
            hint: None,
          },
          GradientStop::Hint(StopPosition(LengthUnit::Percentage(25.0))),
          GradientStop::ColorHint {
            color: Color([0, 255, 0, 255]).into(),
            hint: None,
          },
          GradientStop::Hint(StopPosition(LengthUnit::Percentage(75.0))),
          GradientStop::ColorHint {
            color: Color([0, 0, 255, 255]).into(),
            hint: None,
          },
        ]
      })
    );
  }

  #[test]
  fn test_linear_gradient_at_simple() {
    let gradient = LinearGradient {
      angle: Angle::new(180.0), // "to bottom" (default) - Top to bottom
      stops: smallvec![
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(), // Red
          hint: Some(StopPosition(LengthUnit::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(), // Blue
          hint: Some(StopPosition(LengthUnit::Percentage(100.0))),
        },
      ],
    };

    // Test at the top (should be red)
    let context = GlobalContext::default();
    let dummy_context = RenderContext::new(&context, (100, 100).into(), Default::default());
    let ctx = gradient.to_draw_context(100.0, 100.0, &dummy_context);
    let color_top = gradient.at(50, 0, &ctx);
    assert_eq!(color_top, Color([255, 0, 0, 255]));

    // Test at the bottom (should be blue)
    let color_bottom = gradient.at(50, 100, &ctx);
    assert_eq!(color_bottom, Color([0, 0, 255, 255]));

    // Test in the middle (should be purple)
    let color_middle = gradient.at(50, 50, &ctx);
    // Middle should be roughly purple (red + blue)
    let [r, g, b, a] = color_middle.0;
    assert_eq!(r, 128); // Approximately halfway between 255 and 0
    assert_eq!(g, 0); // No green component
    assert_eq!(b, 128); // Approximately halfway between 0 and 255
    assert_eq!(a, 255); // Fully opaque
  }

  #[test]
  fn test_linear_gradient_at_horizontal() {
    let gradient = LinearGradient {
      angle: Angle::new(90.0), // "to right" - Left to right
      stops: smallvec![
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(), // Red
          hint: Some(StopPosition(LengthUnit::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(), // Blue
          hint: Some(StopPosition(LengthUnit::Percentage(100.0))),
        },
      ],
    };

    // Test at the left (should be red)
    let context = GlobalContext::default();
    let dummy_context = RenderContext::new(&context, (100, 100).into(), Default::default());
    let ctx = gradient.to_draw_context(100.0, 100.0, &dummy_context);
    let color_left = gradient.at(0, 50, &ctx);
    assert_eq!(color_left, Color([255, 0, 0, 255]));

    // Test at the right (should be blue)
    let color_right = gradient.at(100, 50, &ctx);
    assert_eq!(color_right, Color([0, 0, 255, 255]));
  }

  #[test]
  fn test_linear_gradient_at_single_color() {
    let gradient = LinearGradient {
      angle: Angle::new(0.0),
      stops: smallvec![GradientStop::ColorHint {
        color: Color([255, 0, 0, 255]).into(), // Red
        hint: None,
      }],
    };

    // Should always return the same color
    let context = GlobalContext::default();
    let dummy_context = RenderContext::new(&context, (100, 100).into(), Default::default());
    let ctx = gradient.to_draw_context(100.0, 100.0, &dummy_context);
    let color = gradient.at(50, 50, &ctx);
    assert_eq!(color, Color([255, 0, 0, 255]));
  }

  #[test]
  fn test_linear_gradient_at_no_steps() {
    let gradient = LinearGradient {
      angle: Angle::new(0.0),
      stops: smallvec![],
    };

    // Should return transparent
    let context = GlobalContext::default();
    let dummy_context = RenderContext::new(&context, (100, 100).into(), Default::default());
    let ctx = gradient.to_draw_context(100.0, 100.0, &dummy_context);
    let color = gradient.at(50, 50, &ctx);
    assert_eq!(color, Color([0, 0, 0, 0]));
  }

  #[test]
  fn test_linear_gradient_px_stops_crisp_line() -> ParseResult<'static, ()> {
    let gradient =
      LinearGradient::from_str("linear-gradient(to right, grey 1px, transparent 1px)")?;

    let context = GlobalContext::default();
    let dummy_context = RenderContext::new(&context, (40, 40).into(), Default::default());
    let ctx = gradient.to_draw_context(40.0, 40.0, &dummy_context);

    // grey at 0,0
    let c0 = gradient.at(0, 0, &ctx);
    assert_eq!(c0, Color([128, 128, 128, 255]));

    // transparent at 1,0
    let c1 = gradient.at(1, 0, &ctx);
    assert_eq!(c1, Color([0, 0, 0, 0]));

    // transparent till the end
    let c2 = gradient.at(40, 0, &ctx);
    assert_eq!(c2, Color([0, 0, 0, 0]));

    Ok(())
  }

  #[test]
  fn test_linear_gradient_vertical_px_stops_top_pixel() -> ParseResult<'static, ()> {
    let gradient =
      LinearGradient::from_str("linear-gradient(to bottom, grey 1px, transparent 1px)")?;

    let context = GlobalContext::default();
    let dummy_context = RenderContext::new(&context, (40, 40).into(), Default::default());
    let ctx = gradient.to_draw_context(40.0, 40.0, &dummy_context);

    // color at top-left (0, 0) should be grey (1px hard stop)
    assert_eq!(gradient.at(0, 0, &ctx), Color([128, 128, 128, 255]));

    Ok(())
  }

  #[test]
  fn test_stop_position_parsing_fraction_number() {
    assert_eq!(
      StopPosition::from_str("0.25"),
      Ok(StopPosition(LengthUnit::Percentage(25.0)))
    );
  }

  #[test]
  fn test_stop_position_parsing_percentage() {
    assert_eq!(
      StopPosition::from_str("75%"),
      Ok(StopPosition(LengthUnit::Percentage(75.0)))
    );
  }

  #[test]
  fn test_stop_position_parsing_length_px() {
    assert_eq!(
      StopPosition::from_str("12px"),
      Ok(StopPosition(LengthUnit::Px(12.0)))
    );
  }

  #[test]
  fn test_stop_position_value_css_roundtrip() {
    assert_eq!(
      StopPosition::from_str("50%"),
      Ok(StopPosition(LengthUnit::Percentage(50.0)))
    );

    assert_eq!(
      StopPosition::from_str("8px"),
      Ok(StopPosition(LengthUnit::Px(8.0)))
    );
  }

  #[test]
  fn resolve_stops_percentage_and_px_linear() {
    let gradient = LinearGradient {
      angle: Angle::new(0.0),
      stops: smallvec![
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(LengthUnit::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(LengthUnit::Percentage(50.0))),
        },
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(LengthUnit::Px(100.0))),
        },
      ],
    };

    let context = GlobalContext::default();
    let ctx = RenderContext::new(&context, (200, 100).into(), Default::default());

    let resolved = gradient
      .resolve_stops_for_axis_size(ctx.sizing.viewport.width.unwrap_or_default() as f32, &ctx);
    assert_eq!(resolved.len(), 3);
    assert!((resolved[0].position - 0.0).abs() < 1e-3);
    assert!((resolved[1].position - 100.0).abs() < 1e-3);
    assert!((resolved[2].position - 100.0).abs() < 1e-3);
  }

  #[test]
  fn resolve_stops_equal_positions_allowed_linear() {
    let gradient = LinearGradient {
      angle: Angle::new(0.0),
      stops: smallvec![
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(LengthUnit::Px(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(LengthUnit::Px(0.0))),
        },
      ],
    };
    let context = GlobalContext::default();
    let ctx = RenderContext::new(&context, (200, 100).into(), Default::default());

    let resolved = gradient
      .resolve_stops_for_axis_size(ctx.sizing.viewport.width.unwrap_or_default() as f32, &ctx);
    assert_eq!(resolved.len(), 2);
    assert!((resolved[0].position - 0.0).abs() < 1e-3);
    assert!((resolved[1].position - 0.0).abs() < 1e-3);
  }
}
