use std::{f32::consts::SQRT_2, fmt};

use cssparser::{Parser, Token, match_ignore_ascii_case};
use tiny_skia::PremultipliedColorU8;
use typed_builder::TypedBuilder;

use super::gradient_utils::{
  ColorLut, GradientOverlayTile, LutAxis, gradient_tile_accessors, parse_gradient_stops,
  write_gradient_css,
};
use crate::style::{
  Color, ColorInterpolationMethod, CssDescriptorKind, CssToken, FromCss, GradientStop, Length,
  MakeComputed, ParseResult, PositionValue, ResolvedGradientStop, SizingContext, StopPosition,
  ToCss, declare_enum_from_css_impl, unexpected_token,
};

/// Radii of the ellipse through `corner`, as Blink's `EllipseRadius`
/// (css-images-3 §3.2.1: a corner-sized ellipse keeps the matching side
/// keyword's aspect ratio, which puts both radii at `sqrt(2)` times the
/// corner offsets). A corner on an axis leaves no ellipse to size.
fn ellipse_radii_through((x, y): (f32, f32)) -> (f32, f32) {
  if x == 0.0 || y == 0.0 {
    return (0.0, 0.0);
  }

  (x * SQRT_2, y * SQRT_2)
}

/// Represents a radial gradient.
#[derive(Debug, Clone, PartialEq, TypedBuilder)]
#[non_exhaustive]
pub struct RadialGradient {
  /// Whether the gradient repeats beyond the last stop.
  #[builder(default)]
  pub repeating: bool,
  /// The radial gradient shape
  #[builder(default)]
  pub shape: RadialShape,
  /// The sizing mode for the gradient
  #[builder(default)]
  pub size: RadialSize,
  /// Center position
  #[builder(default = PositionValue::center())]
  pub center: PositionValue,
  /// The color interpolation method used between stops.
  #[builder(default = ColorInterpolationMethod::LEGACY)]
  pub interpolation: ColorInterpolationMethod,
  /// Gradient stops
  #[builder(setter(into))]
  pub stops: Box<[GradientStop]>,
}

impl MakeComputed for RadialGradient {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.center.make_computed(sizing);
    self.stops.make_computed(sizing);
  }
}

/// Supported shapes for radial gradients
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
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
#[non_exhaustive]
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
  /// Explicit radii. Percentages resolve against the corresponding axis of the gradient box.
  ///
  /// For `circle`, the larger of the two radii is used.
  Explicit {
    /// Horizontal radius.
    radius_x: Length,
    /// Vertical radius.
    radius_y: Length,
  },
}
impl<'i> FromCss<'i> for RadialSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! { &ident,
      "closest-side" => Ok(RadialSize::ClosestSide),
      "farthest-side" => Ok(RadialSize::FarthestSide),
      "closest-corner" => Ok(RadialSize::ClosestCorner),
      "farthest-corner" => Ok(RadialSize::FarthestCorner),
      _ => Err(unexpected_token!(location, &Token::Ident(ident.clone()))),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("closest-side"),
    CssToken::Keyword("farthest-side"),
    CssToken::Keyword("closest-corner"),
    CssToken::Keyword("farthest-corner"),
  ];
}

impl MakeComputed for RadialSize {}

/// Precomputed drawing context for repeated sampling of a `RadialGradient`.
#[derive(Debug, Clone)]
pub struct RadialGradientTile {
  /// Target width in pixels.
  pub width: u32,
  /// Target height in pixels.
  pub height: u32,
  /// Center X coordinate in pixels
  pub cx: f32,
  /// Center Y coordinate in pixels
  pub cy: f32,
  /// Reciprocal of `radius_x` for sampling.
  pub inv_radius_x: f32,
  /// Reciprocal of `radius_y` for sampling.
  pub inv_radius_y: f32,
  /// Axis length used to resolve stop percentages (in pixels).
  pub radius_scale: f32,
  /// Whether this gradient repeats.
  pub repeating: bool,
  /// First resolved stop position in pixels, used as repeating origin.
  pub repeat_start: f32,
  /// Repeat period in pixels.
  pub repeat_period: f32,
  /// Scale converting axis-space distance in pixels into LUT index space.
  pub position_to_lut_scale: f32,
  /// Whether every LUT entry has alpha = 255.
  pub fully_opaque: bool,
  /// Pre-computed colour samples along the radius.
  pub lut: ColorLut,
}

