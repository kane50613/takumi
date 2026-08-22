use std::{borrow::Cow, fmt, vec::Vec};

use cssparser::{BasicParseErrorKind, Parser, Token, match_ignore_ascii_case};
use typed_builder::TypedBuilder;

use crate::style::{
  CssDescriptorKind, CssSyntaxKind, CssToken, FromCss, FromCssStr, MakeComputed, ParseResult,
  ToCss, declare_enum_from_css_impl, next_is_comma, properties::write_css_string,
  tw::TailwindPropertyParser, unexpected_token,
};

/// Implements `FromCss` for a `Box<[T]>` animation list type as a comma-separated list of `$elem`.
macro_rules! impl_comma_list_from_css {
  ($list:ty, $elem:ty) => {
    impl_comma_list_from_css!($list, $elem, <$elem>::VALID_TOKENS);
  };
  ($list:ty, $elem:ty, $valid:expr) => {
    impl<'i> FromCss<'i> for $list {
      fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
        input
          .parse_comma_separated(<$elem>::from_css)
          .map(Vec::into_boxed_slice)
      }

      const VALID_TOKENS: &'static [CssToken] = $valid;
    }
  };
}

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
        _ => Err(unexpected_token!(location, token)),
      },
      Token::Number { value, .. } if *value == 0.0 => Ok(Self::from_milliseconds(0.0)),
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Time)];
}

/// Parsed value for one `animation-name`.
pub(crate) type AnimationName = Option<String>;

/// Parsed values for `animation-name`.
pub(crate) type AnimationNames = Box<[AnimationName]>;

impl MakeComputed for AnimationNames {}

impl_comma_list_from_css!(
  AnimationNames,
  AnimationName,
  &[
    CssToken::Keyword("none"),
    CssToken::Syntax(CssSyntaxKind::CustomIdent),
    CssToken::Syntax(CssSyntaxKind::String),
  ]
);

/// Parsed values for `animation-duration` and `animation-delay`.
pub(crate) type AnimationDurations = Box<[AnimationTime]>;

impl_comma_list_from_css!(AnimationDurations, AnimationTime);

/// Supported CSS timing functions for animations.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
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
#[non_exhaustive]
pub enum StepPosition {
  /// Jumps at the start of each step interval.
  Start,
  /// Jumps at the end of each step interval.
  End,
  /// Jumps at both ends, giving one more jump than steps.
  JumpBoth,
  /// Jumps at neither end, giving one fewer jump than steps.
  JumpNone,
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

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("linear"),
    CssToken::Keyword("ease"),
    CssToken::Keyword("ease-in"),
    CssToken::Keyword("ease-out"),
    CssToken::Keyword("ease-in-out"),
    CssToken::Keyword("step-start"),
    CssToken::Keyword("step-end"),
    CssToken::Descriptor(CssDescriptorKind::StepsFn),
    CssToken::Descriptor(CssDescriptorKind::CubicBezierFn),
  ];
}

/// Parsed values for `animation-timing-function`.
pub(crate) type AnimationTimingFunctions = Box<[AnimationTimingFunction]>;

impl_comma_list_from_css!(AnimationTimingFunctions, AnimationTimingFunction);

/// Supported values for `animation-iteration-count`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
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

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Number),
    CssToken::Keyword("infinite"),
  ];
}

/// Parsed values for `animation-iteration-count`.
pub(crate) type AnimationIterationCounts = Box<[AnimationIterationCount]>;

impl_comma_list_from_css!(AnimationIterationCounts, AnimationIterationCount);

/// Supported values for `animation-direction`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
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
pub(crate) type AnimationDirections = Box<[AnimationDirection]>;

impl_comma_list_from_css!(AnimationDirections, AnimationDirection);

/// Supported values for `animation-fill-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
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
pub(crate) type AnimationFillModes = Box<[AnimationFillMode]>;

impl_comma_list_from_css!(AnimationFillModes, AnimationFillMode);

/// Supported values for `animation-play-state`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
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
pub(crate) type AnimationPlayStates = Box<[AnimationPlayState]>;

impl_comma_list_from_css!(AnimationPlayStates, AnimationPlayState);

