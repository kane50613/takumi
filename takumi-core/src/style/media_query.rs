use std::rc::Rc;

use cssparser::*;

use crate::{
  error::StyleSheetParseError,
  geometry::Size,
  style::{CalcArena, FromCss, Length, SizingContext},
  viewport::Viewport,
};

#[derive(Debug, Clone, PartialEq)]
enum MediaType {
  All,
  Screen,
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
  Orientation(MediaOrientation),
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

impl MediaFeature {
  fn sized(name: &str, comparison: MediaFeatureComparison, length: Length) -> Option<Self> {
    if name.eq_ignore_ascii_case("width") {
      Some(Self::Width(comparison, length))
    } else if name.eq_ignore_ascii_case("height") {
      Some(Self::Height(comparison, length))
    } else {
      None
    }
  }

  fn matches(&self, viewport: Viewport, sizing: &SizingContext) -> bool {
    match self {
      Self::Width(comparison, value) => viewport.size.width.is_some_and(|width| {
        compare_media_feature(*comparison, width as f32, value.to_px(sizing, width as f32))
      }),
      Self::Height(comparison, value) => viewport.size.height.is_some_and(|height| {
        compare_media_feature(
          *comparison,
          height as f32,
          value.to_px(sizing, height as f32),
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
      MediaType::All | MediaType::Screen => true,
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

        while input.next().is_ok() {}

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

fn compare_media_feature(comparison: MediaFeatureComparison, actual: f32, expected: f32) -> bool {
  const MEDIA_FEATURE_EQUALITY_TOLERANCE: f32 = 0.5;

  match comparison {
    MediaFeatureComparison::Equal => (actual - expected).abs() <= MEDIA_FEATURE_EQUALITY_TOLERANCE,
    MediaFeatureComparison::Min => actual >= expected,
    MediaFeatureComparison::Max => actual <= expected,
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
    return MediaFeature::sized(
      &feature_name,
      MediaFeatureComparison::GreaterThan,
      Length::Px(0.0),
    )
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

  let comparison = if feature_name.eq_ignore_ascii_case("min-width")
    || feature_name.eq_ignore_ascii_case("min-height")
  {
    MediaFeatureComparison::Min
  } else if feature_name.eq_ignore_ascii_case("max-width")
    || feature_name.eq_ignore_ascii_case("max-height")
  {
    MediaFeatureComparison::Max
  } else {
    MediaFeatureComparison::Equal
  };

  let length = Length::from_css(input).map_err(ParseError::into)?;
  let name = feature_name
    .split_at_checked("min-".len())
    .filter(|(prefix, _)| {
      prefix.eq_ignore_ascii_case("min-") || prefix.eq_ignore_ascii_case("max-")
    })
    .map_or(&*feature_name, |(_, name)| name);

  MediaFeature::sized(name, comparison, length)
    .ok_or_else(|| input.new_custom_error(StyleSheetParseError::unsupported_media_feature()))
}

/// The range context of Media Queries Level 4, such as `(width >= 40em)` and
/// `(400px < height <= 700px)`.
/// <https://drafts.csswg.org/mediaqueries-4/#mq-range-context>
fn parse_media_feature_range<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<(MediaFeature, Option<MediaFeature>), ParseError<'i, StyleSheetParseError>> {
  let sized = |input: &mut Parser<'i, 't>, name: &str, comparison, length| {
    MediaFeature::sized(name, comparison, length)
      .ok_or_else(|| input.new_custom_error(StyleSheetParseError::unsupported_media_feature()))
  };

  if let Ok(name) = input.try_parse(Parser::expect_ident_cloned) {
    let comparison = MediaFeatureComparison::parse(input)?;
    let length = Length::from_css(input).map_err(ParseError::into)?;

    return Ok((sized(input, &name, comparison, length)?, None));
  }

  let lower_length = Length::from_css(input).map_err(ParseError::into)?;
  let lower = MediaFeatureComparison::parse(input)?.flipped();
  let name = input.expect_ident_cloned()?;
  let lower_feature = sized(input, &name, lower, lower_length)?;

  if input.is_exhausted() {
    return Ok((lower_feature, None));
  }

  let upper = MediaFeatureComparison::parse(input)?;

  if lower.is_upper_bound() == upper.is_upper_bound() {
    return Err(input.new_custom_error(StyleSheetParseError::invalid_reason(
      "media range comparisons must point the same way",
    )));
  }

  let upper_length = Length::from_css(input).map_err(ParseError::into)?;

  Ok((
    lower_feature,
    Some(sized(input, &name, upper, upper_length)?),
  ))
}