/// Per-row sampling state for incremental distance stepping.
#[derive(Debug, Clone, Copy)]
pub struct RadialGradientRowState {
  dx2: f32,
  dx2_step: f32,
  dx2_step_delta: f32,
  dy2: f32,
  max_lut_index: usize,
}

impl RadialGradientTile {
  /// Color of the outermost LUT entry, used outside the gradient ellipse.
  #[inline(always)]
  pub fn outer_sample(&self) -> Option<PremultipliedColorU8> {
    self.lut.colors().last().copied()
  }

  /// Span of `x` within a row that falls inside the gradient ellipse.
  #[inline(always)]
  pub fn non_repeating_active_span(
    &self,
    src_x_start: u32,
    src_x_end: u32,
    src_y: u32,
  ) -> Option<(u32, u32)> {
    if self.repeating || src_x_start >= src_x_end {
      return None;
    }

    let dy = (src_y as f32 - self.cy) * self.inv_radius_y;
    let dy2 = dy * dy;
    if dy2 >= 1.0 {
      return Some((src_x_start, src_x_start));
    }

    let max_dx = (1.0 - dy2).sqrt() / self.inv_radius_x;
    let active_start = (self.cx - max_dx).floor() as i32 + 1;
    let active_end = (self.cx + max_dx).ceil() as i32;
    let clamped_start = active_start.max(src_x_start as i32).min(src_x_end as i32) as u32;
    let clamped_end = active_end.max(clamped_start as i32).min(src_x_end as i32) as u32;
    Some((clamped_start, clamped_end))
  }

  /// Maps an axis-space distance to a LUT index for a LUT of `lut_len`.
  #[inline(always)]
  pub(crate) fn lut_index_for_distance_px_with_len(
    &self,
    distance_px: f32,
    lut_len: usize,
  ) -> usize {
    if lut_len <= 1 {
      return 0;
    }

    let position_px = if self.repeating && self.repeat_period > 1e-6 {
      (distance_px - self.repeat_start).rem_euclid(self.repeat_period)
    } else {
      distance_px.clamp(0.0, self.radius_scale)
    };

    ((position_px * self.position_to_lut_scale).round() as usize).min(lut_len - 1)
  }

  /// Builds a drawing context from a gradient and a target viewport.
  pub fn new(
    gradient: &RadialGradient,
    width: u32,
    height: u32,
    sizing: &SizingContext,
    current_color: Color,
    dither: bool,
  ) -> Self {
    let cx = Length::from(gradient.center.0.x).to_px(sizing, width as f32);
    let cy = Length::from(gradient.center.0.y).to_px(sizing, height as f32);

    // Absolute distances to the sides, so an out-of-box center still measures
    // non-negative radii (Blink RadiusToSide).
    let dx_left = cx.abs();
    let dx_right = (width as f32 - cx).abs();
    let dy_top = cy.abs();
    let dy_bottom = (height as f32 - cy).abs();

    let corner_distances = [
      (dx_left, dy_top),
      (dx_left, dy_bottom),
      (dx_right, dy_top),
      (dx_right, dy_bottom),
    ]
    .map(|(dx, dy)| (dx * dx + dy * dy).sqrt());

    let (radius_x, radius_y) = match (gradient.shape, gradient.size) {
      (shape, RadialSize::Explicit { radius_x, radius_y }) => {
        let resolved_radius_x = radius_x.to_px(sizing, width as f32).max(0.0);
        let resolved_radius_y = radius_y.to_px(sizing, height as f32).max(0.0);

        match shape {
          RadialShape::Circle => {
            let r = resolved_radius_x.max(resolved_radius_y);
            (r, r)
          }
          RadialShape::Ellipse => (resolved_radius_x, resolved_radius_y),
        }
      }
      (RadialShape::Ellipse, RadialSize::FarthestCorner) => {
        ellipse_radii_through((dx_left.max(dx_right), dy_top.max(dy_bottom)))
      }
      (RadialShape::Circle, RadialSize::FarthestCorner) => {
        let r = corner_distances.into_iter().fold(0.0_f32, f32::max);
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
        ellipse_radii_through((dx_left.min(dx_right), dy_top.min(dy_bottom)))
      }
      (RadialShape::Circle, RadialSize::ClosestCorner) => {
        let r = corner_distances.into_iter().fold(f32::INFINITY, f32::min);
        (r, r)
      }
    };

    let radius_scale = radius_x.max(radius_y);
    let resolved_stops = ResolvedGradientStop::resolve(
      &gradient.stops,
      radius_scale.max(1e-6),
      sizing,
      current_color,
    );
    let axis = LutAxis::new(gradient.repeating, resolved_stops, radius_scale);
    let lut_size = axis.lut_size_covering((radius_scale.ceil() as usize).saturating_add(1));
    let lut = axis.lut(lut_size, gradient.interpolation, dither);
    let lut_len = lut.len();
    let inv_radius_x = radius_x.max(1e-6).recip();
    let inv_radius_y = radius_y.max(1e-6).recip();
    let position_to_lut_scale = if axis.length.abs() <= f32::EPSILON || lut_len <= 1 {
      0.0
    } else {
      (lut_len - 1) as f32 / axis.length
    };
    let fully_opaque = lut.colors().iter().all(|p| p.alpha() == u8::MAX);

    RadialGradientTile {
      width,
      height,
      cx,
      cy,
      inv_radius_x,
      inv_radius_y,
      radius_scale,
      repeating: axis.repeating,
      repeat_start: axis.repeat_start,
      repeat_period: axis.repeat_period,
      position_to_lut_scale,
      fully_opaque,
      lut,
    }
  }

