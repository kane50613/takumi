use std::{borrow::Cow, fmt, sync::Arc};

use cssparser::{Parser, ParserInput};

use crate::style::{Color, SizingContext, math::lcm};

/// Parser result type alias for CSS property parsers.
pub(crate) type ParseResult<'i, T> = Result<T, cssparser::ParseError<'i, Cow<'i, str>>>;

/// Owned error returned by [`FromCssStr::from_css_str`]. Carries a
/// human-readable message and borrows nothing from the parser, keeping
/// `cssparser` out of the public API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
  message: String,
}

impl fmt::Display for ParseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl std::error::Error for ParseError {}

impl ParseError {
  /// Stringifies an internal `cssparser` error into an owned [`ParseError`].
  pub(crate) fn from_css_error(error: &cssparser::ParseError<'_, Cow<'_, str>>) -> Self {
    let message = match &error.kind {
      cssparser::ParseErrorKind::Custom(message) => message.to_string(),
      basic => format!("{basic:?}"),
    };

    Self { message }
  }
}

/// Runs the internal `FromCss` parser over `source`, mapping the borrowed
/// `cssparser` error into the owned public [`ParseError`].
pub(crate) fn parse_css_str<T>(source: &str) -> Result<T, ParseError>
where
  T: for<'i> FromCss<'i>,
{
  let mut input = ParserInput::new(source);
  let mut parser = Parser::new(&mut input);

  T::from_css(&mut parser).map_err(|error| ParseError::from_css_error(&error))
}

/// Parses a CSS value type from a string.
pub trait FromCssStr: Sized {
  /// Parses `source` into this value type.
  fn from_css_str(source: &str) -> Result<Self, ParseError>;
}

impl<T> FromCssStr for T
where
  T: for<'i> FromCss<'i>,
{
  fn from_css_str(source: &str) -> Result<Self, ParseError> {
    parse_css_str::<Self>(source)
  }
}

/// Compact identifiers for frequently reused CSS syntax tokens.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub(crate) enum CssSyntaxKind {
  /// `<angle>`
  Angle,
  /// `<border-style>`
  BorderStyle,
  /// `<clip>`
  Clip,
  /// `<color>`
  Color,
  /// `<custom-ident>`
  CustomIdent,
  /// `<easing-function>`
  EasingFunction,
  /// `<family-name>`
  FamilyName,
  /// `<generic-name>`
  GenericName,
  /// `<ident>`
  Ident,
  /// `<image>`
  Image,
  /// `<integer>`
  Integer,
  /// `<length>`
  Length,
  /// `<line-names>`
  LineNames,
  /// `<number>`
  Number,
  /// `<percentage>`
  Percentage,
  /// `<position>`
  Position,
  /// `<repeat>`
  Repeat,
  /// `<string>`
  String,
  /// `<time>`
  Time,
  /// `<track-size>`
  TrackSize,
  /// `<transform-function>`
  TransformFunction,
}

impl CssSyntaxKind {
  const fn as_str(self) -> &'static str {
    match self {
      Self::Angle => "angle",
      Self::BorderStyle => "border-style",
      Self::Clip => "clip",
      Self::Color => "color",
      Self::CustomIdent => "custom-ident",
      Self::EasingFunction => "easing-function",
      Self::FamilyName => "family-name",
      Self::GenericName => "generic-name",
      Self::Ident => "ident",
      Self::Image => "image",
      Self::Integer => "integer",
      Self::Length => "length",
      Self::LineNames => "line-names",
      Self::Number => "number",
      Self::Percentage => "percentage",
      Self::Position => "position",
      Self::Repeat => "repeat",
      Self::String => "string",
      Self::Time => "time",
      Self::TrackSize => "track-size",
      Self::TransformFunction => "transform-function",
    }
  }
}

