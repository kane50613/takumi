use cssparser::{BasicParseErrorKind, Parser, Token, match_ignore_ascii_case};

use crate::layout::style::{
  CssToken, FromCss, MakeComputed, ParseResult, declare_enum_from_css_impl,
};

/// Represents a CSS animation time value stored in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AnimationTime {
  /// Milliseconds represented by this time value.
  pub milliseconds: f32,
}

impl AnimationTime {
  /// Creates a time value from milliseconds.
  pub const fn from_milliseconds(milliseconds: f32) -> Self {
    Self { milliseconds }
  }
}

impl MakeComputed for AnimationTime {}

impl<'i> FromCss<'i> for AnimationTime {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Dimension { value, unit, .. } => match_ignore_ascii_case! {unit.as_ref(),
        "ms" => Ok(Self::from_milliseconds(*value)),
        "s" => Ok(Self::from_milliseconds(*value * 1000.0)),
        _ => Err(Self::unexpected_token_error(location, token)),
      },
      Token::Number { value, .. } if *value == 0.0 => Ok(Self::from_milliseconds(0.0)),
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("time")]
  }
}

/// Parsed values for `animation-name`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationNames(pub Box<[String]>);

impl MakeComputed for AnimationNames {}

impl<'i> FromCss<'i> for AnimationNames {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|parser| parser.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Self::default());
    }

    let mut names = Vec::new();

    loop {
      let location = input.current_source_location();
      let token = input.next()?;
      let Token::Ident(name) = token else {
        return Err(Self::unexpected_token_error(location, token));
      };

      if name.eq_ignore_ascii_case("none") {
        return Err(Self::unexpected_token_error(location, token));
      }

      names.push(name.to_string());

      if input.try_parse(Parser::expect_comma).is_err() {
        break;
      }
    }

    Ok(Self(names.into_boxed_slice()))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Keyword("none"), CssToken::Token("custom-ident")]
  }
}

/// Parsed values for `animation-duration` and `animation-delay`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationDurations(pub Box<[AnimationTime]>);

impl MakeComputed for AnimationDurations {}

impl<'i> FromCss<'i> for AnimationDurations {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_separated(input, AnimationTime::from_css).map(Self)
  }

  fn valid_tokens() -> &'static [CssToken] {
    AnimationTime::valid_tokens()
  }
}

/// Supported CSS timing functions for animations.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AnimationTimingFunction {
  /// Uses linear interpolation.
  Linear,
  /// Uses the CSS `ease` curve.
  #[default]
  Ease,
  /// Uses the CSS `ease-in` curve.
  EaseIn,
  /// Uses the CSS `ease-out` curve.
  EaseOut,
  /// Uses the CSS `ease-in-out` curve.
  EaseInOut,
  /// Uses the CSS `step-start` timing function.
  StepStart,
  /// Uses the CSS `step-end` timing function.
  StepEnd,
  /// Uses a stepped timing function with an explicit position.
  Steps(u32, StepPosition),
  /// Uses a custom cubic bezier timing curve.
  CubicBezier(f32, f32, f32, f32),
}

impl MakeComputed for AnimationTimingFunction {}

/// Supported step positions for CSS stepped easing functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepPosition {
  /// Jumps at the start of each step interval.
  Start,
  /// Jumps at the end of each step interval.
  End,
}

impl<'i> FromCss<'i> for AnimationTimingFunction {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(function) = input.try_parse(parse_timing_keyword) {
      return Ok(function);
    }

    if let Ok(function) = input.try_parse(parse_steps_function) {
      return Ok(function);
    }

    input.expect_function_matching("cubic-bezier")?;
    input.parse_nested_block(|input| {
      let x1 = expect_number(input)?;
      input.expect_comma()?;
      let y1 = expect_number(input)?;
      input.expect_comma()?;
      let x2 = expect_number(input)?;
      input.expect_comma()?;
      let y2 = expect_number(input)?;

      if !(0.0..=1.0).contains(&x1) || !(0.0..=1.0).contains(&x2) {
        return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
      }

      Ok(Self::CubicBezier(x1, y1, x2, y2))
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("linear"),
      CssToken::Keyword("ease"),
      CssToken::Keyword("ease-in"),
      CssToken::Keyword("ease-out"),
      CssToken::Keyword("ease-in-out"),
      CssToken::Keyword("step-start"),
      CssToken::Keyword("step-end"),
      CssToken::Token("steps()"),
      CssToken::Token("cubic-bezier()"),
    ]
  }
}

