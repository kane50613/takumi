use std::rc::Rc;

use cssparser::*;

use crate::{
  error::StyleSheetParseError,
  geometry::Size,
  style::{CalcArena, FromCss, Length, SizingContext},
  viewport::{MediaTarget, Viewport},
};

#[derive(Debug, Clone, PartialEq)]
enum MediaType {
  All,
  Screen,
  Print,
  Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaFeatureComparison {
  Equal,
  Min,
  Max,
  GreaterThan,
  LessThan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaOrientation {
  Portrait,
  Landscape,
}

#[derive(Debug, Clone, PartialEq)]
enum MediaFeature {
  Width(MediaFeatureComparison, Length),
  Height(MediaFeatureComparison, Length),
  Resolution(MediaFeatureComparison, f32),
  AspectRatio(MediaFeatureComparison, MediaRatio),
  Orientation(MediaOrientation),
}

/// A `<ratio>`, kept as written so a comparison can cross-multiply instead of
/// dividing twice.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MediaRatio {
  numerator: f32,
  denominator: f32,
}

/// A `<mf-value>`, read before the feature it belongs to is known: the range
/// context writes the value first in `(2dppx <= resolution)`.
#[derive(Debug, Clone, PartialEq)]
enum MediaFeatureValue {
  /// A resolution in dots per `px` unit.
  Resolution(f32),
  /// A ratio written with its slash.
  Ratio(MediaRatio),
  /// A bare number, a length for `width` and a ratio for `aspect-ratio`.
  Number(f32),
  Length(Length),
}

#[derive(Debug, Clone, PartialEq)]
struct MediaQuery {
  media_type: MediaType,
  features: Vec<MediaFeature>,
  negated: bool,
}

/// A comma-separated list of media queries, matching if any query matches.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaQueryList {
  queries: Vec<MediaQuery>,
}

impl MediaFeatureComparison {
  fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    let location = input.current_source_location();
    let token = input.next()?.clone();

    match token {
      Token::Delim('=') => Ok(Self::Equal),
      Token::Delim(direction @ ('<' | '>')) => {
        let or_equal = input.try_parse(|input| input.expect_delim('=')).is_ok();

        Ok(match (direction, or_equal) {
          ('<', true) => Self::Max,
          ('<', false) => Self::LessThan,
          (_, true) => Self::Min,
          (_, false) => Self::GreaterThan,
        })
      }
      _ => Err(location.new_unexpected_token_error(token)),
    }
  }

  /// The comparison with the feature name and the value swapped.
  fn flipped(self) -> Self {
    match self {
      Self::Equal => Self::Equal,
      Self::Min => Self::Max,
      Self::Max => Self::Min,
      Self::GreaterThan => Self::LessThan,
      Self::LessThan => Self::GreaterThan,
    }
  }

  fn is_upper_bound(self) -> bool {
    matches!(self, Self::Max | Self::LessThan)
  }
}

impl MediaFeatureValue {
  fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    if let Ok(resolution) = input.try_parse(parse_resolution) {
      return Ok(Self::Resolution(resolution));
    }

    if let Ok(number) = input.try_parse(Parser::expect_number) {
      if input.try_parse(|input| input.expect_delim('/')).is_ok() {
        let location = input.current_source_location();
        let divisor = input.expect_number()?;

        if divisor <= 0.0 {
          return Err(location.new_unexpected_token_error(Token::Number {
            has_sign: divisor < 0.0,
            value: divisor,
            int_value: None,
          }));
        }

        return Ok(Self::Ratio(MediaRatio {
          numerator: number,
          denominator: divisor,
        }));
      }

      return Ok(Self::Number(number));
    }

    Ok(Self::Length(
      Length::from_css(input).map_err(ParseError::into)?,
    ))
  }
}

impl MediaFeature {
  /// The boolean context, which asks whether the feature's value is non-zero.
  /// <https://drafts.csswg.org/mediaqueries-4/#mq-boolean-context>
  fn boolean(name: &str) -> Option<Self> {
    let comparison = MediaFeatureComparison::GreaterThan;

    if name.eq_ignore_ascii_case("resolution") {
      Some(Self::Resolution(comparison, 0.0))
    } else if name.eq_ignore_ascii_case("aspect-ratio") {
      Some(Self::AspectRatio(
        comparison,
        MediaRatio {
          numerator: 0.0,
          denominator: 1.0,
        },
      ))
    } else {
      Self::new(name, comparison, MediaFeatureValue::Number(0.0))
    }
  }