  #[inline(always)]
  fn pixel_lut_index(&self, x: u32, y: u32) -> usize {
    let dx = (x as f32 - self.cx) * self.inv_radius_x;
    let dy = (y as f32 - self.cy) * self.inv_radius_y;
    let distance_px = (dx * dx + dy * dy).sqrt() * self.radius_scale;

    self.lut_index_for_distance_px_with_len(distance_px, self.lut.len())
  }
}

impl GradientOverlayTile for RadialGradientTile {
  type RowState = RadialGradientRowState;

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
    let dy = (src_y as f32 - self.cy) * self.inv_radius_y;
    let dx = (src_x_start as f32 - self.cx) * self.inv_radius_x;
    let dx_step = self.inv_radius_x;
    RadialGradientRowState {
      dx2: dx * dx,
      dx2_step: 2.0 * dx * dx_step + dx_step * dx_step,
      dx2_step_delta: 2.0 * dx_step * dx_step,
      dy2: dy * dy,
      max_lut_index: lut_len.saturating_sub(1),
    }
  }

  #[inline(always)]
  fn next_lut_index(&self, row_state: &mut Self::RowState) -> usize {
    if !self.repeating && row_state.dy2 >= 1.0 {
      return row_state.max_lut_index;
    }

    let normalized_distance = (row_state.dx2 + row_state.dy2).sqrt();
    let distance_px = normalized_distance * self.radius_scale;
    let lut_idx = self.lut_index_for_distance_px_with_len(distance_px, row_state.max_lut_index + 1);
    row_state.dx2 += row_state.dx2_step;
    row_state.dx2_step += row_state.dx2_step_delta;
    lut_idx
  }
}

impl<'i> FromCss<'i> for RadialGradient {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, RadialGradient> {
    let location = input.current_source_location();
    let name = input.expect_function()?;
    let repeating = match_ignore_ascii_case! { &name,
      "radial-gradient" => false,
      "repeating-radial-gradient" => true,
      _ => return Err(unexpected_token!(location, &Token::Function(name.clone()))),
    };

    input.parse_nested_block(|input| {
      let mut shape = RadialShape::Ellipse;
      let mut size = RadialSize::FarthestCorner;
      let mut center = PositionValue::center();
      let mut interpolation = None;

      loop {
        if let Ok(s) = input.try_parse(RadialShape::from_css) {
          shape = s;
          continue;
        }

        if let Ok(s) = input.try_parse(RadialSize::from_css) {
          size = s;
          continue;
        }

        if let Ok(radius_x) = input.try_parse(Length::from_css) {
          let radius_y = input.try_parse(Length::from_css).unwrap_or(radius_x);
          size = RadialSize::Explicit { radius_x, radius_y };
          continue;
        }

        if input.try_parse(|i| i.expect_ident_matching("at")).is_ok() {
          center = PositionValue::from_css(input)?;
          continue;
        }

        if let Ok(parsed_interpolation) = input.try_parse(ColorInterpolationMethod::from_css) {
          interpolation = Some(parsed_interpolation);
          continue;
        }

        input.try_parse(Parser::expect_comma).ok();

        break;
      }

      let (stops, modern) = parse_gradient_stops(input, StopPosition::from_css)?;

      Ok(RadialGradient {
        repeating,
        shape,
        size,
        center,
        interpolation: interpolation.unwrap_or(ColorInterpolationMethod::gradient_default(modern)),
        stops: stops.into_boxed_slice(),
      })
    })
  }