/// Parsed values for `animation-timing-function`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationTimingFunctions(pub Box<[AnimationTimingFunction]>);

impl MakeComputed for AnimationTimingFunctions {}

impl<'i> FromCss<'i> for AnimationTimingFunctions {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_separated(input, AnimationTimingFunction::from_css).map(Self)
  }

  fn valid_tokens() -> &'static [CssToken] {
    AnimationTimingFunction::valid_tokens()
  }
}

/// Supported values for `animation-iteration-count`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationIterationCount {
  /// A finite iteration count.
  Number(f32),
  /// Repeats forever.
  Infinite,
}

impl Default for AnimationIterationCount {
  fn default() -> Self {
    Self::Number(1.0)
  }
}

impl MakeComputed for AnimationIterationCount {}

impl<'i> FromCss<'i> for AnimationIterationCount {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|parser| parser.expect_ident_matching("infinite"))
      .is_ok()
    {
      return Ok(Self::Infinite);
    }

    let value = expect_number(input)?;
    if value < 0.0 {
      return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
    }

    Ok(Self::Number(value))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("number"), CssToken::Keyword("infinite")]
  }
}

/// Parsed values for `animation-iteration-count`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationIterationCounts(pub Box<[AnimationIterationCount]>);

impl MakeComputed for AnimationIterationCounts {}

impl<'i> FromCss<'i> for AnimationIterationCounts {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_separated(input, AnimationIterationCount::from_css).map(Self)
  }

  fn valid_tokens() -> &'static [CssToken] {
    AnimationIterationCount::valid_tokens()
  }
}

/// Supported values for `animation-direction`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AnimationDirection {
  #[default]
  /// Plays from the first keyframe to the last keyframe.
  Normal,
  /// Plays from the last keyframe to the first keyframe.
  Reverse,
  /// Alternates between forward and reverse playback.
  Alternate,
  /// Alternates between reverse and forward playback.
  AlternateReverse,
}

declare_enum_from_css_impl!(
  AnimationDirection,
  "normal" => AnimationDirection::Normal,
  "reverse" => AnimationDirection::Reverse,
  "alternate" => AnimationDirection::Alternate,
  "alternate-reverse" => AnimationDirection::AlternateReverse,
);

/// Parsed values for `animation-direction`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationDirections(pub Box<[AnimationDirection]>);

impl MakeComputed for AnimationDirections {}

impl<'i> FromCss<'i> for AnimationDirections {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_separated(input, AnimationDirection::from_css).map(Self)
  }

  fn valid_tokens() -> &'static [CssToken] {
    AnimationDirection::valid_tokens()
  }
}

/// Supported values for `animation-fill-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AnimationFillMode {
  #[default]
  /// Does not apply keyframe values outside the active interval.
  None,
  /// Keeps the final keyframe value after completion.
  Forwards,
  /// Applies the starting keyframe value during delay.
  Backwards,
  /// Applies both backwards and forwards fill behavior.
  Both,
}

declare_enum_from_css_impl!(
  AnimationFillMode,
  "none" => AnimationFillMode::None,
  "forwards" => AnimationFillMode::Forwards,
  "backwards" => AnimationFillMode::Backwards,
  "both" => AnimationFillMode::Both,
);

/// Parsed values for `animation-fill-mode`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationFillModes(pub Box<[AnimationFillMode]>);

impl MakeComputed for AnimationFillModes {}

impl<'i> FromCss<'i> for AnimationFillModes {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_separated(input, AnimationFillMode::from_css).map(Self)
  }

  fn valid_tokens() -> &'static [CssToken] {
    AnimationFillMode::valid_tokens()
  }
}

/// Supported values for `animation-play-state`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AnimationPlayState {
  #[default]
  /// The animation is actively progressing with time.
  Running,
  /// The animation is frozen and does not advance.
  Paused,
}

declare_enum_from_css_impl!(
  AnimationPlayState,
  "running" => AnimationPlayState::Running,
  "paused" => AnimationPlayState::Paused,
);

/// Parsed values for `animation-play-state`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationPlayStates(pub Box<[AnimationPlayState]>);

impl MakeComputed for AnimationPlayStates {}

impl<'i> FromCss<'i> for AnimationPlayStates {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_separated(input, AnimationPlayState::from_css).map(Self)
  }

  fn valid_tokens() -> &'static [CssToken] {
    AnimationPlayState::valid_tokens()
  }
}