/// Compact identifiers for reusable CSS descriptor and function labels.
#[derive(Clone, Copy)]
pub(crate) enum CssDescriptorKind {
  /// `<blur()>`
  BlurFn,
  /// `<brightness()>`
  BrightnessFn,
  /// `<circle()>`
  CircleFn,
  /// `<color and percentage>`
  ColorAndPercentage,
  /// `<color-mix()>`
  ColorMixFn,
  /// `<conic-gradient()>`
  ConicGradientFn,
  /// `<repeating-conic-gradient()>`
  RepeatingConicGradientFn,
  /// `<contrast()>`
  ContrastFn,
  /// `<cubic-bezier()>`
  CubicBezierFn,
  /// `<drop-shadow()>`
  DropShadowFn,
  /// `<ellipse()>`
  EllipseFn,
  /// `<grayscale()>`
  GrayscaleFn,
  /// `<hue-rotate()>`
  HueRotateFn,
  /// `<in <color-space>>`
  InColorSpace,
  /// `<inset()>`
  InsetFn,
  /// `<invert()>`
  InvertFn,
  /// `<linear-gradient()>`
  LinearGradientFn,
  /// `<repeating-linear-gradient()>`
  RepeatingLinearGradientFn,
  /// `<minmax()>`
  MinmaxFn,
  /// `<opacity()>`
  OpacityFn,
  /// `<path()>`
  PathFn,
  /// `<polygon()>`
  PolygonFn,
  /// `<radial-gradient()>`
  RadialGradientFn,
  /// `<repeating-radial-gradient()>`
  RepeatingRadialGradientFn,
  /// `<repeat()>`
  RepeatFn,
  /// `<saturate()>`
  SaturateFn,
  /// `<sepia()>`
  SepiaFn,
  /// `<steps()>`
  StepsFn,
  /// `<superellipse()>`
  SuperellipseFn,
  /// `<text-wrap-mode>`
  TextWrapMode,
  /// `<text-wrap-style>`
  TextWrapStyle,
  /// `<url()>`
  UrlFn,
  /// `<white-space-collapse>`
  WhiteSpaceCollapse,
}

impl CssDescriptorKind {
  const fn as_str(self) -> &'static str {
    match self {
      Self::BlurFn => "blur()",
      Self::BrightnessFn => "brightness()",
      Self::CircleFn => "circle()",
      Self::ColorAndPercentage => "color and percentage",
      Self::ColorMixFn => "color-mix()",
      Self::ConicGradientFn => "conic-gradient()",
      Self::RepeatingConicGradientFn => "repeating-conic-gradient()",
      Self::ContrastFn => "contrast()",
      Self::CubicBezierFn => "cubic-bezier()",
      Self::DropShadowFn => "drop-shadow()",
      Self::EllipseFn => "ellipse()",
      Self::GrayscaleFn => "grayscale()",
      Self::HueRotateFn => "hue-rotate()",
      Self::InColorSpace => "in <color-space>",
      Self::InsetFn => "inset()",
      Self::InvertFn => "invert()",
      Self::LinearGradientFn => "linear-gradient()",
      Self::RepeatingLinearGradientFn => "repeating-linear-gradient()",
      Self::MinmaxFn => "minmax()",
      Self::OpacityFn => "opacity()",
      Self::PathFn => "path()",
      Self::PolygonFn => "polygon()",
      Self::RadialGradientFn => "radial-gradient()",
      Self::RepeatingRadialGradientFn => "repeating-radial-gradient()",
      Self::RepeatFn => "repeat()",
      Self::SaturateFn => "saturate()",
      Self::SepiaFn => "sepia()",
      Self::StepsFn => "steps()",
      Self::SuperellipseFn => "superellipse()",
      Self::TextWrapMode => "text-wrap-mode",
      Self::TextWrapStyle => "text-wrap-style",
      Self::UrlFn => "url()",
      Self::WhiteSpaceCollapse => "white-space-collapse",
    }
  }
}

/// Enum representing CSS tokens.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub(crate) enum CssToken {
  /// A CSS keyword.
  Keyword(&'static str),
  /// A common CSS syntax token backed by a compact enum table.
  Syntax(CssSyntaxKind),
  /// A reusable CSS descriptor backed by a compact enum table.
  Descriptor(CssDescriptorKind),
}

impl CssToken {
  /// Total length of `lists` flattened, for sizing the [`Self::merge_lists`] array.
  pub(crate) const fn merged_len(lists: &[&[CssToken]]) -> usize {
    let (mut len, mut list) = (0, 0);
    while list < lists.len() {
      len += lists[list].len();
      list += 1;
    }
    len
  }

  /// Concatenates token lists into a single array, e.g. to build a shorthand's
  /// `VALID_TOKENS` from its longhands'. `N` must be [`Self::merged_len`].
  pub(crate) const fn merge_lists<const N: usize>(lists: &[&[CssToken]]) -> [CssToken; N] {
    let mut merged = [CssToken::Keyword(""); N];
    let (mut index, mut list) = (0, 0);
    while list < lists.len() {
      let tokens = lists[list];
      let mut token = 0;
      while token < tokens.len() {
        merged[index] = tokens[token];
        index += 1;
        token += 1;
      }
      list += 1;
    }
    merged
  }
}