  fn new(name: &str, comparison: MediaFeatureComparison, value: MediaFeatureValue) -> Option<Self> {
    let length = match value {
      MediaFeatureValue::Length(length) => Some(length),
      MediaFeatureValue::Number(number) => Some(Length::Px(number)),
      _ => None,
    };

    if name.eq_ignore_ascii_case("width") {
      Some(Self::Width(comparison, length?))
    } else if name.eq_ignore_ascii_case("height") {
      Some(Self::Height(comparison, length?))
    } else if name.eq_ignore_ascii_case("resolution") {
      match value {
        MediaFeatureValue::Resolution(dppx) => Some(Self::Resolution(comparison, dppx)),
        _ => None,
      }
    } else if name.eq_ignore_ascii_case("aspect-ratio") {
      match value {
        MediaFeatureValue::Ratio(ratio) => Some(Self::AspectRatio(comparison, ratio)),
        MediaFeatureValue::Number(numerator) => Some(Self::AspectRatio(
          comparison,
          MediaRatio {
            numerator,
            denominator: 1.0,
          },
        )),
        _ => None,
      }
    } else {
      None
    }
  }

  fn matches(&self, viewport: Viewport, sizing: &SizingContext) -> bool {
    match self {
      Self::Width(comparison, value) => viewport.size.width.is_some_and(|width| {
        compare_media_feature(
          *comparison,
          width as f32,
          value.to_px(sizing, width as f32),
          LAYOUT_UNIT_EPSILON,
        )
      }),
      Self::Height(comparison, value) => viewport.size.height.is_some_and(|height| {
        compare_media_feature(
          *comparison,
          height as f32,
          value.to_px(sizing, height as f32),
          LAYOUT_UNIT_EPSILON,
        )
      }),
      Self::Resolution(comparison, dppx) => {
        compare_media_feature(*comparison, viewport.effective_dpr(), *dppx, 0.0)
      }
      Self::AspectRatio(comparison, ratio) => viewport
        .size
        .width
        .zip(viewport.size.height)
        .is_some_and(|(width, height)| {
          compare_media_feature(
            *comparison,
            width as f32 * ratio.denominator,
            height as f32 * ratio.numerator,
            LAYOUT_UNIT_EPSILON,
          )
        }),
      Self::Orientation(MediaOrientation::Portrait) => viewport
        .size
        .width
        .zip(viewport.size.height)
        .is_some_and(|(width, height)| height >= width),
      Self::Orientation(MediaOrientation::Landscape) => viewport
        .size
        .width
        .zip(viewport.size.height)
        .is_some_and(|(width, height)| width > height),
    }
  }
}

impl MediaQuery {
  /// The `not all` an unknown or malformed query is replaced by.
  /// <https://drafts.csswg.org/mediaqueries-4/#error-handling>
  fn not_all() -> Self {
    Self {
      media_type: MediaType::All,
      features: Vec::new(),
      negated: true,
    }
  }

  fn matches(&self, viewport: Viewport, sizing: &SizingContext) -> bool {
    let media_type_matches = match &self.media_type {
      MediaType::All => true,
      MediaType::Screen => viewport.media_target == MediaTarget::Screen,
      MediaType::Print => viewport.media_target == MediaTarget::Print,
      MediaType::Unsupported(_) => false,
    };

    let mut is_match = media_type_matches
      && self
        .features
        .iter()
        .all(|feature| feature.matches(viewport, sizing));

    if self.negated {
      is_match = !is_match;
    }

    is_match
  }
}

impl MediaQueryList {
  pub(crate) fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    Ok(Self {
      queries: input.parse_comma_separated(|input| {
        let query = input
          .try_parse(parse_media_query)
          .ok()
          .filter(|_| input.is_exhausted())
          .unwrap_or_else(MediaQuery::not_all);

        skip_malformed_query(input)?;

        Ok(query)
      })?,
    })
  }

  /// Whether any query matches the viewport; empty lists always match.
  pub fn matches(&self, viewport: Viewport) -> bool {
    if self.queries.is_empty() {
      return true;
    }

    let sizing = SizingContext {
      viewport,
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: viewport.font_size,
      root_font_size: None,
      line_height: viewport.font_size,
      root_line_height: Some(viewport.font_size),
      calc_arena: Rc::new(CalcArena::default()),
    };

    self
      .queries
      .iter()
      .any(|query| query.matches(viewport, &sizing))
  }
}

