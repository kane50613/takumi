use cssparser::Parser;
use image::{GenericImageView, Rgba};
use smallvec::SmallVec;

use super::gradient_utils::{adaptive_lut_size, build_color_lut, resolve_stops_along_axis};
use crate::{
  layout::style::{
    BackgroundPosition, CssToken, FromCss, GradientStop, Length, ParseResult, ResolvedGradientStop,
    declare_enum_from_css_impl,
  },
  rendering::RenderContext,
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
  /// Gradient stops
  pub stops: Box<[GradientStop]>,
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
pub struct RadialGradientTile {
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
  /// Scale component used for stop resolution.
  pub radius_scale: f32,
  /// Resolved and ordered color stops.
  pub resolved_stops: SmallVec<[ResolvedGradientStop; 4]>,
  /// Pre-computed color lookup table for fast gradient sampling.
  /// Maps normalized distance [0.0, 1.0] from center to color.
  pub color_lut: Box<[Rgba<u8>]>,
}

impl GenericImageView for RadialGradientTile {
  type Pixel = Rgba<u8>;

  fn dimensions(&self) -> (u32, u32) {
    (self.width, self.height)
  }

  fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
    // Fast path for empty or single-color gradients
    if self.color_lut.is_empty() {
      return Rgba([0, 0, 0, 0]);
    }
    if self.color_lut.len() == 1 {
      return self.color_lut[0];
    }

    let dx = (x as f32 - self.cx) / self.radius_x.max(1e-6);
    let dy = (y as f32 - self.cy) / self.radius_y.max(1e-6);

    // Normalized distance from center (1.0 = at radius)
    let d = (dx * dx + dy * dy).sqrt();
    let normalized = d.clamp(0.0, 1.0);

    // Map distance to LUT index using rounding (nearest neighbor).
    let lut_idx = (normalized * (self.color_lut.len() - 1) as f32).round() as usize;

    self.color_lut[lut_idx]
  }
}

impl RadialGradientTile {
  /// Builds a drawing context from a gradient and a target viewport.
  pub fn new(gradient: &RadialGradient, width: u32, height: u32, context: &RenderContext) -> Self {
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
      // For corner sizes, use farthest-corner as sensible default
      (RadialShape::Ellipse, RadialSize::ClosestCorner) => {
        (dx_left.max(dx_right), dy_top.max(dy_bottom))
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
    let color_lut = build_color_lut(&resolved_stops, radius_scale, lut_size);

    RadialGradientTile {
      width,
      height,
      cx,
      cy,
      radius_x,
      radius_y,
      radius_scale,
      resolved_stops,
      color_lut,
    }
  }
}

impl<'i> FromCss<'i> for RadialGradient {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, RadialGradient> {
    input.expect_function_matching("radial-gradient")?;

    input.parse_nested_block(|input| {
      let mut shape = RadialShape::Ellipse;
      let mut size = RadialSize::FarthestCorner;
      let mut center = BackgroundPosition::default();

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

        input.try_parse(Parser::expect_comma).ok();

        break;
      }

      // Parse at least one stop, comma-separated
      let mut stops = Vec::new();

      stops.push(GradientStop::from_css(input)?);

      while input.try_parse(Parser::expect_comma).is_ok() {
        stops.push(GradientStop::from_css(input)?);
      }

      Ok(RadialGradient {
        shape,
        size,
        center,
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
  fn test_parse_radial_gradient_circle_farthest_side() {
    let gradient =
      RadialGradient::from_str("radial-gradient(circle farthest-side, #ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(RadialGradient {
        shape: RadialShape::Circle,
        size: RadialSize::FarthestSide,
        center: BackgroundPosition::default(),
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
          PositionComponent::Length(Length::Percentage(25.0)),
          PositionComponent::Length(Length::Percentage(70.0)),
        )),
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
  fn resolve_stops_percentage_and_px_radial() {
    let gradient = RadialGradient {
      shape: RadialShape::Ellipse,
      size: RadialSize::FarthestCorner,
      center: BackgroundPosition::default(),
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
    let render_context = RenderContext::new(&context, (200, 100).into(), Default::default());
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
    let render_context = RenderContext::new(&context, (200, 100).into(), Default::default());
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
    let dummy_context = RenderContext::new(&context, (100, 100).into(), Default::default());
    let tile = RadialGradientTile::new(&gradient, 100, 100, &dummy_context);

    // Center (50, 50) should be red
    let color_center = tile.get_pixel(50, 50);
    assert_eq!(color_center, Rgba([255, 0, 0, 255]));

    // Far outside (200, 200) should be clamped to blue
    let color_far = tile.get_pixel(200, 200);
    assert_eq!(color_far, Rgba([0, 0, 255, 255]));
  }
}