impl std::fmt::Display for CssToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CssToken::Keyword(keyword) => write!(f, "'{}'", keyword),
      CssToken::Syntax(token) => write!(f, "<{}>", token.as_str()),
      CssToken::Descriptor(token) => write!(f, "<{}>", token.as_str()),
    }
  }
}

/// Defines reusable message templates for CSS parse errors.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub(crate) enum CssExpectedMessage {
  /// Expects a value or the `none` keyword.
  ValueOrNone,
  /// Expects exactly one value.
  OneValue,
  /// Expects one or two values.
  OneOrTwoValues,
  /// Expects one to four values.
  OneToFourValues,
  /// Expects the border-radius shorthand grammar.
  BorderRadius,
}

impl CssExpectedMessage {
  /// Builds the parse-error message for an unexpected token.
  pub(crate) fn build_message(&self, token: &str, valid_tokens: String) -> String {
    match self {
      Self::ValueOrNone => {
        format!("Unexpected token: {token}, expected a value of {valid_tokens} or 'none'")
      }
      Self::OneValue => format!("Unexpected token: {token}, expected a value of {valid_tokens}"),
      Self::OneOrTwoValues => {
        format!("Unexpected token: {token}, expected 1 ~ 2 values of {valid_tokens}")
      }
      Self::OneToFourValues => {
        format!("Unexpected token: {token}, expected 1 ~ 4 values of {valid_tokens}")
      }
      Self::BorderRadius => format!(
        "Unexpected token: {token}, expected 1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height"
      ),
    }
  }
}

/// Trait for types that can be parsed from CSS.
pub(crate) trait FromCss<'i> {
  /// Parses the type from a [`Parser`] instance.
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self>
  where
    Self: Sized;

  /// Returns the list of valid CSS tokens for this type.
  const VALID_TOKENS: &'static [CssToken];

  /// Message template used when building parse errors for this type.
  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::OneValue;
}

impl<'i, T: FromCss<'i>> FromCss<'i> for Option<T> {
  // 'none' is intentionally omitted and applied in `expect_message`
  const VALID_TOKENS: &'static [CssToken] = T::VALID_TOKENS;

  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::ValueOrNone;

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
      return Ok(None);
    }

    T::from_css(input).map(Some)
  }
}

impl<'i> FromCss<'i> for String {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::String),
    CssToken::Syntax(CssSyntaxKind::CustomIdent),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(input.expect_ident_or_string()?.to_string())
  }
}

/// Converts a parsed/inherited value into a computed value for the current node context.
pub(crate) trait MakeComputed {
  /// Default no-op for types that do not need computed-value normalization.
  fn make_computed(&mut self, _sizing: &SizingContext) {}
}

pub(crate) trait Animatable: Sized + Clone {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    *self = if progress >= 0.5 {
      to.clone()
    } else {
      from.clone()
    };
  }

  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::Discrete
  }

  fn neutral_value_like(_other: &Self) -> Option<Self> {
    None
  }

  fn missing_value() -> Option<Self> {
    None
  }
}

pub(crate) enum ListInterpolationStrategy {
  Discrete,
  RepeatToLcm,
  PadToLongestWithNeutral,
}

impl<T: MakeComputed> MakeComputed for Option<T> {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Some(value) = self.as_mut() {
      value.make_computed(sizing);
    }
  }
}

impl<T: MakeComputed> MakeComputed for Box<[T]> {
  fn make_computed(&mut self, sizing: &SizingContext) {
    for value in self.iter_mut() {
      value.make_computed(sizing);
    }
  }
}

impl<T: MakeComputed> MakeComputed for Vec<T> {
  fn make_computed(&mut self, sizing: &SizingContext) {
    for value in self.iter_mut() {
      value.make_computed(sizing);
    }
  }
}

