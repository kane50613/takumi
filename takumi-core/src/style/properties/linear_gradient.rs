use std::{
  fmt,
  ops::{Deref, Neg},
};

use cssparser::{Parser, Token, match_ignore_ascii_case};
use tiny_skia::PremultipliedColorU8;
use typed_builder::TypedBuilder;

use super::gradient_utils::{
  GradientOverlayTile, adaptive_lut_size, adaptive_lut_size_with_visible_samples,
  build_color_lut_with_interpolation, compute_repeat_setup, gradient_tile_accessors,
  parse_gradient_stops, resolve_stops_along_axis, write_gradient_css,
};
use crate::style::{
  Animatable, Color, ColorInterpolationMethod, CssDescriptorKind, CssSyntaxKind, CssToken, FromCss,
  Length, MakeComputed, ParseResult, SizingContext, ToCss, declare_enum_from_css_impl,
  properties::ColorInput, tw::TailwindPropertyParser, unexpected_token,
};

/// Represents a linear gradient.
#[derive(Debug, Clone, PartialEq, TypedBuilder)]
#[non_exhaustive]
pub struct LinearGradient {
  /// Whether the gradient repeats beyond the last stop.
  #[builder(default)]
  pub repeating: bool,
  /// The gradient direction.
  #[builder(default)]
  pub direction: LinearGradientDirection,
  /// The color interpolation method used between stops.
  #[builder(default = ColorInterpolationMethod::LEGACY)]
  pub interpolation: ColorInterpolationMethod,
  /// The steps of the gradient.
  #[builder(setter(into))]
  pub stops: Box<[GradientStop]>,
}

impl MakeComputed for LinearGradient {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.stops.make_computed(sizing);
  }
}

/// Precomputed drawing context for repeated sampling of a `LinearGradient`.
#[derive(Debug, Clone)]
pub struct LinearGradientTile {
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
  /// Whether this gradient repeats.
  pub repeating: bool,
  /// First resolved stop position in pixels, used as repeating origin.
  pub repeat_start: f32,
  /// Repeat period in pixels.
  pub repeat_period: f32,
  /// Projection bias for `x * dir_x + y * dir_y + projection_bias`.
  pub projection_bias: f32,
  /// Scale converting axis-space position in pixels into LUT index space.
  pub position_to_lut_scale: f32,
  /// Whether every sampled pixel in this gradient is fully opaque.
  pub fully_opaque: bool,
  /// Pre-computed color lookup table for fast gradient sampling.
  /// Maps normalized position [0.0, 1.0] to color.
  pub color_lut: Vec<PremultipliedColorU8>,
  /// Precomputed axis samples for fast horizontal/vertical fills.
  pub axis_aligned_fast_path: Option<LinearGradientFastPathData>,
}

/// Per-row sampling state for incremental projection stepping.
#[derive(Debug, Clone, Copy)]
pub struct LinearGradientRowState {
  projection: f32,
  projection_step: f32,
  max_lut_index: usize,
}

/// Axis-aligned fast-path orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearGradientFastPathKind {
  /// Left-to-right gradient.
  Horizontal,
  /// Top-to-bottom gradient.
  Vertical,
}

/// Owned axis samples for an axis-aligned gradient.
#[derive(Debug, Clone)]
pub struct LinearGradientFastPathData {
  /// Fast-path orientation.
  pub kind: LinearGradientFastPathKind,
  /// One sample per pixel along the axis.
  pub axis_samples: Box<[PremultipliedColorU8]>,
}

/// Borrowed view of axis samples for an axis-aligned gradient.
#[derive(Debug, Clone, Copy)]
pub struct LinearGradientFastPath<'a> {
  /// Fast-path orientation.
  pub kind: LinearGradientFastPathKind,
  /// One sample per pixel along the axis.
  pub axis_samples: &'a [PremultipliedColorU8],
  /// Whether every sample is fully opaque.
  pub fully_opaque: bool,
}

impl LinearGradientTile {
  const AXIS_ALIGNMENT_EPSILON: f32 = 1e-4;