/// Parsed value for one `animation` shorthand item.
#[derive(Debug, Clone, PartialEq, Default, TypedBuilder)]
#[builder(field_defaults(default))]
#[non_exhaustive]
pub struct Animation {
  /// Parsed `animation-duration`.
  pub duration: AnimationTime,
  /// Parsed `animation-delay`.
  pub delay: AnimationTime,
  /// Parsed `animation-timing-function`.
  pub timing_function: AnimationTimingFunction,
  /// Parsed `animation-iteration-count`.
  pub iteration_count: AnimationIterationCount,
  /// Parsed `animation-direction`.
  pub direction: AnimationDirection,
  /// Parsed `animation-fill-mode`.
  pub fill_mode: AnimationFillMode,
  /// Parsed `animation-play-state`.
  pub play_state: AnimationPlayState,
  /// Parsed `animation-name`, with `None` representing the CSS `none` keyword.
  #[builder(setter(strip_option))]
  pub name: Option<String>,
}

impl MakeComputed for Animation {}

impl<'i> FromCss<'i> for Animation {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut animation = Self::default();
    let mut time_count = 0;

    while !input.is_exhausted() && !next_is_comma(input) {
      if let Ok(value) = input.try_parse(AnimationTime::from_css) {
        match time_count {
          0 => animation.duration = value,
          1 => animation.delay = value,
          _ => return Err(input.new_error_for_next_token()),
        }
        time_count += 1;
        continue;
      }

      if let Ok(value) = input.try_parse(AnimationTimingFunction::from_css) {
        animation.timing_function = value;
        continue;
      }

      if let Ok(value) = input.try_parse(AnimationIterationCount::from_css) {
        animation.iteration_count = value;
        continue;
      }

      if let Ok(value) = input.try_parse(AnimationDirection::from_css) {
        animation.direction = value;
        continue;
      }

      if let Ok(value) = input.try_parse(AnimationFillMode::from_css) {
        animation.fill_mode = value;
        continue;
      }

      if let Ok(value) = input.try_parse(AnimationPlayState::from_css) {
        animation.play_state = value;
        continue;
      }

      if let Ok(value) = input.try_parse(AnimationName::from_css) {
        animation.name = value;
        continue;
      }

      return Err(input.new_error_for_next_token());
    }

    Ok(animation)
  }

  const VALID_TOKENS: &'static [CssToken] = Animations::VALID_TOKENS;
}

/// Parsed values for the `animation` shorthand.
pub(crate) type Animations = Box<[Animation]>;

impl<'i> FromCss<'i> for Animations {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(
      input
        .parse_comma_separated(Animation::from_css)?
        .into_boxed_slice(),
    )
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Time),
    CssToken::Syntax(CssSyntaxKind::EasingFunction),
    CssToken::Syntax(CssSyntaxKind::Number),
    CssToken::Keyword("infinite"),
    CssToken::Keyword("normal"),
    CssToken::Keyword("reverse"),
    CssToken::Keyword("alternate"),
    CssToken::Keyword("alternate-reverse"),
    CssToken::Keyword("none"),
    CssToken::Keyword("forwards"),
    CssToken::Keyword("backwards"),
    CssToken::Keyword("both"),
    CssToken::Keyword("running"),
    CssToken::Keyword("paused"),
    CssToken::Syntax(CssSyntaxKind::CustomIdent),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl TailwindPropertyParser for Animations {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "none" => Some(Box::from([Animation::default()])),
      "spin" => Some(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(1000.0),
        timing_function: AnimationTimingFunction::Linear,
        iteration_count: AnimationIterationCount::Infinite,
        name: Some("spin".to_string()),
        ..Animation::default()
      }])),
      "ping" => Some(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(1000.0),
        timing_function: AnimationTimingFunction::CubicBezier(0.0, 0.0, 0.2, 1.0),
        iteration_count: AnimationIterationCount::Infinite,
        name: Some("ping".to_string()),
        ..Animation::default()
      }])),
      "pulse" => Some(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(2000.0),
        timing_function: AnimationTimingFunction::CubicBezier(0.4, 0.0, 0.6, 1.0),
        iteration_count: AnimationIterationCount::Infinite,
        name: Some("pulse".to_string()),
        ..Animation::default()
      }])),
      "bounce" => Some(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(1000.0),
        iteration_count: AnimationIterationCount::Infinite,
        name: Some("bounce".to_string()),
        ..Animation::default()
      }])),
      _ => None,
    }
  }

  fn parse_tw_with_arbitrary(token: &str) -> Option<Self> {
    if let Some(value) = token
      .strip_prefix('[')
      .and_then(|value| value.strip_suffix(']'))
    {
      let value = if value.contains('_') {
        Cow::Owned(value.replace('_', " "))
      } else {
        Cow::Borrowed(value)
      };

      return Self::from_css_str(&value).ok();
    }

    Self::parse_tw(token)
  }
}