/// Consumes what is left of a query that parsed as `not all`. A block or a
/// stray closing delimiter means the text was never a prelude, which is how a
/// caller assembling CSS from strings catches a rule smuggled into one.
fn skip_malformed_query<'i>(
  input: &mut Parser<'i, '_>,
) -> Result<(), ParseError<'i, StyleSheetParseError>> {
  loop {
    let location = input.current_source_location();
    let Ok(token) = input.next() else {
      return Ok(());
    };

    if matches!(
      token,
      Token::CurlyBracketBlock
        | Token::CloseCurlyBracket
        | Token::CloseParenthesis
        | Token::CloseSquareBracket
    ) {
      let token = token.clone();

      return Err(location.new_unexpected_token_error(token));
    }
  }
}

/// A `<resolution>` in dots per `px` unit. A `dpcm` value is rounded to two
/// decimals, as Blink does; Blink rounds the device pixel ratio the same way
/// before comparing, which this does not, so the two disagree when that ratio
/// carries more than two decimals.
fn parse_resolution<'i>(
  input: &mut Parser<'i, '_>,
) -> Result<f32, ParseError<'i, StyleSheetParseError>> {
  const DPCM_PER_DPPX: f32 = 96.0 / 2.54;

  let location = input.current_source_location();
  let token = input.next()?.clone();

  if let Token::Dimension {
    value, ref unit, ..
  } = token
  {
    let dppx = match_ignore_ascii_case! { unit.as_ref(),
      "dppx" | "x" => Some(value),
      "dpi" => Some(value / 96.0),
      "dpcm" => Some(((value / DPCM_PER_DPPX) * 100.0).round() / 100.0),
      _ => None,
    };

    if let Some(dppx) = dppx {
      return Ok(dppx);
    }
  }

  Err(location.new_unexpected_token_error(token))
}

/// Blink compares lengths against `LayoutUnit::Epsilon()`, the step of the grid
/// it rounds layout onto.
/// <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/media_query_evaluator.cc>
const LAYOUT_UNIT_EPSILON: f32 = 1.0 / 64.0;

fn compare_media_feature(
  comparison: MediaFeatureComparison,
  actual: f32,
  expected: f32,
  tolerance: f32,
) -> bool {
  // <https://drafts.csswg.org/mediaqueries-4/#false-in-the-negative-range>
  if expected < 0.0 {
    return matches!(
      comparison,
      MediaFeatureComparison::Min | MediaFeatureComparison::GreaterThan
    );
  }

  match comparison {
    MediaFeatureComparison::Equal => (actual - expected).abs() <= tolerance,
    MediaFeatureComparison::Min => actual >= expected - tolerance,
    MediaFeatureComparison::Max => actual <= expected + tolerance,
    MediaFeatureComparison::GreaterThan => actual > expected,
    MediaFeatureComparison::LessThan => actual < expected,
  }
}

fn parse_media_query<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<MediaQuery, ParseError<'i, StyleSheetParseError>> {
  let mut negated = false;
  let mut media_type = MediaType::All;
  let mut features = Vec::new();
  let mut has_explicit_media_type = false;

  if let Ok(keyword) = input.try_parse(Parser::expect_ident_cloned) {
    if keyword.eq_ignore_ascii_case("not") {
      negated = true;
    } else if !keyword.eq_ignore_ascii_case("only") {
      media_type = parse_media_type(keyword);
      has_explicit_media_type = true;
    }

    // A `not`/`only` modifier may be followed by an optional media type.
    if !has_explicit_media_type && let Ok(name) = input.try_parse(Parser::expect_ident_cloned) {
      media_type = parse_media_type(name);
      has_explicit_media_type = true;
    }
  }

  if input
    .try_parse(|input| parse_media_feature_block(input, &mut features))
    .is_ok()
    || has_explicit_media_type
  {
    while input
      .try_parse(|input| input.expect_ident_matching("and"))
      .is_ok()
    {
      parse_media_feature_block(input, &mut features)?;
    }
  }

  Ok(MediaQuery {
    media_type,
    features,
    negated,
  })
}