  fn direction_components(gradient: &LinearGradient, width: u32, height: u32) -> (f32, f32) {
    match gradient.direction {
      LinearGradientDirection::Angle(angle) => {
        let rad = angle.0.to_radians();
        (rad.sin(), -rad.cos())
      }
      LinearGradientDirection::Keyword(keyword_direction) => {
        if let (Some(horizontal), Some(vertical)) =
          (keyword_direction.horizontal, keyword_direction.vertical)
        {
          let dir_x = match horizontal {
            HorizontalKeyword::Left => -(height as f32),
            HorizontalKeyword::Right => height as f32,
          };
          let dir_y = match vertical {
            VerticalKeyword::Top => -(width as f32),
            VerticalKeyword::Bottom => width as f32,
          };
          let magnitude = dir_x.hypot(dir_y);
          if magnitude > f32::EPSILON {
            return (dir_x / magnitude, dir_y / magnitude);
          }
        }

        let angle = keyword_direction.to_angle();
        let rad = angle.0.to_radians();
        (rad.sin(), -rad.cos())
      }
    }
  }

  /// Projects a pixel onto the gradient axis, in pixels.
  #[inline(always)]
  pub(crate) fn projection_at(&self, x: f32, y: f32) -> f32 {
    x * self.dir_x + y * self.dir_y + self.projection_bias
  }

  /// Maps an axis projection to a LUT index for a LUT of `lut_len`.
  #[inline(always)]
  pub(crate) fn lut_index_for_projection_with_len(&self, projection: f32, lut_len: usize) -> usize {
    if lut_len <= 1 {
      return 0;
    }

    let position_px = projection.clamp(0.0, self.axis_length);
    ((position_px * self.position_to_lut_scale).round() as usize).min(lut_len - 1)
  }

  fn classify_axis_aligned(dir_x: f32, dir_y: f32) -> Option<LinearGradientFastPathKind> {
    if dir_y.abs() <= Self::AXIS_ALIGNMENT_EPSILON
      && (dir_x.abs() - 1.0).abs() <= Self::AXIS_ALIGNMENT_EPSILON
    {
      return Some(LinearGradientFastPathKind::Horizontal);
    }

    if dir_x.abs() <= Self::AXIS_ALIGNMENT_EPSILON
      && (dir_y.abs() - 1.0).abs() <= Self::AXIS_ALIGNMENT_EPSILON
    {
      return Some(LinearGradientFastPathKind::Vertical);
    }

    None
  }

  fn build_axis_samples(&self, kind: LinearGradientFastPathKind) -> Box<[PremultipliedColorU8]> {
    match kind {
      LinearGradientFastPathKind::Horizontal => {
        (0..self.width).map(|x| self.sample_pixel(x, 0)).collect()
      }
      LinearGradientFastPathKind::Vertical => {
        (0..self.height).map(|y| self.sample_pixel(0, y)).collect()
      }
    }
  }

  /// Borrowed axis-aligned fast path, if this gradient has one.
  pub fn fast_path(&self) -> Option<LinearGradientFastPath<'_>> {
    let fast_path = self.axis_aligned_fast_path.as_ref()?;
    Some(LinearGradientFastPath {
      kind: fast_path.kind,
      axis_samples: &fast_path.axis_samples,
      fully_opaque: self.fully_opaque,
    })
  }

  /// Builds a drawing context from a gradient and a target viewport.
  pub fn new(
    gradient: &LinearGradient,
    width: u32,
    height: u32,
    sizing: &SizingContext,
    current_color: Color,
  ) -> Self {
    let (dir_x, dir_y) = Self::direction_components(gradient, width, height);
    let axis_aligned_kind = Self::classify_axis_aligned(dir_x, dir_y);

    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let max_extent = ((width as f32 * dir_x.abs()) + (height as f32 * dir_y.abs())) / 2.0;
    let axis_length = 2.0 * max_extent;
    let projection_bias = max_extent - cx * dir_x - cy * dir_y;

    let resolved_stops = resolve_stops_along_axis(
      &gradient.stops,
      axis_length.max(1e-6),
      sizing,
      current_color,
    );

    let (repeating, repeat_start, repeat_period, lut_axis_length, lut_resolved_stops) =
      compute_repeat_setup(gradient.repeating, resolved_stops, axis_length);

    let visible_lut_samples = match axis_aligned_kind {
      Some(LinearGradientFastPathKind::Horizontal) => width as usize + 1,
      Some(LinearGradientFastPathKind::Vertical) => height as usize + 1,
      None => (lut_axis_length.ceil() as usize).saturating_add(1),
    };
    let lut_size = if axis_aligned_kind.is_some() {
      adaptive_lut_size_with_visible_samples(
        visible_lut_samples,
        lut_axis_length,
        &lut_resolved_stops,
      )
    } else {
      adaptive_lut_size(lut_axis_length, &lut_resolved_stops)
    };
    let color_lut = build_color_lut_with_interpolation(
      &lut_resolved_stops,
      lut_axis_length,
      lut_size,
      gradient.interpolation,
    );
    let lut_len = color_lut.len();
    let position_to_lut_scale = if lut_axis_length.abs() <= f32::EPSILON || lut_len <= 1 {
      0.0
    } else {
      (lut_len - 1) as f32 / lut_axis_length
    };
    let fully_opaque = lut_resolved_stops
      .iter()
      .all(|stop| stop.color.0[3] == u8::MAX);

    let mut tile = LinearGradientTile {
      width,
      height,
      dir_x,
      dir_y,
      axis_length,
      repeating,
      repeat_start,
      repeat_period,
      projection_bias,
      position_to_lut_scale,
      fully_opaque,
      color_lut,
      axis_aligned_fast_path: None,
    };

    if !tile.repeating
      && let Some(kind) = axis_aligned_kind
    {
      tile.axis_aligned_fast_path = Some(LinearGradientFastPathData {
        kind,
        axis_samples: tile.build_axis_samples(kind),
      });
    }

    tile
  }
}