fn parse_timing_keyword<'i>(
  input: &mut Parser<'i, '_>,
) -> ParseResult<'i, AnimationTimingFunction> {
  let location = input.current_source_location();
  let token = input.next()?;
  let Token::Ident(ident) = token else {
    return Err(unexpected_token!(AnimationTimingFunction, location, token));
  };

  match_ignore_ascii_case! {ident,
    "linear" => Ok(AnimationTimingFunction::Linear),
    "ease" => Ok(AnimationTimingFunction::Ease),
    "ease-in" => Ok(AnimationTimingFunction::EaseIn),
    "ease-out" => Ok(AnimationTimingFunction::EaseOut),
    "ease-in-out" => Ok(AnimationTimingFunction::EaseInOut),
    "step-start" => Ok(AnimationTimingFunction::StepStart),
    "step-end" => Ok(AnimationTimingFunction::StepEnd),
    _ => Err(unexpected_token!(AnimationTimingFunction, location, token)),
  }
}

fn parse_step_position<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, StepPosition> {
  let location = input.current_source_location();
  let token = input.next()?;
  let Token::Ident(ident) = token else {
    return Err(unexpected_token!(AnimationTimingFunction, location, token));
  };

  match_ignore_ascii_case! {ident,
    "start" | "jump-start" => Ok(StepPosition::Start),
    "end" | "jump-end" => Ok(StepPosition::End),
    "jump-both" => Ok(StepPosition::JumpBoth),
    "jump-none" => Ok(StepPosition::JumpNone),
    _ => Err(unexpected_token!(AnimationTimingFunction, location, token)),
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

    let position = if input.try_parse(Parser::expect_comma).is_ok() {
      parse_step_position(input)?
    } else {
      StepPosition::End
    };

    if position == StepPosition::JumpNone && count < 2 {
      return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
    }

    Ok(AnimationTimingFunction::Steps(count as u32, position))
  })
}

fn expect_number<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, f32> {
  let location = input.current_source_location();
  let token = input.next()?;
  let Token::Number { value, .. } = token else {
    return Err(unexpected_token!(AnimationTime, location, token));
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
  repeated_list_value(values, index, AnimationTimingFunction::default())
}

pub(crate) fn time_at(
  values: &AnimationDurations,
  index: usize,
  default: AnimationTime,
) -> AnimationTime {
  repeated_list_value(values, index, default)
}

pub(crate) fn iteration_count_at(
  values: &AnimationIterationCounts,
  index: usize,
) -> AnimationIterationCount {
  repeated_list_value(values, index, AnimationIterationCount::default())
}

pub(crate) fn direction_at(values: &AnimationDirections, index: usize) -> AnimationDirection {
  repeated_list_value(values, index, AnimationDirection::default())
}

pub(crate) fn fill_mode_at(values: &AnimationFillModes, index: usize) -> AnimationFillMode {
  repeated_list_value(values, index, AnimationFillMode::default())
}

fn cubic_bezier_sample(x1: f32, y1: f32, x2: f32, y2: f32, progress: f32) -> f32 {
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
  let steps = step_count as f32;
  let progress = progress.clamp(0.0, 1.0);

  // Jumps differ from steps when both or neither end point is discontinuous.
  // https://drafts.csswg.org/css-easing-1/#step-easing-functions
  let (jumps, start_offset) = match position {
    StepPosition::Start => (steps, 1.0),
    StepPosition::End => (steps, 0.0),
    StepPosition::JumpBoth => (steps + 1.0, 1.0),
    StepPosition::JumpNone => ((steps - 1.0).max(1.0), 0.0),
  };

  ((progress * steps + start_offset).floor().clamp(0.0, jumps)) / jumps
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

impl ToCss for AnimationTime {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}ms", self.milliseconds)
  }
}

impl ToCss for StepPosition {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Start => dest.write_str("start"),
      Self::End => dest.write_str("end"),
      Self::JumpBoth => dest.write_str("jump-both"),
      Self::JumpNone => dest.write_str("jump-none"),
    }
  }
}

impl ToCss for AnimationTimingFunction {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Linear => dest.write_str("linear"),
      Self::Ease => dest.write_str("ease"),
      Self::EaseIn => dest.write_str("ease-in"),
      Self::EaseOut => dest.write_str("ease-out"),
      Self::EaseInOut => dest.write_str("ease-in-out"),
      Self::StepStart => dest.write_str("step-start"),
      Self::StepEnd => dest.write_str("step-end"),
      Self::Steps(count, position) => {
        dest.write_str("steps(")?;
        write!(dest, "{}", count)?;
        dest.write_str(", ")?;
        position.to_css(dest)?;
        dest.write_char(')')
      }
      Self::CubicBezier(x1, y1, x2, y2) => write!(dest, "cubic-bezier({x1}, {y1}, {x2}, {y2})"),
    }
  }
}