fn parse_comma_separated<'i, T>(
  input: &mut Parser<'i, '_>,
  mut parse_item: impl FnMut(&mut Parser<'i, '_>) -> ParseResult<'i, T>,
) -> ParseResult<'i, Box<[T]>> {
  let mut items = Vec::new();

  loop {
    items.push(parse_item(input)?);
    if input.try_parse(Parser::expect_comma).is_err() {
      break;
    }
  }

  Ok(items.into_boxed_slice())
}

fn parse_timing_keyword<'i>(
  input: &mut Parser<'i, '_>,
) -> ParseResult<'i, AnimationTimingFunction> {
  let location = input.current_source_location();
  let token = input.next()?;
  let Token::Ident(ident) = token else {
    return Err(AnimationTimingFunction::unexpected_token_error(
      location, token,
    ));
  };

  match_ignore_ascii_case! {ident,
    "linear" => Ok(AnimationTimingFunction::Linear),
    "ease" => Ok(AnimationTimingFunction::Ease),
    "ease-in" => Ok(AnimationTimingFunction::EaseIn),
    "ease-out" => Ok(AnimationTimingFunction::EaseOut),
    "ease-in-out" => Ok(AnimationTimingFunction::EaseInOut),
    "step-start" => Ok(AnimationTimingFunction::StepStart),
    "step-end" => Ok(AnimationTimingFunction::StepEnd),
    _ => Err(AnimationTimingFunction::unexpected_token_error(location, token)),
  }
}

fn parse_step_position<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, StepPosition> {
  let location = input.current_source_location();
  let token = input.next()?;
  let Token::Ident(ident) = token else {
    return Err(AnimationTimingFunction::unexpected_token_error(
      location, token,
    ));
  };

  match_ignore_ascii_case! {ident,
    "start" => Ok(StepPosition::Start),
    "end" => Ok(StepPosition::End),
    _ => Err(AnimationTimingFunction::unexpected_token_error(location, token)),
  }
}

fn parse_steps_function<'i>(
  input: &mut Parser<'i, '_>,
) -> ParseResult<'i, AnimationTimingFunction> {
  input.expect_function_matching("steps")?;
  input.parse_nested_block(|input| {
    let count = input.expect_integer()?;
    if count <= 0 {
      return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
    }

    input.expect_comma()?;
    let position = parse_step_position(input)?;
    Ok(AnimationTimingFunction::Steps(count as u32, position))
  })
}

fn expect_number<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, f32> {
  let location = input.current_source_location();
  let token = input.next()?;
  let Token::Number { value, .. } = token else {
    return Err(AnimationTime::unexpected_token_error(location, token));
  };
  Ok(*value)
}

pub(crate) fn repeated_list_value<T: Clone>(values: &[T], index: usize, default: T) -> T {
  if values.is_empty() {
    return default;
  }

  values[index % values.len()].clone()
}

pub(crate) fn timing_function_at(
  values: &AnimationTimingFunctions,
  index: usize,
) -> AnimationTimingFunction {
  repeated_list_value(&values.0, index, AnimationTimingFunction::default())
}

pub(crate) fn time_at(
  values: &AnimationDurations,
  index: usize,
  default: AnimationTime,
) -> AnimationTime {
  repeated_list_value(&values.0, index, default)
}

pub(crate) fn iteration_count_at(
  values: &AnimationIterationCounts,
  index: usize,
) -> AnimationIterationCount {
  repeated_list_value(&values.0, index, AnimationIterationCount::default())
}

pub(crate) fn direction_at(values: &AnimationDirections, index: usize) -> AnimationDirection {
  repeated_list_value(&values.0, index, AnimationDirection::default())
}

pub(crate) fn fill_mode_at(values: &AnimationFillModes, index: usize) -> AnimationFillMode {
  repeated_list_value(&values.0, index, AnimationFillMode::default())
}