impl GradientOverlayTile for LinearGradientTile {
  type RowState = LinearGradientRowState;

  gradient_tile_accessors!();

  #[inline(always)]
  fn sample_pixel(&self, x: u32, y: u32) -> PremultipliedColorU8 {
    if self.color_lut.is_empty() {
      return PremultipliedColorU8::TRANSPARENT;
    }

    if self.color_lut.len() == 1 {
      return self.color_lut[0];
    }

    let projection = self.projection_at(x as f32, y as f32);
    let lut_idx = if self.repeating && self.repeat_period > 1e-6 {
      let wrapped = (projection - self.repeat_start).rem_euclid(self.repeat_period);
      ((wrapped * self.position_to_lut_scale).round() as usize).min(self.color_lut.len() - 1)
    } else {
      self.lut_index_for_projection_with_len(projection, self.color_lut.len())
    };

    self.color_lut[lut_idx]
  }

  #[inline(always)]
  fn begin_row(&self, src_x_start: u32, src_y: u32, lut_len: usize) -> Self::RowState {
    let projection = self.projection_at(src_x_start as f32, src_y as f32);
    LinearGradientRowState {
      projection,
      projection_step: self.dir_x,
      max_lut_index: lut_len.saturating_sub(1),
    }
  }

  #[inline(always)]
  fn next_lut_index(&self, row_state: &mut Self::RowState) -> usize {
    let lut_idx = if self.repeating && self.repeat_period > 1e-6 {
      let wrapped = (row_state.projection - self.repeat_start).rem_euclid(self.repeat_period);
      ((wrapped * self.position_to_lut_scale).round() as usize).min(row_state.max_lut_index)
    } else {
      let position_px = row_state.projection.clamp(0.0, self.axis_length);
      ((position_px * self.position_to_lut_scale).round() as usize).min(row_state.max_lut_index)
    };

    row_state.projection += row_state.projection_step;
    lut_idx
  }
}

/// Represents a gradient stop position.
/// If a percentage or number (0.0-1.0) is provided, it is treated as a percentage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopPosition(pub Length);

impl MakeComputed for StopPosition {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.0.make_computed(sizing);
  }
}

/// Represents a gradient stop.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
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
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      GradientStop::ColorHint { hint, .. } => hint.make_computed(sizing),
      GradientStop::Hint(hint) => hint.make_computed(sizing),
    }
  }
}

/// Represents a resolved gradient stop with a position.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
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
      return Err(unexpected_token!(
        input.current_source_location(),
        input.next()?,
      ));
    };

    Ok(StopPosition(length))
  }

  const VALID_TOKENS: &'static [CssToken] = Length::VALID_TOKENS;
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

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Color),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

/// Represents an angle value in degrees.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Angle(f32);

impl MakeComputed for Angle {}

impl ToCss for Angle {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}deg", **self)
  }
}

impl Animatable for Angle {
  fn missing_value() -> Option<Self> {
    Some(Angle::zero())
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    let from_degrees = **from;
    let to_degrees = **to;
    let delta = (to_degrees - from_degrees + 180.0).rem_euclid(360.0) - 180.0;
    *self = Angle::new(from_degrees + delta * progress);
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
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum HorizontalKeyword {
  /// The left keyword.
  Left,
  /// The right keyword.
  Right,
}

/// Represents a vertical keyword.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
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

/// The original `to <side-or-corner>` direction parsed from CSS.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientKeywordDirection {
  /// Horizontal keyword, if one was provided.
  pub horizontal: Option<HorizontalKeyword>,
  /// Vertical keyword, if one was provided.
  pub vertical: Option<VerticalKeyword>,
}

/// The direction of a linear gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinearGradientDirection {
  /// An explicit numeric angle.
  Angle(Angle),
  /// A box-relative side-or-corner direction.
  Keyword(GradientKeywordDirection),
}