impl ToCss for AnimationIterationCount {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Number(n) => write!(dest, "{}", n),
      Self::Infinite => dest.write_str("infinite"),
    }
  }
}

impl ToCss for Animation {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if let Some(ref name) = self.name {
      let needs_quoting = name
        .chars()
        .any(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'));
      if needs_quoting {
        write_css_string(dest, name)?;
      } else {
        dest.write_str(name)?;
      }
    } else {
      dest.write_str("none")?;
    }
    dest.write_char(' ')?;
    self.duration.to_css(dest)?;
    dest.write_char(' ')?;
    self.timing_function.to_css(dest)?;
    dest.write_char(' ')?;
    self.delay.to_css(dest)?;
    dest.write_char(' ')?;
    self.iteration_count.to_css(dest)?;
    dest.write_char(' ')?;
    self.direction.to_css(dest)?;
    dest.write_char(' ')?;
    self.fill_mode.to_css(dest)?;
    dest.write_char(' ')?;
    self.play_state.to_css(dest)
  }
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use super::*;

  #[test]
  fn parse_animation_time() {
    assert_eq!(
      AnimationTime::from_css_str("150ms"),
      Ok(AnimationTime::from_milliseconds(150.0))
    );
    assert_eq!(
      AnimationTime::from_css_str("2s"),
      Ok(AnimationTime::from_milliseconds(2000.0))
    );
  }

  #[test]
  fn parse_animation_names() {
    assert_matches!(
      AnimationNames::from_css_str("fade, slide"),
      Ok(names) if names.as_ref() == [Some("fade".to_string()), Some("slide".to_string())]
    );
  }

  #[test]
  fn parse_quoted_animation_names() {
    assert_matches!(
      AnimationNames::from_css_str("\"fade\", slide"),
      Ok(names) if names.as_ref() == [Some("fade".to_string()), Some("slide".to_string())]
    );
  }

  #[test]
  fn parse_animation_names_with_none_entry() {
    assert_matches!(
      AnimationNames::from_css_str("none, slide"),
      Ok(names) if names.as_ref() == [None, Some("slide".to_string())]
    );
  }

  #[test]
  fn parse_steps_timing_functions() {
    assert_eq!(
      AnimationTimingFunction::from_css_str("step-start"),
      Ok(AnimationTimingFunction::StepStart)
    );
    assert_eq!(
      AnimationTimingFunction::from_css_str("step-end"),
      Ok(AnimationTimingFunction::StepEnd)
    );
    assert_eq!(
      AnimationTimingFunction::from_css_str("steps(4, end)"),
      Ok(AnimationTimingFunction::Steps(4, StepPosition::End))
    );
    assert_eq!(
      AnimationTimingFunction::from_css_str("steps(4)"),
      Ok(AnimationTimingFunction::Steps(4, StepPosition::End))
    );
    assert_eq!(
      AnimationTimingFunction::from_css_str("steps(4, jump-start)"),
      Ok(AnimationTimingFunction::Steps(4, StepPosition::Start))
    );
    assert_eq!(
      AnimationTimingFunction::from_css_str("steps(4, jump-none)"),
      Ok(AnimationTimingFunction::Steps(4, StepPosition::JumpNone))
    );
    assert_eq!(
      AnimationTimingFunction::from_css_str("steps(4, jump-both)"),
      Ok(AnimationTimingFunction::Steps(4, StepPosition::JumpBoth))
    );
  }

  #[test]
  fn reject_single_step_jump_none() {
    assert!(AnimationTimingFunction::from_css_str("steps(1, jump-none)").is_err());
  }

  #[test]
  fn steps_timing_functions_sample_as_staircase() {
    let sample = |css: &str, progress: f32| {
      apply_timing_function(
        &AnimationTimingFunction::from_css_str(css).unwrap(),
        progress,
      )
    };

    for (progress, expected) in [(0.0, 0.0), (0.3, 0.25), (0.6, 0.5), (1.0, 1.0)] {
      assert_eq!(sample("steps(4, jump-end)", progress), expected);
    }

    for (progress, expected) in [(0.0, 0.0), (0.3, 1.0 / 3.0), (0.6, 2.0 / 3.0), (1.0, 1.0)] {
      assert_eq!(sample("steps(4, jump-none)", progress), expected);
    }

    for (progress, expected) in [(0.0, 0.2), (0.3, 0.4), (0.6, 0.6), (1.0, 1.0)] {
      assert_eq!(sample("steps(4, jump-both)", progress), expected);
    }

    for (progress, expected) in [(0.0, 0.25), (0.3, 0.5), (0.6, 0.75), (1.0, 1.0)] {
      assert_eq!(sample("steps(4, jump-start)", progress), expected);
    }
  }