fn parse_media_type(name: CowRcStr<'_>) -> MediaType {
  if name.eq_ignore_ascii_case("all") {
    MediaType::All
  } else if name.eq_ignore_ascii_case("screen") {
    MediaType::Screen
  } else if name.eq_ignore_ascii_case("print") {
    MediaType::Print
  } else {
    MediaType::Unsupported(name.to_string())
  }
}

fn parse_media_feature_block<'i, 't>(
  input: &mut Parser<'i, 't>,
  features: &mut Vec<MediaFeature>,
) -> Result<(), ParseError<'i, StyleSheetParseError>> {
  let location = input.current_source_location();
  let token = input.next()?;
  match token {
    Token::ParenthesisBlock => input.parse_nested_block(|input| {
      if let Ok((lower, upper)) = input.try_parse(parse_media_feature_range) {
        features.push(lower);
        features.extend(upper);
        return Ok(());
      }

      features.push(parse_media_feature(input)?);
      Ok(())
    }),
    _ => Err(location.new_unexpected_token_error(token.clone())),
  }
}

fn parse_media_feature<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<MediaFeature, ParseError<'i, StyleSheetParseError>> {
  let feature_name = input.expect_ident_cloned()?;

  // Boolean context: the feature name alone matches when its value is non-zero.
  // <https://drafts.csswg.org/mediaqueries-4/#mq-boolean-context>
  if input.try_parse(Parser::expect_colon).is_err() {
    return MediaFeature::boolean(&feature_name)
      .ok_or_else(|| input.new_custom_error(StyleSheetParseError::unsupported_media_feature()));
  }

  if feature_name.eq_ignore_ascii_case("orientation") {
    let orientation = input.expect_ident_cloned()?;
    return if orientation.eq_ignore_ascii_case("portrait") {
      Ok(MediaFeature::Orientation(MediaOrientation::Portrait))
    } else if orientation.eq_ignore_ascii_case("landscape") {
      Ok(MediaFeature::Orientation(MediaOrientation::Landscape))
    } else {
      Err(
        input.new_error(BasicParseErrorKind::UnexpectedToken(Token::Ident(
          orientation,
        ))),
      )
    };
  }

  let (comparison, name) = match feature_name.split_at_checked("min-".len()) {
    Some((prefix, name)) if prefix.eq_ignore_ascii_case("min-") => {
      (MediaFeatureComparison::Min, name)
    }
    Some((prefix, name)) if prefix.eq_ignore_ascii_case("max-") => {
      (MediaFeatureComparison::Max, name)
    }
    _ => (MediaFeatureComparison::Equal, &*feature_name),
  };

  let value = MediaFeatureValue::parse(input)?;

  MediaFeature::new(name, comparison, value)
    .ok_or_else(|| input.new_custom_error(StyleSheetParseError::unsupported_media_feature()))
}

/// The range context of Media Queries Level 4, such as `(width >= 40em)` and
/// `(400px < height <= 700px)`.
/// <https://drafts.csswg.org/mediaqueries-4/#mq-range-context>
fn parse_media_feature_range<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<(MediaFeature, Option<MediaFeature>), ParseError<'i, StyleSheetParseError>> {
  let feature = |input: &mut Parser<'i, 't>, name: &str, comparison, value| {
    MediaFeature::new(name, comparison, value)
      .ok_or_else(|| input.new_custom_error(StyleSheetParseError::unsupported_media_feature()))
  };

  if let Ok(name) = input.try_parse(Parser::expect_ident_cloned) {
    let comparison = MediaFeatureComparison::parse(input)?;
    let value = MediaFeatureValue::parse(input)?;

    return Ok((feature(input, &name, comparison, value)?, None));
  }

  let lower_value = MediaFeatureValue::parse(input)?;
  let lower = MediaFeatureComparison::parse(input)?.flipped();
  let name = input.expect_ident_cloned()?;
  let lower_feature = feature(input, &name, lower, lower_value)?;

  if input.is_exhausted() {
    return Ok((lower_feature, None));
  }

  let upper = MediaFeatureComparison::parse(input)?;

  if lower.is_upper_bound() == upper.is_upper_bound() {
    return Err(input.new_custom_error(StyleSheetParseError::invalid_reason(
      "media range comparisons must point the same way",
    )));
  }

  let upper_value = MediaFeatureValue::parse(input)?;

  Ok((
    lower_feature,
    Some(feature(input, &name, upper, upper_value)?),
  ))
}
