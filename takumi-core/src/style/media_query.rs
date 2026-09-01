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

impl MediaType {
  /// The media type a `<media-type>` ident names.
  fn from_name(name: CowRcStr<'_>) -> Self {
    match_ignore_ascii_case! { name.as_ref(),
      "all" => Self::All,
      "screen" => Self::Screen,
      "print" => Self::Print,
      _ => Self::Unsupported(name.to_string()),
    }
  }
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

/// A `<media-condition>`.
/// <https://drafts.csswg.org/mediaqueries-4/#media-condition>
#[derive(Debug, Clone, PartialEq)]
enum MediaCondition {
  Feature(MediaFeature),
  Not(Box<MediaCondition>),
  And(Vec<MediaCondition>),
  Or(Vec<MediaCondition>),
}

#[derive(Debug, Clone, PartialEq)]
struct MediaQuery {
  media_type: MediaType,
  condition: Option<MediaCondition>,
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

  fn matches(self, actual: f32, expected: f32, tolerance: f32) -> bool {
    // <https://drafts.csswg.org/mediaqueries-4/#false-in-the-negative-range>
    if expected < 0.0 {
      return matches!(self, Self::Min | MediaFeatureComparison::GreaterThan);
    }

    match self {
      Self::Equal => (actual - expected).abs() <= tolerance,
      Self::Min => actual >= expected - tolerance,
      Self::Max => actual <= expected + tolerance,
      Self::GreaterThan => actual > expected,
      Self::LessThan => actual < expected,
    }
  }
}

impl MediaFeatureValue {
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

  fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    if let Ok(resolution) = input.try_parse(Self::parse_resolution) {
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

    match_ignore_ascii_case! { name,
      "resolution" => Some(Self::Resolution(comparison, 0.0)),
      "aspect-ratio" => Some(Self::AspectRatio(
        comparison,
        MediaRatio {
          numerator: 0.0,
          denominator: 1.0,
        },
      )),
      _ => Self::new(name, comparison, MediaFeatureValue::Number(0.0)),
    }
  }

  fn new(name: &str, comparison: MediaFeatureComparison, value: MediaFeatureValue) -> Option<Self> {
    let length = match value {
      MediaFeatureValue::Length(length) => Some(length),
      MediaFeatureValue::Number(number) => Some(Length::Px(number)),
      _ => None,
    };

    match_ignore_ascii_case! { name,
      "width" => Some(Self::Width(comparison, length?)),
      "height" => Some(Self::Height(comparison, length?)),
      "resolution" => match value {
        MediaFeatureValue::Resolution(dppx) => Some(Self::Resolution(comparison, dppx)),
        _ => None,
      },
      "aspect-ratio" => match value {
        MediaFeatureValue::Ratio(ratio) => Some(Self::AspectRatio(comparison, ratio)),
        MediaFeatureValue::Number(numerator) => Some(Self::AspectRatio(
          comparison,
          MediaRatio {
            numerator,
            denominator: 1.0,
          },
        )),
        _ => None,
      },
      _ => None,
    }
  }