  #[test]
  fn parse_animation_shorthand_with_steps() {
    assert_eq!(
      Animations::from_css_str("fade 1s steps(4, jump-none)"),
      Ok(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(1000.0),
        timing_function: AnimationTimingFunction::Steps(4, StepPosition::JumpNone),
        name: Some("fade".to_string()),
        ..Animation::default()
      }]))
    );
  }

  #[test]
  fn reject_invalid_cubic_bezier_x_coordinates() {
    assert!(AnimationTimingFunction::from_css_str("cubic-bezier(-0.1, 0, 0.2, 1)").is_err());
    assert!(AnimationTimingFunction::from_css_str("cubic-bezier(0.1, 0, 1.2, 1)").is_err());
  }

  #[test]
  fn reject_negative_animation_iteration_count() {
    assert!(AnimationIterationCount::from_css_str("-1").is_err());
  }

  #[test]
  fn cubic_bezier_preserves_overshoot() {
    let Ok(function) = AnimationTimingFunction::from_css_str("cubic-bezier(0.68, -0.6, 0.32, 1.6)")
    else {
      return;
    };

    let early = apply_timing_function(&function, 0.2);
    let late = apply_timing_function(&function, 0.8);

    assert!(early < 0.0, "expected negative overshoot, got {early}");
    assert!(late > 1.0, "expected positive overshoot, got {late}");
  }

  #[test]
  fn repeated_list_value_wraps() {
    let values = [AnimationDirection::Normal, AnimationDirection::Reverse].into();
    assert_eq!(direction_at(&values, 2), AnimationDirection::Normal);
  }

  #[test]
  fn parse_animation_shorthand() {
    assert_eq!(
      Animations::from_css_str("fade 1s ease-in 200ms 2 alternate both paused"),
      Ok(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(1000.0),
        delay: AnimationTime::from_milliseconds(200.0),
        timing_function: AnimationTimingFunction::EaseIn,
        iteration_count: AnimationIterationCount::Number(2.0),
        direction: AnimationDirection::Alternate,
        fill_mode: AnimationFillMode::Both,
        play_state: AnimationPlayState::Paused,
        name: Some("fade".to_string()),
      }]))
    );
  }

  #[test]
  fn parse_animation_shorthand_with_quoted_name() {
    assert_eq!(
      Animations::from_css_str("\"fade\" 1s linear"),
      Ok(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(1000.0),
        timing_function: AnimationTimingFunction::Linear,
        name: Some("fade".to_string()),
        ..Animation::default()
      }]))
    );
  }

  #[test]
  fn parse_multiple_animation_shorthand_values() {
    assert_eq!(
      Animations::from_css_str("fade 1s linear, 2s slide"),
      Ok(Box::from([
        Animation {
          duration: AnimationTime::from_milliseconds(1000.0),
          timing_function: AnimationTimingFunction::Linear,
          name: Some("fade".to_string()),
          ..Animation::default()
        },
        Animation {
          duration: AnimationTime::from_milliseconds(2000.0),
          name: Some("slide".to_string()),
          ..Animation::default()
        },
      ]))
    );
  }

  #[test]
  fn parse_tailwind_animation_preset() {
    assert_eq!(
      Animations::parse_tw("spin"),
      Some(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(1000.0),
        timing_function: AnimationTimingFunction::Linear,
        iteration_count: AnimationIterationCount::Infinite,
        name: Some("spin".to_string()),
        ..Animation::default()
      }]))
    );
  }

  #[test]
  fn parse_tailwind_animation_arbitrary_value() {
    assert_eq!(
      Animations::parse_tw_with_arbitrary("[wiggle_1s_ease-in-out_infinite]"),
      Some(Box::from([Animation {
        duration: AnimationTime::from_milliseconds(1000.0),
        timing_function: AnimationTimingFunction::EaseInOut,
        iteration_count: AnimationIterationCount::Infinite,
        name: Some("wiggle".to_string()),
        ..Animation::default()
      }]))
    );
  }
}