  const VALID_TOKENS: &'static [CssToken] =
    &[CssToken::Descriptor(CssDescriptorKind::RadialGradientFn)];
}

impl ToCss for RadialSize {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::ClosestSide => dest.write_str("closest-side"),
      Self::FarthestSide => dest.write_str("farthest-side"),
      Self::ClosestCorner => dest.write_str("closest-corner"),
      Self::FarthestCorner => dest.write_str("farthest-corner"),
      Self::Explicit { radius_x, radius_y } => {
        radius_x.to_css(dest)?;
        if radius_x != radius_y {
          dest.write_char(' ')?;
          radius_y.to_css(dest)?;
        }
        Ok(())
      }
    }
  }
}

impl ToCss for RadialGradient {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let name = if self.repeating {
      "repeating-radial-gradient"
    } else {
      "radial-gradient"
    };

    // Build shape/size/center as a temp buffer to check if anything is non-default
    let mut shape_size_buf = String::new();
    if self.shape != RadialShape::Ellipse {
      self.shape.to_css(&mut shape_size_buf)?;
    }
    if self.size != RadialSize::FarthestCorner {
      if !shape_size_buf.is_empty() {
        shape_size_buf.push(' ');
      }
      self.size.to_css(&mut shape_size_buf)?;
    }

    let mut center_buf = String::new();
    self.center.to_css(&mut center_buf)?;
    let is_center_default = center_buf == "center center" || center_buf == "50% 50%";

    let mut params = String::new();
    if !shape_size_buf.is_empty() || !is_center_default {
      params.push_str(&shape_size_buf);
      if !is_center_default {
        if !shape_size_buf.is_empty() {
          params.push(' ');
        }
        params.push_str("at ");
        params.push_str(&center_buf);
      }
    }

    write_gradient_css(dest, name, &params, &self.interpolation, &self.stops)
  }
}

#[cfg(test)]
mod tests {
  use color::{ColorSpaceTag, HueDirection};
  use tiny_skia::ColorU8;