impl<T: Animatable + Clone> Animatable for Option<T> {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (from, to) {
      (Some(from), Some(to)) => {
        let mut value = from.clone();
        value.interpolate(from, to, progress, sizing, current_color);
        Some(value)
      }
      (Some(from), None) => T::missing_value().map_or_else(
        || {
          if progress >= 0.5 {
            None
          } else {
            Some(from.clone())
          }
        },
        |missing| {
          let mut value = from.clone();
          value.interpolate(from, &missing, progress, sizing, current_color);
          Some(value)
        },
      ),
      (None, Some(to)) => T::missing_value().map_or_else(
        || {
          if progress >= 0.5 {
            Some(to.clone())
          } else {
            None
          }
        },
        |missing| {
          let mut value = missing.clone();
          value.interpolate(&missing, to, progress, sizing, current_color);
          Some(value)
        },
      ),
      (None, None) => None,
    };
  }
}

impl<T: Animatable + Clone> Animatable for Box<[T]> {
  fn missing_value() -> Option<Self> {
    match T::list_interpolation_strategy() {
      ListInterpolationStrategy::Discrete => None,
      ListInterpolationStrategy::RepeatToLcm
      | ListInterpolationStrategy::PadToLongestWithNeutral => Some(Box::default()),
    }
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = interpolate_list(
      from,
      to,
      progress,
      sizing,
      current_color,
      Vec::into_boxed_slice,
    )
    .unwrap_or_else(|| {
      if progress >= 0.5 {
        to.clone()
      } else {
        from.clone()
      }
    });
  }
}

impl<T: Animatable + Clone> Animatable for Arc<[T]> {
  fn missing_value() -> Option<Self> {
    match T::list_interpolation_strategy() {
      ListInterpolationStrategy::Discrete => None,
      ListInterpolationStrategy::RepeatToLcm
      | ListInterpolationStrategy::PadToLongestWithNeutral => Some(Arc::from([])),
    }
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self =
      interpolate_list(from, to, progress, sizing, current_color, Vec::into).unwrap_or_else(|| {
        if progress >= 0.5 {
          to.clone()
        } else {
          from.clone()
        }
      });
  }
}

impl<T: Animatable + Clone> Animatable for Vec<T> {
  fn missing_value() -> Option<Self> {
    match T::list_interpolation_strategy() {
      ListInterpolationStrategy::Discrete => None,
      ListInterpolationStrategy::RepeatToLcm
      | ListInterpolationStrategy::PadToLongestWithNeutral => Some(Vec::new()),
    }
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = interpolate_list(from, to, progress, sizing, current_color, |values| values)
      .unwrap_or_else(|| {
        if progress >= 0.5 {
          to.clone()
        } else {
          from.clone()
        }
      });
  }
}

// Matches Blink's `kRepeatableListMaxLength` (list_interpolation_functions.cc);
// transitions restarted on an already-animating value otherwise compound the
// LCM expansion until it exhausts memory. See crbug.com/739197.
const MAX_INTERPOLATED_LIST_LEN: usize = 1000;

fn interpolate_list<T: Animatable + Clone, C: AsRef<[T]>, O>(
  from: &C,
  to: &C,
  progress: f32,
  sizing: &SizingContext,
  current_color: Color,
  build: impl FnOnce(Vec<T>) -> O,
) -> Option<O> {
  let from = from.as_ref();
  let to = to.as_ref();

  let values = match T::list_interpolation_strategy() {
    ListInterpolationStrategy::Discrete => {
      if from.len() != to.len() {
        return None;
      }
      interpolate_pairwise_list(from, to, from.len(), progress, sizing, current_color)
    }
    ListInterpolationStrategy::RepeatToLcm => {
      if from.is_empty() || to.is_empty() {
        return None;
      }
      let output_len = lcm(from.len(), to.len()).min(MAX_INTERPOLATED_LIST_LEN);
      interpolate_pairwise_list(from, to, output_len, progress, sizing, current_color)
    }
    ListInterpolationStrategy::PadToLongestWithNeutral => {
      interpolate_neutral_padded_list(from, to, progress, sizing, current_color)?
    }
  };

  Some(build(values))
}

fn interpolate_pairwise_list<T: Animatable + Clone>(
  from: &[T],
  to: &[T],
  output_len: usize,
  progress: f32,
  sizing: &SizingContext,
  current_color: Color,
) -> Vec<T> {
  (0..output_len)
    .map(|index| {
      let from_value = &from[index % from.len()];
      let to_value = &to[index % to.len()];
      let mut value = from_value.clone();
      value.interpolate(from_value, to_value, progress, sizing, current_color);
      value
    })
    .collect()
}