impl Default for LinearGradientDirection {
  fn default() -> Self {
    Self::Angle(Angle::new(180.0))
  }
}

impl HorizontalKeyword {
  /// Returns the angle in degrees.
  pub(crate) fn degrees(&self) -> f32 {
    match self {
      HorizontalKeyword::Left => 270.0, // "to left" = 270deg
      HorizontalKeyword::Right => 90.0, // "to right" = 90deg
    }
  }

  /// Returns the mixed angle in degrees.
  pub(crate) fn vertical_mixed_degrees(&self) -> f32 {
    match self {
      HorizontalKeyword::Left => -45.0, // For diagonals with left
      HorizontalKeyword::Right => 45.0, // For diagonals with right
    }
  }
}

impl VerticalKeyword {
  /// Returns the angle in degrees.
  pub(crate) fn degrees(&self) -> f32 {
    match self {
      VerticalKeyword::Top => 0.0,
      VerticalKeyword::Bottom => 180.0,
    }
  }
}

impl GradientKeywordDirection {
  /// Converts a side-or-corner direction into the matching CSS angle.
  pub(crate) fn to_angle(self) -> Angle {
    Angle::degrees_from_keywords(self.horizontal, self.vertical)
  }
}

impl<'i> FromCss<'i> for GradientKeywordDirection {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    input.expect_ident_matching("to")?;

    if let Ok(vertical) = input.try_parse(VerticalKeyword::from_css) {
      if let Ok(horizontal) = input.try_parse(HorizontalKeyword::from_css) {
        return Ok(Self {
          horizontal: Some(horizontal),
          vertical: Some(vertical),
        });
      }

      return Ok(Self {
        horizontal: None,
        vertical: Some(vertical),
      });
    }

    if let Ok(horizontal) = input.try_parse(HorizontalKeyword::from_css) {
      return Ok(Self {
        horizontal: Some(horizontal),
        vertical: None,
      });
    }

    Err(input.new_error_for_next_token())
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("to"),
    CssToken::Keyword("top"),
    CssToken::Keyword("bottom"),
    CssToken::Keyword("left"),
    CssToken::Keyword("right"),
  ];
}

impl<'i> FromCss<'i> for LinearGradientDirection {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(direction) = input.try_parse(GradientKeywordDirection::from_css) {
      return Ok(Self::Keyword(direction));
    }

    Angle::from_css(input).map(Self::Angle)
  }

  const VALID_TOKENS: &'static [CssToken] = Angle::VALID_TOKENS;
}

impl<'i> FromCss<'i> for LinearGradient {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, LinearGradient> {
    let location = input.current_source_location();
    let name = input.expect_function()?;
    let repeating = match_ignore_ascii_case! { &name,
      "linear-gradient" => false,
      "repeating-linear-gradient" => true,
      _ => return Err(unexpected_token!(location, &Token::Function(name.clone()))),
    };

    input.parse_nested_block(|input| {
      let mut direction = LinearGradientDirection::default();
      let mut interpolation = None;
      let mut saw_direction = false;

      loop {
        if let Ok(parsed_direction) = input.try_parse(LinearGradientDirection::from_css) {
          if saw_direction {
            return Err(input.new_error_for_next_token());
          }

          direction = parsed_direction;
          saw_direction = true;
          continue;
        }

        if let Ok(parsed_interpolation) = input.try_parse(ColorInterpolationMethod::from_css) {
          interpolation = Some(parsed_interpolation);
          continue;
        }

        break;
      }

      input.try_parse(Parser::expect_comma).ok();

      let (stops, modern) = parse_gradient_stops(input, StopPosition::from_css)?;

      Ok(LinearGradient {
        repeating,
        direction,
        interpolation: interpolation.unwrap_or(ColorInterpolationMethod::gradient_default(modern)),
        stops: stops.into_boxed_slice(),
      })
    })
  }

  const VALID_TOKENS: &'static [CssToken] =
    &[CssToken::Descriptor(CssDescriptorKind::LinearGradientFn)];
}