  use super::*;
  use crate::{
    style::{
      Color, FromCssStr, Length, PositionComponent, PositionKeywordX, PositionKeywordY,
      PositionValue, SpacePair, StopPosition, properties::gradient_utils::red_blue_stops,
    },
    viewport::Viewport,
  };
  #[test]
  fn test_parse_radial_gradient_basic() {
    let gradient = RadialGradient::from_css_str("radial-gradient(#ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(
        RadialGradient::builder()
          .stops(red_blue_stops(None, None))
          .build()
      )
    );
  }

  #[test]
  fn test_parse_radial_gradient_with_interpolation_color_space() {
    assert_eq!(
      RadialGradient::from_css_str("radial-gradient(in oklab, red, blue)"),
      Ok(
        RadialGradient::builder()
          .interpolation(ColorInterpolationMethod {
            color_space: ColorSpaceTag::Oklab,
            hue_direction: HueDirection::Shorter,
          })
          .stops([
            GradientStop::ColorHint {
              color: Color::from_rgb(0xff0000).into(),
              hint: None,
            },
            GradientStop::ColorHint {
              color: Color::from_rgb(0x0000ff).into(),
              hint: None,
            },
          ])
          .build()
      )
    );
  }

  #[test]
  fn test_parse_radial_gradient_circle_farthest_side() {
    let gradient =
      RadialGradient::from_css_str("radial-gradient(circle farthest-side, #ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(
        RadialGradient::builder()
          .shape(RadialShape::Circle)
          .size(RadialSize::FarthestSide)
          .stops(red_blue_stops(None, None))
          .build()
      )
    );
  }

  #[test]
  fn test_parse_radial_gradient_ellipse_at_left_top() {
    let gradient =
      RadialGradient::from_css_str("radial-gradient(ellipse at left top, #ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(
        RadialGradient::builder()
          .center(PositionValue(SpacePair::from_pair(
            PositionComponent::KeywordX(PositionKeywordX::Left),
            PositionComponent::KeywordY(PositionKeywordY::Top),
          )))
          .stops(red_blue_stops(None, None))
          .build()
      )
    );
  }

  #[test]
  fn test_parse_radial_gradient_size_then_position() {
    let gradient =
      RadialGradient::from_css_str("radial-gradient(farthest-corner at 25% 70%, #ffffff, #000000)");

    assert_eq!(
      gradient,
      Ok(
        RadialGradient::builder()
          .center(PositionValue(SpacePair::from_pair(
            Length::Percentage(25.0).into(),
            Length::Percentage(70.0).into(),
          )))
          .stops([
            GradientStop::ColorHint {
              color: Color::white().into(),
              hint: None,
            },
            GradientStop::ColorHint {
              color: Color::black().into(),
              hint: None,
            },
          ])
          .build()
      )
    );
  }

  #[test]
  fn test_parse_radial_gradient_circle_farthest_side_with_stops() {
    let gradient = RadialGradient::from_css_str(
      "radial-gradient(circle at 25px 25px, lightgray 2%, transparent 0%)",
    );

    assert_eq!(
      gradient,
      Ok(
        RadialGradient::builder()
          .shape(RadialShape::Circle)
          .center(PositionValue(SpacePair::from_single(
            PositionComponent::Length(Length::Px(25.0),)
          )))
          .stops([
            GradientStop::ColorHint {
              color: Color([211, 211, 211, 255]).into(),
              hint: Some(StopPosition(Length::Percentage(2.0))),
            },
            GradientStop::ColorHint {
              color: Color::transparent().into(),
              hint: Some(StopPosition(Length::Percentage(0.0))),
            },
          ])
          .build()
      )
    );
  }

  #[test]
  fn test_parse_radial_gradient_circle_stops() {
    for (input, stops) in [
      (
        "radial-gradient(circle, #ff0000 0%, #00ff00 50%, #0000ff 100%)",
        vec![
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
        ],
      ),
      (
        "radial-gradient(circle, red 10% 20%, blue)",
        vec![
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
        ],
      ),
    ] {
      assert_eq!(
        RadialGradient::from_css_str(input),
        Ok(
          RadialGradient::builder()
            .shape(RadialShape::Circle)
            .stops(stops)
            .build()
        ),
        "input: {input}",
      );
    }
  }

  #[test]
  fn test_parse_radial_gradient_with_explicit_ellipse_radii() {
    let gradient = RadialGradient::from_css_str(
      "radial-gradient(ellipse 60% 60% at 50% 50%, rgba(255, 53, 53, 0.10) 0%, transparent 70%)",
    );

    assert!(match gradient {
      Ok(RadialGradient {
        shape: RadialShape::Ellipse,
        size:
          RadialSize::Explicit {
            radius_x: Length::Percentage(radius_x),
            radius_y: Length::Percentage(radius_y),
          },
        center:
          PositionValue(SpacePair {
            x: PositionComponent::Length(Length::Percentage(center_x)),
            y: PositionComponent::Length(Length::Percentage(center_y)),
          }),
        stops,
        ..
      }) => {
        (radius_x - 60.0).abs() < 1e-3
          && (radius_y - 60.0).abs() < 1e-3
          && (center_x - 50.0).abs() < 1e-3
          && (center_y - 50.0).abs() < 1e-3
          && stops.len() == 2
      }
      _ => false,
    });
  }

  #[test]
  fn resolve_stops_percentage_and_px_radial() {
    let gradient = RadialGradient::builder()
      .stops([
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
      ])
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((200, 100)))
      .build();
    let resolved = ResolvedGradientStop::resolve(
      &gradient.stops,
      sizing.viewport.size.width.unwrap_or_default() as f32,
      &sizing,
      Color::black(),
    );

    assert_eq!(resolved.len(), 3);
    assert!((resolved[0].position - 0.0).abs() < 1e-3);
    assert_eq!(resolved[1].position, resolved[2].position);
  }

  #[test]
  fn resolve_stops_equal_positions_distributed_radial() {
    let gradient = RadialGradient::builder()
      .stops([
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
      ])
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((200, 100)))
      .build();
    let resolved = ResolvedGradientStop::resolve(
      &gradient.stops,
      sizing.viewport.size.width.unwrap_or_default() as f32,
      &sizing,
      Color::black(),
    );

    assert_eq!(resolved.len(), 3);
    assert!(resolved[0].position >= 0.0);
    assert!(resolved[1].position >= resolved[0].position);
    assert!(resolved[2].position >= resolved[1].position);
  }

  #[test]
  fn test_radial_gradient_at() {
    let gradient = RadialGradient::builder()
      .shape(RadialShape::Circle)
      .stops(red_blue_stops(
        Some(StopPosition(Length::Percentage(0.0))),
        Some(StopPosition(Length::Percentage(100.0))),
      ))
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let tile = RadialGradientTile::new(&gradient, 100, 100, &sizing, Color::black(), false);

    // Center (50, 50) should be red
    let color_center = tile.sample_pixel(50, 50).demultiply();
    assert_eq!(color_center, ColorU8::from_rgba(255, 0, 0, 255));

    // Far outside (200, 200) should be clamped to blue
    let color_far = tile.sample_pixel(200, 200).demultiply();
    assert_eq!(color_far, ColorU8::from_rgba(0, 0, 255, 255));
  }

  #[test]
  fn test_repeating_radial_gradient_rings() {
    let gradient = RadialGradient::builder()
      .repeating(true)
      .shape(RadialShape::Circle)
      .size(RadialSize::Explicit {
        radius_x: Length::Px(20.0),
        radius_y: Length::Px(20.0),
      })
      .stops([
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Px(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Px(5.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Px(5.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Px(10.0))),
        },
      ])
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((40, 40)))
      .build();
    let tile = RadialGradientTile::new(&gradient, 40, 40, &sizing, Color::black(), false);

    assert_eq!(
      [
        tile.sample_pixel(22, 20).demultiply(),
        tile.sample_pixel(27, 20).demultiply(),
        tile.sample_pixel(32, 20).demultiply(),
        tile.sample_pixel(37, 20).demultiply(),
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
  fn test_radial_gradient_ellipse_closest_corner() {
    let gradient = RadialGradient::builder()
      .center(PositionValue(SpacePair::from_pair(
        Length::Px(20.0).into(),
        Length::Px(20.0).into(),
      )))
      .size(RadialSize::ClosestCorner)
      .stops([
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::white().into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ])
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let tile = RadialGradientTile::new(&gradient, 100, 100, &sizing, Color::black(), false);

    // Closest corner (20, 20) with the closest-side aspect ratio 20/20 = 1:
    // the ellipse through the corner is the circle of radius 20 * sqrt(2).
    let expected = 20.0 * 2.0_f32.sqrt();
    assert!((tile.inv_radius_x - expected.recip()).abs() < 1e-3);
    assert!((tile.inv_radius_y - expected.recip()).abs() < 1e-3);
  }

  #[test]
  fn closest_corner_with_center_outside_the_box_keeps_positive_radii() {
    let gradient = RadialGradient::builder()
      .size(RadialSize::ClosestCorner)
      .center(PositionValue(SpacePair::from_pair(
        Length::Percentage(-50.0).into(),
        Length::Percentage(50.0).into(),
      )))
      .stops([
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::white().into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ])
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let tile = RadialGradientTile::new(&gradient, 100, 100, &sizing, Color::black(), false);

    // Center (-50, 50): closest corner offset (50, 50), side aspect 50/50 = 1
    // (Blink RadiusToSide takes absolute distances): rx = ry = 50 * sqrt(2).
    let r = 50.0 * 2.0_f32.sqrt();
    assert!((tile.inv_radius_x - r.recip()).abs() < 1e-6);
    assert!((tile.inv_radius_y - r.recip()).abs() < 1e-6);
  }

  #[test]
  fn test_radial_gradient_ellipse_farthest_corner_passes_through_corner() {
    let gradient = RadialGradient::builder()
      .center(PositionValue(SpacePair::from_pair(
        Length::Percentage(50.0).into(),
        Length::Percentage(0.0).into(),
      )))
      .stops([
        GradientStop::ColorHint {
          color: Color::black().into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color::white().into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ])
      .build();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((1200, 630)))
      .build();
    let tile = RadialGradientTile::new(&gradient, 1200, 630, &sizing, Color::black(), false);

    // Farthest corner offset (600, 630), farthest-side aspect 600/630
    // (Blink `EllipseRadius`): rx = sqrt(600^2 + 630^2 * (600/630)^2) = 600 * sqrt(2).
    let rx = 600.0 * 2.0_f32.sqrt();
    let ry = rx * 630.0 / 600.0;
    assert!((tile.inv_radius_x - rx.recip()).abs() < 1e-6);
    assert!((tile.inv_radius_y - ry.recip()).abs() < 1e-6);

    // The corner sits on the ellipse: normalized distance 1.
    let dx = 600.0 * tile.inv_radius_x;
    let dy = 630.0 * tile.inv_radius_y;
    assert!(((dx * dx + dy * dy).sqrt() - 1.0_f32).abs() < 1e-3);
  }
}