fn interpolate_neutral_padded_list<T: Animatable + Clone>(
  from: &[T],
  to: &[T],
  progress: f32,
  sizing: &SizingContext,
  current_color: Color,
) -> Option<Vec<T>> {
  let output_len = from.len().max(to.len());

  (0..output_len)
    .map(|index| {
      let from_value = if index < from.len() {
        from.get(index).cloned()
      } else {
        to.get(index).and_then(T::neutral_value_like)
      }?;
      let to_value = if index < to.len() {
        to.get(index).cloned()
      } else {
        from.get(index).and_then(T::neutral_value_like)
      }?;

      let mut value = from_value.clone();
      value.interpolate(&from_value, &to_value, progress, sizing, current_color);
      Some(value)
    })
    .collect()
}

/// Serialize a style value to its CSS string representation.
pub trait ToCss {
  /// Separator used between items when this type is serialized in a `Vec`/`[T]`
  /// list. Defaults to `", "`; space-separated grammars (e.g. `filter`, grid
  /// track lists) override it.
  const LIST_SEPARATOR: &'static str = ", ";

  /// Keyword an empty `Vec`/`[T]` list of this type serializes to instead of an
  /// empty string (`none` for `filter`/`grid-template-*`, `normal` for
  /// `font-feature-settings`/`font-variation-settings`). `None` keeps the empty
  /// string.
  const EMPTY_LIST_KEYWORD: Option<&'static str> = None;

  /// Write the CSS representation of this value into `dest`.
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result;

  /// The CSS representation of this value as an owned `String`.
  fn to_css_string(&self) -> String {
    let mut css = String::new();
    let _ = self.to_css(&mut css);
    css
  }
}

impl<T: ToCss + ?Sized> ToCss for &T {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    (*self).to_css(dest)
  }
}

impl<T: ToCss> ToCss for Option<T> {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Some(v) => v.to_css(dest),
      None => dest.write_str("none"),
    }
  }
}

fn write_css_list<W: fmt::Write, T: ToCss>(items: &[T], dest: &mut W) -> fmt::Result {
  if items.is_empty() {
    return match T::EMPTY_LIST_KEYWORD {
      Some(keyword) => dest.write_str(keyword),
      None => Ok(()),
    };
  }

  for (i, item) in items.iter().enumerate() {
    if i > 0 {
      dest.write_str(T::LIST_SEPARATOR)?;
    }
    item.to_css(dest)?;
  }
  Ok(())
}

impl<T: ToCss> ToCss for Box<[T]> {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write_css_list(self, dest)
  }
}

impl<T: ToCss> ToCss for Arc<[T]> {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write_css_list(self, dest)
  }
}

impl<T: ToCss> ToCss for Vec<T> {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write_css_list(self, dest)
  }
}

impl ToCss for f32 {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self)
  }
}

impl ToCss for u32 {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self)
  }
}

impl MakeComputed for u32 {}
impl Animatable for u32 {}

impl<'i> FromCss<'i> for u32 {
  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Integer)];

  fn from_css(input: &mut cssparser::Parser<'i, '_>) -> ParseResult<'i, Self> {
    let value = input.expect_integer()?;
    if value < 0 {
      return Err(input.new_error(cssparser::BasicParseErrorKind::QualifiedRuleInvalid));
    }
    Ok(value as u32)
  }
}

impl ToCss for i32 {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self)
  }
}

impl ToCss for String {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str(self)
  }
}

impl ToCss for Arc<str> {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str(self)
  }
}

/// Macro to implement a `pub(crate)` conversion method into a foreign (taffy/parley) enum.
macro_rules! impl_from_taffy_enum {
  ($from_ty:ty, $method:ident -> $to_ty:ty, $($variant:ident),*) => {
    impl $from_ty {
      pub(crate) fn $method(self) -> $to_ty {
        match self {
          $(<$from_ty>::$variant => <$to_ty>::$variant,)*
        }
      }
    }
  };
}

pub(crate) use impl_from_taffy_enum;