impl Angle {
  /// Calculates the angle from horizontal and vertical keywords.
  pub(crate) fn degrees_from_keywords(
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

    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Number { value, .. } => Ok(Angle::new(*value)),
      Token::Dimension { value, unit, .. } => match unit.as_ref() {
        "deg" => Ok(Angle::new(*value)),
        "grad" => Ok(Angle::new(*value / 400.0 * 360.0)),
        "turn" => Ok(Angle::new(*value * 360.0)),
        "rad" => Ok(Angle::new(value.to_degrees())),
        _ => Err(unexpected_token!(location, token)),
      },
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Angle),
    CssToken::Keyword("none"),
  ];
}

impl ToCss for StopPosition {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.0.to_css(dest)
  }
}

impl ToCss for GradientStop {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::ColorHint { color, hint } => {
        color.to_css(dest)?;
        if let Some(h) = hint {
          dest.write_char(' ')?;
          h.to_css(dest)?;
        }
        Ok(())
      }
      Self::Hint(h) => h.to_css(dest),
    }
  }
}

impl ToCss for GradientKeywordDirection {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str("to")?;
    if let Some(v) = self.vertical {
      dest.write_char(' ')?;
      v.to_css(dest)?;
    }
    if let Some(h) = self.horizontal {
      dest.write_char(' ')?;
      h.to_css(dest)?;
    }
    Ok(())
  }
}

impl ToCss for LinearGradientDirection {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Angle(a) => a.to_css(dest),
      Self::Keyword(kw) => kw.to_css(dest),
    }
  }
}

impl ToCss for LinearGradient {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let name = if self.repeating {
      "repeating-linear-gradient"
    } else {
      "linear-gradient"
    };

    let mut dir_buf = String::new();
    self.direction.to_css(&mut dir_buf)?;
    if dir_buf == "180deg" || dir_buf == "to bottom" {
      dir_buf.clear();
    }

    write_gradient_css(dest, name, &dir_buf, &self.interpolation, &self.stops)
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use color::{ColorSpaceTag, HueDirection};
  use tiny_skia::ColorU8;