pub(crate) fn cubic_bezier_sample(x1: f32, y1: f32, x2: f32, y2: f32, progress: f32) -> f32 {
  fn sample_curve(a: f32, b: f32, c: f32, t: f32) -> f32 {
    ((a * t + b) * t + c) * t
  }

  fn sample_derivative(a: f32, b: f32, c: f32, t: f32) -> f32 {
    (3.0 * a * t + 2.0 * b) * t + c
  }

  let cx = 3.0 * x1;
  let bx = 3.0 * (x2 - x1) - cx;
  let ax = 1.0 - cx - bx;
  let cy = 3.0 * y1;
  let by = 3.0 * (y2 - y1) - cy;
  let ay = 1.0 - cy - by;

  let mut t = progress.clamp(0.0, 1.0);
  for _ in 0..6 {
    let x = sample_curve(ax, bx, cx, t) - progress;
    let derivative = sample_derivative(ax, bx, cx, t);
    if derivative.abs() < f32::EPSILON {
      break;
    }
    t = (t - x / derivative).clamp(0.0, 1.0);
  }

  sample_curve(ay, by, cy, t)
}

fn steps_sample(step_count: u32, position: StepPosition, progress: f32) -> f32 {
  let step_count = step_count as f32;
  let progress = progress.clamp(0.0, 1.0);

  match position {
    StepPosition::Start => (((progress * step_count).floor() + 1.0).min(step_count)) / step_count,
    StepPosition::End => ((progress * step_count).floor()) / step_count,
  }
}

pub(crate) fn apply_timing_function(function: &AnimationTimingFunction, progress: f32) -> f32 {
  match function {
    AnimationTimingFunction::Linear => progress,
    AnimationTimingFunction::Ease => cubic_bezier_sample(0.25, 0.1, 0.25, 1.0, progress),
    AnimationTimingFunction::EaseIn => cubic_bezier_sample(0.42, 0.0, 1.0, 1.0, progress),
    AnimationTimingFunction::EaseOut => cubic_bezier_sample(0.0, 0.0, 0.58, 1.0, progress),
    AnimationTimingFunction::EaseInOut => cubic_bezier_sample(0.42, 0.0, 0.58, 1.0, progress),
    AnimationTimingFunction::StepStart => steps_sample(1, StepPosition::Start, progress),
    AnimationTimingFunction::StepEnd => steps_sample(1, StepPosition::End, progress),
    AnimationTimingFunction::Steps(count, position) => steps_sample(*count, *position, progress),
    AnimationTimingFunction::CubicBezier(x1, y1, x2, y2) => {
      cubic_bezier_sample(*x1, *y1, *x2, *y2, progress)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_animation_time() {
    assert_eq!(
      AnimationTime::from_str("150ms"),
      Ok(AnimationTime::from_milliseconds(150.0))
    );
    assert_eq!(
      AnimationTime::from_str("2s"),
      Ok(AnimationTime::from_milliseconds(2000.0))
    );
  }

  #[test]
  fn parse_animation_names() {
    assert!(matches!(
      AnimationNames::from_str("fade, slide"),
      Ok(names) if names.0.as_ref() == ["fade", "slide"]
    ));
  }

  #[test]
  fn parse_steps_timing_functions() {
    assert_eq!(
      AnimationTimingFunction::from_str("step-start"),
      Ok(AnimationTimingFunction::StepStart)
    );
    assert_eq!(
      AnimationTimingFunction::from_str("step-end"),
      Ok(AnimationTimingFunction::StepEnd)
    );
    assert_eq!(
      AnimationTimingFunction::from_str("steps(4, end)"),
      Ok(AnimationTimingFunction::Steps(4, StepPosition::End))
    );
  }

  #[test]
  fn reject_invalid_cubic_bezier_x_coordinates() {
    assert!(AnimationTimingFunction::from_str("cubic-bezier(-0.1, 0, 0.2, 1)").is_err());
    assert!(AnimationTimingFunction::from_str("cubic-bezier(0.1, 0, 1.2, 1)").is_err());
  }

  #[test]
  fn reject_negative_animation_iteration_count() {
    assert!(AnimationIterationCount::from_str("-1").is_err());
  }

  #[test]
  fn cubic_bezier_preserves_overshoot() {
    let Ok(function) = AnimationTimingFunction::from_str("cubic-bezier(0.68, -0.6, 0.32, 1.6)")
    else {
      unreachable!()
    };

    let early = apply_timing_function(&function, 0.2);
    let late = apply_timing_function(&function, 0.8);

    assert!(early < 0.0, "expected negative overshoot, got {early}");
    assert!(late > 1.0, "expected positive overshoot, got {late}");
  }

  #[test]
  fn repeated_list_value_wraps() {
    let values = AnimationDirections(Box::from([
      AnimationDirection::Normal,
      AnimationDirection::Reverse,
    ]));
    assert_eq!(direction_at(&values, 2), AnimationDirection::Normal);
  }
}