/// Declares a CSS enum parser with automatic value list generation.
/// The first token in a `|` group is the canonical form; the rest parse as
/// aliases and serialize back to it.
macro_rules! declare_enum_from_css_impl {
  (
    $enum_type:ty,
    $($canonical:literal $(| $alias:literal)* => $variant:path),* $(,)?
  ) => {
    impl crate::style::MakeComputed for $enum_type {}

    impl<'i> crate::style::FromCss<'i> for $enum_type {
      const VALID_TOKENS: &'static [crate::style::CssToken] = &[
        $(
          crate::style::CssToken::Keyword($canonical)
          $(, crate::style::CssToken::Keyword($alias))*
        ),*
      ];

      fn from_css(input: &mut cssparser::Parser<'i, '_>) -> crate::style::ParseResult<'i, Self> {
        let location = input.current_source_location();
        let token = input.next()?;

        let cssparser::Token::Ident(ident) = token else {
          return Err($crate::style::unexpected_token!(location, token));
        };

        cssparser::match_ignore_ascii_case! {&ident,
          $(
            $canonical $(| $alias)* => Ok($variant),
          )*
          _ => Err($crate::style::unexpected_token!(location, token)),
        }
      }
    }

    impl crate::style::properties::ToCss for $enum_type {
      fn to_css<W: std::fmt::Write>(&self, dest: &mut W) -> std::fmt::Result {
        match self {
          $(
            $variant => dest.write_str($canonical),
          )*
        }
      }
    }

  };
}

pub(crate) use declare_enum_from_css_impl;

/// Declares a box-alignment enum parser that accepts the optional `safe`/`unsafe`
/// overflow-position prefix on its positional keywords.
macro_rules! declare_box_alignment_enum_impl {
  (
    $enum_type:ty,
    safe { $($safe_css:literal => $base_variant:ident / $safe_variant:ident),+ $(,)? },
    plain { $($plain_css:literal => $plain_variant:ident),* $(,)? }
  ) => {
    impl crate::style::MakeComputed for $enum_type {}

    impl<'i> crate::style::FromCss<'i> for $enum_type {
      const VALID_TOKENS: &'static [crate::style::CssToken] = &[
        $(crate::style::CssToken::Keyword($plain_css),)*
        $(crate::style::CssToken::Keyword($safe_css),)*
        crate::style::CssToken::Keyword("safe"),
        crate::style::CssToken::Keyword("unsafe"),
      ];

      fn from_css(input: &mut cssparser::Parser<'i, '_>) -> crate::style::ParseResult<'i, Self> {
        let mut safe = false;

        loop {
          let location = input.current_source_location();
          let token = input.next()?;

          let cssparser::Token::Ident(ident) = token else {
            return Err($crate::style::unexpected_token!(location, token));
          };

          cssparser::match_ignore_ascii_case! {&ident,
            "safe" => safe = true,
            "unsafe" => safe = false,
            $($safe_css => return Ok(if safe { Self::$safe_variant } else { Self::$base_variant }),)*
            $($plain_css => return if safe {
              Err($crate::style::unexpected_token!(location, token))
            } else {
              Ok(Self::$plain_variant)
            },)*
            _ => return Err($crate::style::unexpected_token!(location, token)),
          }
        }
      }
    }

    impl crate::style::properties::ToCss for $enum_type {
      fn to_css<W: std::fmt::Write>(&self, dest: &mut W) -> std::fmt::Result {
        match self {
          $(Self::$plain_variant => dest.write_str($plain_css),)*
          $(Self::$base_variant => dest.write_str($safe_css),)*
          $(Self::$safe_variant => {
            dest.write_str("safe ")?;
            dest.write_str($safe_css)
          })*
        }
      }
    }

  };
}

pub(crate) use declare_box_alignment_enum_impl;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    style::{BackgroundRepeat, BackgroundRepeatStyle},
    viewport::Viewport,
  };

  fn repeats(len: usize, style: BackgroundRepeatStyle) -> Vec<BackgroundRepeat> {
    vec![BackgroundRepeat(style, style); len]
  }

  fn interpolated(from_len: usize, to_len: usize) -> Option<Vec<BackgroundRepeat>> {
    let sizing = SizingContext::builder()
      .viewport(Viewport::default())
      .build();

    interpolate_list(
      &repeats(from_len, BackgroundRepeatStyle::Repeat),
      &repeats(to_len, BackgroundRepeatStyle::NoRepeat),
      0.75,
      &sizing,
      Color([0, 0, 0, 255]),
      |values| values,
    )
  }

  #[test]
  fn repeat_to_lcm_interpolates_under_the_cap() {
    assert_eq!(interpolated(2, 3).map(|values| values.len()), Some(6));
  }

  #[test]
  fn repeat_to_lcm_clamps_past_the_cap() {
    assert_eq!(
      interpolated(61, 67).map(|values| values.len()),
      Some(MAX_INTERPOLATED_LIST_LEN)
    );
  }
}