  use super::*;
  use crate::{
    geometry::Size,
    style::{CalcArena, FromCssStr, properties::gradient_utils::red_blue_stops},
    viewport::Viewport,
  };
  fn sizing() -> SizingContext {
    SizingContext {
      viewport: Viewport::new((200, 100)),
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: 16.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  #[test]
  fn test_parse_linear_gradient() {
    assert_eq!(
      LinearGradient::from_css_str("linear-gradient(to top right, #ff0000, #0000ff)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::Keyword(GradientKeywordDirection {
          horizontal: Some(HorizontalKeyword::Right),
          vertical: Some(VerticalKeyword::Top),
        }),
        interpolation: ColorInterpolationMethod::LEGACY,
        stops: red_blue_stops(None, None).into(),
      })
    )
  }

  #[test]
  fn test_parse_angle() {
    for (input, expected) in [
      ("45deg", Angle::new(45.0)),
      ("200grad", Angle::new(180.0)),
      ("0.5turn", Angle::new(180.0)),
      ("90", Angle::new(90.0)),
    ] {
      assert_eq!(Angle::from_css_str(input), Ok(expected), "input: {input}");
    }
    // rad requires approximate comparison
    assert!(Angle::from_css_str("3.14159rad").is_ok_and(|a| (a.0 - 180.0).abs() < 0.001));
  }

  #[test]
  fn test_parse_direction_keywords() {
    use HorizontalKeyword::{Left, Right};
    use VerticalKeyword::{Bottom, Top};
    for (input, expected) in [
      (
        "to top",
        GradientKeywordDirection {
          horizontal: None,
          vertical: Some(Top),
        },
      ),
      (
        "to right",
        GradientKeywordDirection {
          horizontal: Some(Right),
          vertical: None,
        },
      ),
      (
        "to bottom",
        GradientKeywordDirection {
          horizontal: None,
          vertical: Some(Bottom),
        },
      ),
      (
        "to left",
        GradientKeywordDirection {
          horizontal: Some(Left),
          vertical: None,
        },
      ),
      (
        "to top right",
        GradientKeywordDirection {
          horizontal: Some(Right),
          vertical: Some(Top),
        },
      ),
      (
        "to bottom left",
        GradientKeywordDirection {
          horizontal: Some(Left),
          vertical: Some(Bottom),
        },
      ),
      (
        "to top left",
        GradientKeywordDirection {
          horizontal: Some(Left),
          vertical: Some(Top),
        },
      ),
      (
        "to bottom right",
        GradientKeywordDirection {
          horizontal: Some(Right),
          vertical: Some(Bottom),
        },
      ),
    ] {
      assert_eq!(
        GradientKeywordDirection::from_css_str(input),
        Ok(expected),
        "input: {input}"
      );
    }
  }

  #[test]
  fn test_angle_interpolate_uses_shortest_path_across_zero() {
    let from = Angle::new(0.0);
    let to = Angle::new(-3.0);
    let mut interpolated = from;

    interpolated.interpolate(&from, &to, 0.5, &sizing(), Color::transparent());

    assert!((*interpolated - 358.5).abs() < 0.001);
  }

  #[test]
  fn test_angle_interpolate_uses_shortest_path_forward_across_zero() {
    let from = Angle::new(-3.0);
    let to = Angle::new(0.0);
    let mut interpolated = from;

    interpolated.interpolate(&from, &to, 0.5, &sizing(), Color::transparent());

    assert!((*interpolated - 358.5).abs() < 0.001);
  }

  #[test]
  fn test_parse_linear_gradient_with_angle() {
    assert_eq!(
      LinearGradient::from_css_str("linear-gradient(45deg, #ff0000, #0000ff)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::Angle(Angle::new(45.0)),
        interpolation: ColorInterpolationMethod::LEGACY,
        stops: red_blue_stops(None, None).into(),
      })
    )
  }

  #[test]
  fn test_parse_linear_gradient_with_interpolation_color_space() {
    assert_eq!(
      LinearGradient::from_css_str("linear-gradient(in oklab, #ff0000, #0000ff)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::default(),
        interpolation: ColorInterpolationMethod {
          color_space: ColorSpaceTag::Oklab,
          hue_direction: HueDirection::Shorter,
        },
        stops: red_blue_stops(None, None).into(),
      })
    );
  }

  #[test]
  fn relative_color_stops_interpolate_in_oklab() {
    let gradient =
      LinearGradient::from_css_str("linear-gradient(rgb(from red r g b), #0000ff)").unwrap();

    assert_eq!(gradient.interpolation.color_space, ColorSpaceTag::Oklab);
  }

  #[test]
  fn interpolation_color_space_round_trips() {
    // The serialized form must stay valid CSS (direction and color space in one
    // clause), so re-parsing keeps the interpolation instead of dropping it.
    let gradient =
      LinearGradient::from_css_str("linear-gradient(to right in oklab, #ff0000, #0000ff)").unwrap();
    let reparsed = LinearGradient::from_css_str(&gradient.to_css_string()).unwrap();
    assert_eq!(gradient, reparsed);
    assert_eq!(reparsed.interpolation.color_space, ColorSpaceTag::Oklab);
  }

  #[test]
  fn test_parse_linear_gradient_with_interpolation_hue_direction() {
    assert_eq!(
      LinearGradient::from_css_str("linear-gradient(to right in oklch longer hue, red, blue)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::Keyword(GradientKeywordDirection {
          horizontal: Some(HorizontalKeyword::Right),
          vertical: None,
        }),
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
  fn test_parse_linear_gradient_rejects_multiple_directions() {
    assert!(LinearGradient::from_css_str("linear-gradient(to right 45deg, red, blue)").is_err());
    assert!(LinearGradient::from_css_str("linear-gradient(45deg to right, red, blue)").is_err());
  }

  #[test]
  fn test_parse_linear_gradient_with_stops() {
    assert_eq!(
      LinearGradient::from_css_str("linear-gradient(to right, #ff0000 0%, #0000ff 100%)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::Keyword(GradientKeywordDirection {
          horizontal: Some(HorizontalKeyword::Right),
          vertical: None,
        }),
        interpolation: ColorInterpolationMethod::LEGACY,
        stops: red_blue_stops(
          Some(StopPosition(Length::Percentage(0.0))),
          Some(StopPosition(Length::Percentage(100.0))),
        )
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_linear_gradient_with_double_position_color_stop() {
    assert_eq!(
      LinearGradient::from_css_str("linear-gradient(to right, red 10% 20%, blue)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::Keyword(GradientKeywordDirection {
          horizontal: Some(HorizontalKeyword::Right),
          vertical: None,
        }),
        interpolation: ColorInterpolationMethod::LEGACY,
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
      LinearGradient::from_css_str("linear-gradient(to right, #ff0000, 50%, #0000ff)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::Keyword(GradientKeywordDirection {
          horizontal: Some(HorizontalKeyword::Right),
          vertical: None,
        }),
        interpolation: ColorInterpolationMethod::LEGACY,
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
      LinearGradient::from_css_str("linear-gradient(to bottom, #ff0000)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::Keyword(GradientKeywordDirection {
          horizontal: None,
          vertical: Some(VerticalKeyword::Bottom),
        }),
        interpolation: ColorInterpolationMethod::LEGACY,
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
      LinearGradient::from_css_str("linear-gradient(#ff0000, #0000ff)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::default(),
        interpolation: ColorInterpolationMethod::LEGACY,
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
      GradientStop::from_css_str("#ff0000"),
      Ok(GradientStop::ColorHint {
        color: ColorInput::Value(Color([255, 0, 0, 255])),
        hint: None,
      })
    );
  }

  #[test]
  fn test_parse_gradient_hint_numeric() {
    assert_eq!(
      GradientStop::from_css_str("50%"),
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
      LinearGradient::from_css_str("linear-gradient(45deg, #ff0000, 25%, #00ff00, 75%, #0000ff)"),
      Ok(LinearGradient {
        repeating: false,
        direction: LinearGradientDirection::Angle(Angle::new(45.0)),
        interpolation: ColorInterpolationMethod::LEGACY,
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
      repeating: false,
      direction: LinearGradientDirection::default(),
      interpolation: ColorInterpolationMethod::LEGACY,
      stops: red_blue_stops(
        Some(StopPosition(Length::Percentage(0.0))),
        Some(StopPosition(Length::Percentage(100.0))),
      )
      .into(),
    };

    // Test at the top (should be red)
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let tile = LinearGradientTile::new(&gradient, 100, 100, &sizing, Color::black());

    let color_top = tile.sample_pixel(50, 0).demultiply();
    assert_eq!(color_top, ColorU8::from_rgba(255, 0, 0, 255));

    // Test at the bottom (should be blue)
    let color_bottom = tile.sample_pixel(50, 100).demultiply();
    assert_eq!(color_bottom, ColorU8::from_rgba(0, 0, 255, 255));

    // Test in the middle (should be purple)
    let color_middle = tile.sample_pixel(50, 50).demultiply();
    assert_eq!(color_middle, ColorU8::from_rgba(127, 0, 128, 255));
  }

  #[test]
  fn test_linear_gradient_at_horizontal() {
    let gradient = LinearGradient {
      repeating: false,
      direction: LinearGradientDirection::Angle(Angle::new(90.0)),
      interpolation: ColorInterpolationMethod::LEGACY,
      stops: red_blue_stops(
        Some(StopPosition(Length::Percentage(0.0))),
        Some(StopPosition(Length::Percentage(100.0))),
      )
      .into(),
    };

    // Test at the left (should be red)
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();

    let tile = LinearGradientTile::new(&gradient, 100, 100, &sizing, Color::black());
    let color_left = tile.sample_pixel(0, 50).demultiply();
    assert_eq!(color_left, ColorU8::from_rgba(255, 0, 0, 255));

    // Test at the right (should be blue)
    let color_right = tile.sample_pixel(100, 50).demultiply();
    assert_eq!(color_right, ColorU8::from_rgba(0, 0, 255, 255));
  }

  #[test]
  fn test_keyword_corner_direction_uses_aspect_ratio() {
    let gradient = LinearGradient {
      repeating: false,
      direction: LinearGradientDirection::Keyword(GradientKeywordDirection {
        horizontal: Some(HorizontalKeyword::Right),
        vertical: Some(VerticalKeyword::Bottom),
      }),
      interpolation: ColorInterpolationMethod::LEGACY,
      stops: red_blue_stops(
        Some(StopPosition(Length::Percentage(0.0))),
        Some(StopPosition(Length::Percentage(100.0))),
      )
      .into(),
    };

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((200, 100)))
      .build();
    let tile = LinearGradientTile::new(&gradient, 200, 100, &sizing, Color::black());

    assert!((tile.dir_x - 0.4472136).abs() < 0.001);
    assert!((tile.dir_y - 0.8944272).abs() < 0.001);
  }

  #[test]
  fn test_linear_gradient_at_single_color() {
    let gradient = LinearGradient {
      repeating: false,
      direction: LinearGradientDirection::Angle(Angle::new(0.0)),
      interpolation: ColorInterpolationMethod::LEGACY,
      stops: [GradientStop::ColorHint {
        color: Color([255, 0, 0, 255]).into(), // Red
        hint: None,
      }]
      .into(),
    };

    // Should always return the same color
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let tile = LinearGradientTile::new(&gradient, 100, 100, &sizing, Color::black());
    let color = tile.sample_pixel(50, 50).demultiply();
    assert_eq!(color, ColorU8::from_rgba(255, 0, 0, 255));
  }

  #[test]
  fn test_linear_gradient_at_no_steps() {
    let gradient = LinearGradient {
      repeating: false,
      direction: LinearGradientDirection::Angle(Angle::new(0.0)),
      interpolation: ColorInterpolationMethod::LEGACY,
      stops: [].into(),
    };

    // Should return transparent
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let tile = LinearGradientTile::new(&gradient, 100, 100, &sizing, Color::black());
    let color = tile.sample_pixel(50, 50).demultiply();
    assert_eq!(color, ColorU8::from_rgba(0, 0, 0, 0));
  }

  #[test]
  fn test_repeating_linear_gradient_stripes() {
    let gradient = LinearGradient::builder()
      .repeating(true)
      .direction(LinearGradientDirection::Angle(Angle::new(90.0)))
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
      .viewport(Viewport::new((40, 1)))
      .build();
    let tile = LinearGradientTile::new(&gradient, 40, 1, &sizing, Color::black());

    assert_eq!(
      [
        tile.sample_pixel(2, 0).demultiply(),
        tile.sample_pixel(7, 0).demultiply(),
        tile.sample_pixel(12, 0).demultiply(),
        tile.sample_pixel(17, 0).demultiply(),
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
  fn test_linear_gradient_px_stops_crisp_line() {
    let gradient =
      LinearGradient::from_css_str("linear-gradient(to right, grey 1px, transparent 1px)").unwrap();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((40, 40)))
      .build();
    let tile = LinearGradientTile::new(&gradient, 40, 40, &sizing, Color::black());

    // grey at 0,0
    let c0 = tile.sample_pixel(0, 0).demultiply();
    assert_eq!(c0, ColorU8::from_rgba(128, 128, 128, 255));

    // transparent at 1,0
    let c1 = tile.sample_pixel(1, 0).demultiply();
    assert_eq!(c1, ColorU8::from_rgba(0, 0, 0, 0));

    // transparent till the end
    let c2 = tile.sample_pixel(40, 0).demultiply();
    assert_eq!(c2, ColorU8::from_rgba(0, 0, 0, 0));
  }

  #[test]
  fn test_linear_gradient_vertical_px_stops_top_pixel() {
    let gradient =
      LinearGradient::from_css_str("linear-gradient(to bottom, grey 1px, transparent 1px)")
        .unwrap();

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((40, 40)))
      .build();
    let tile = LinearGradientTile::new(&gradient, 40, 40, &sizing, Color::black());

    // color at top-left (0, 0) should be grey (1px hard stop)
    assert_eq!(
      tile.sample_pixel(0, 0).demultiply(),
      ColorU8::from_rgba(128, 128, 128, 255)
    );
  }

  #[test]
  fn test_stop_position_parsing() {
    for (input, expected) in [
      ("0.25", StopPosition(Length::Percentage(25.0))),
      ("75%", StopPosition(Length::Percentage(75.0))),
      ("50%", StopPosition(Length::Percentage(50.0))),
      ("12px", StopPosition(Length::Px(12.0))),
      ("8px", StopPosition(Length::Px(8.0))),
    ] {
      assert_eq!(
        StopPosition::from_css_str(input),
        Ok(expected),
        "input: {input}"
      );
    }
  }

  #[test]
  fn resolve_stops_percentage_and_px_linear() {
    let gradient = LinearGradient::builder()
      .direction(LinearGradientDirection::Angle(Angle::new(0.0)))
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

    let resolved = resolve_stops_along_axis(
      &gradient.stops,
      sizing.viewport.size.width.unwrap_or_default() as f32,
      &sizing,
      Color::black(),
    );
    assert_eq!(resolved.len(), 3);
    assert!((resolved[0].position - 0.0).abs() < 1e-3);
    assert!((resolved[1].position - 100.0).abs() < 1e-3);
    assert!((resolved[2].position - 100.0).abs() < 1e-3);
  }

  #[test]
  fn resolve_stops_equal_positions_allowed_linear() {
    let gradient = LinearGradient::builder()
      .direction(LinearGradientDirection::Angle(Angle::new(0.0)))
      .stops([
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

    let resolved = resolve_stops_along_axis(
      &gradient.stops,
      sizing.viewport.size.width.unwrap_or_default() as f32,
      &sizing,
      Color::black(),
    );
    assert_eq!(resolved.len(), 2);
    assert!((resolved[0].position - 0.0).abs() < 1e-3);
    assert!((resolved[1].position - 0.0).abs() < 1e-3);
  }
}