  fn parse<'i, 't>(
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
      return match_ignore_ascii_case! { orientation.as_ref(),
        "portrait" => Ok(Self::Orientation(MediaOrientation::Portrait)),
        "landscape" => Ok(Self::Orientation(MediaOrientation::Landscape)),
        _ => Err(
          input.new_error(BasicParseErrorKind::UnexpectedToken(Token::Ident(
            orientation.clone(),
          ))),
        ),
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
  fn parse_range<'i, 't>(
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

  fn matches(&self, viewport: Viewport, sizing: &SizingContext) -> bool {
    match self {
      Self::Width(comparison, value) => viewport.size.width.is_some_and(|width| {
        comparison.matches(
          width as f32,
          value.to_px(sizing, width as f32),
          LAYOUT_UNIT_EPSILON,
        )
      }),
      Self::Height(comparison, value) => viewport.size.height.is_some_and(|height| {
        comparison.matches(
          height as f32,
          value.to_px(sizing, height as f32),
          LAYOUT_UNIT_EPSILON,
        )
      }),
      Self::Resolution(comparison, dppx) => {
        comparison.matches(viewport.effective_dpr(), *dppx, 0.0)
      }
      Self::AspectRatio(comparison, ratio) => viewport
        .size
        .width
        .zip(viewport.size.height)
        .is_some_and(|(width, height)| {
          comparison.matches(
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

impl MediaCondition {
  fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    Self::parse_with_or(input, true)
  }

  /// The `and`-only form a media type takes, which cannot carry a bare `or`.
  fn parse_without_or<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    Self::parse_with_or(input, false)
  }

  fn parse_with_or<'i, 't>(
    input: &mut Parser<'i, 't>,
    allow_or: bool,
  ) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    if input
      .try_parse(|input| input.expect_ident_matching("not"))
      .is_ok()
    {
      return Ok(Self::Not(Box::new(Self::parse_in_parens(input)?)));
    }

    let first = Self::parse_in_parens(input)?;

    if input
      .try_parse(|input| input.expect_ident_matching("and"))
      .is_ok()
    {
      return Ok(Self::And(Self::parse_operands(input, first, "and")?));
    }

    if allow_or
      && input
        .try_parse(|input| input.expect_ident_matching("or"))
        .is_ok()
    {
      return Ok(Self::Or(Self::parse_operands(input, first, "or")?));
    }

    Ok(first)
  }

  /// The operands of a chain whose first keyword the caller has consumed.
  fn parse_operands<'i, 't>(
    input: &mut Parser<'i, 't>,
    first: Self,
    keyword: &str,
  ) -> Result<Vec<Self>, ParseError<'i, StyleSheetParseError>> {
    let mut operands = vec![first, Self::parse_in_parens(input)?];

    while input
      .try_parse(|input| input.expect_ident_matching(keyword))
      .is_ok()
    {
      operands.push(Self::parse_in_parens(input)?);
    }

    Ok(operands)
  }

  /// `( <media-condition> ) | ( <media-feature> )`
  fn parse_in_parens<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    let location = input.current_source_location();
    let token = input.next()?.clone();

    if token != Token::ParenthesisBlock {
      return Err(location.new_unexpected_token_error(token));
    }

    input.parse_nested_block(|input| {
      if let Ok((lower, upper)) = input.try_parse(MediaFeature::parse_range) {
        let lower = Self::Feature(lower);

        return Ok(match upper {
          Some(upper) => Self::And(vec![lower, Self::Feature(upper)]),
          None => lower,
        });
      }

      if let Ok(feature) = input.try_parse(MediaFeature::parse) {
        return Ok(Self::Feature(feature));
      }

      Self::parse(input)
    })
  }

  fn matches(&self, viewport: Viewport, sizing: &SizingContext) -> bool {
    match self {
      Self::Feature(feature) => feature.matches(viewport, sizing),
      Self::Not(condition) => !condition.matches(viewport, sizing),
      Self::And(conditions) => conditions
        .iter()
        .all(|condition| condition.matches(viewport, sizing)),
      Self::Or(conditions) => conditions
        .iter()
        .any(|condition| condition.matches(viewport, sizing)),
    }
  }
}

impl MediaQuery {
  fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<MediaQuery, ParseError<'i, StyleSheetParseError>> {
    if let Ok(query) = input.try_parse(Self::parse_with_media_type) {
      return Ok(query);
    }

    Ok(Self {
      media_type: MediaType::All,
      condition: Some(MediaCondition::parse(input)?),
      negated: false,
    })
  }

  /// `[not | only]? <media-type> [and <media-condition-without-or>]?`
  fn parse_with_media_type<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<MediaQuery, ParseError<'i, StyleSheetParseError>> {
    let keyword = input.expect_ident_cloned()?;
    let mut negated = false;
    let name = if keyword.eq_ignore_ascii_case("not") {
      negated = true;
      input.expect_ident_cloned()?
    } else if keyword.eq_ignore_ascii_case("only") {
      input.expect_ident_cloned()?
    } else {
      keyword
    };
    let condition = input
      .try_parse(|input| input.expect_ident_matching("and"))
      .is_ok()
      .then(|| MediaCondition::parse_without_or(input))
      .transpose()?;

    Ok(Self {
      media_type: MediaType::from_name(name),
      condition,
      negated,
    })
  }

  /// The `not all` an unknown or malformed query is replaced by.
  /// <https://drafts.csswg.org/mediaqueries-4/#error-handling>
  fn not_all() -> Self {
    Self {
      media_type: MediaType::All,
      condition: None,
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
        .condition
        .as_ref()
        .is_none_or(|condition| condition.matches(viewport, sizing));

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
          .try_parse(MediaQuery::parse)
          .ok()
          .filter(|_| input.is_exhausted())
          .unwrap_or_else(MediaQuery::not_all);

        Self::skip_malformed_query(input)?;

        Ok(query)
      })?,
    })
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

/// Blink compares lengths against `LayoutUnit::Epsilon()`, the step of the grid
/// it rounds layout onto.
/// <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/media_query_evaluator.cc>
const LAYOUT_UNIT_EPSILON: f32 = 1.0 / 64.0;
