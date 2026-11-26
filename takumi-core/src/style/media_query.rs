use cssparser::ParseError as CssParseError;

use cssparser::*;

use crate::{
  error::{StyleSheetParseError, StyleSheetParseErrorKind},
  geometry::{LAYOUT_UNIT_EPSILON, Size},
  style::{ContainerName, FromCss, Length, SizingContext},
  viewport::{ColorScheme, MediaTarget, Viewport},
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

/// A media feature about the user's settings, which no render sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaPreference {
  ReducedMotion,
  Contrast,
  ForcedColors,
  InvertedColors,
}

impl MediaPreference {
  const NAMES: [(&'static str, Self); 4] = [
    ("prefers-reduced-motion", Self::ReducedMotion),
    ("prefers-contrast", Self::Contrast),
    ("forced-colors", Self::ForcedColors),
    ("inverted-colors", Self::InvertedColors),
  ];

  fn from_name(name: &str) -> Option<Self> {
    Self::NAMES
      .iter()
      .find(|(preference_name, _)| preference_name.eq_ignore_ascii_case(name))
      .map(|&(_, preference)| preference)
  }

  /// The values it takes, the first being what a default render has.
  /// <https://drafts.csswg.org/mediaqueries-5/#mf-user-preferences>
  fn values(self) -> &'static [&'static str] {
    match self {
      Self::ReducedMotion => &["no-preference", "reduce"],
      Self::Contrast => &["no-preference", "more", "less", "custom"],
      Self::ForcedColors => &["none", "active"],
      Self::InvertedColors => &["none", "inverted"],
    }
  }

  /// The value `value` names, in the form `values` lists it.
  fn value(self, value: &str) -> Option<&'static str> {
    self
      .values()
      .iter()
      .copied()
      .find(|known| known.eq_ignore_ascii_case(value))
  }
}

#[derive(Debug, Clone, PartialEq)]
enum MediaFeature {
  Width(MediaFeatureComparison, Length),
  Height(MediaFeatureComparison, Length),
  /// `inline-size`, which only a container query reads.
  InlineSize(MediaFeatureComparison, Length),
  /// `block-size`, which only a container query reads.
  BlockSize(MediaFeatureComparison, Length),
  Resolution(MediaFeatureComparison, f32),
  AspectRatio(MediaFeatureComparison, MediaRatio),
  Orientation(MediaOrientation),
  /// The scheme to match, or none for the boolean context, which every scheme matches.
  PrefersColorScheme(Option<ColorScheme>),
  /// A user preference, which a render answers with its default, or the boolean context for none.
  Preference(MediaPreference, Option<&'static str>),
  /// A `<general-enclosed>`: a feature or condition this parser does not know, which never holds.
  Unknown,
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
  fn parse<'i>(input: &mut Parser<'i>) -> Result<Self, ParseError<StyleSheetParseError>> {
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
      _ => Err(CssParseError::unexpected_token()),
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
      return matches!(self, Self::Min | Self::GreaterThan);
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
  /// A `<resolution>` in dots per `px`, `dpcm` rounded as Blink rounds it.
  fn parse_resolution<'i>(input: &mut Parser<'i>) -> Result<f32, ParseError<StyleSheetParseError>> {
    const DPCM_PER_DPPX: f32 = 96.0 / 2.54;

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

    Err(CssParseError::unexpected_token())
  }

  fn parse<'i>(input: &mut Parser<'i>) -> Result<Self, ParseError<StyleSheetParseError>> {
    if let Ok(resolution) = input.try_parse(Self::parse_resolution) {
      return Ok(Self::Resolution(resolution));
    }

    if let Ok(number) = input.try_parse(Parser::expect_number) {
      if input.try_parse(|input| input.expect_delim('/')).is_ok() {
        let divisor = input.expect_number()?;

        if divisor <= 0.0 {
          return Err(CssParseError::unexpected_token());
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

    if let Some(preference) = MediaPreference::from_name(name) {
      return Some(Self::Preference(preference, None));
    }

    match_ignore_ascii_case! { name,
      "resolution" => Some(Self::Resolution(comparison, 0.0)),
      "prefers-color-scheme" => Some(Self::PrefersColorScheme(None)),
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
    // A size feature takes a `<length>`, which neither `auto` nor a percentage is.
    let length = match value {
      MediaFeatureValue::Length(length) if length != Length::Auto && !length.has_percentage() => {
        Some(length)
      }
      MediaFeatureValue::Number(number) => Some(Length::Px(number)),
      _ => None,
    };

    match_ignore_ascii_case! { name,
      "width" => Some(Self::Width(comparison, length?)),
      "height" => Some(Self::Height(comparison, length?)),
      "inline-size" => Some(Self::InlineSize(comparison, length?)),
      "block-size" => Some(Self::BlockSize(comparison, length?)),
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

  /// The feature `name` names, or the unsupported-feature error when `value` does not fit it.
  fn from_value(
    name: &str,
    comparison: MediaFeatureComparison,
    value: MediaFeatureValue,
  ) -> Result<Self, ParseError<StyleSheetParseError>> {
    Self::new(name, comparison, value)
      .ok_or_else(|| CssParseError::custom(StyleSheetParseErrorKind::UnsupportedMediaFeature))
  }

  fn parse<'i>(input: &mut Parser<'i>) -> Result<MediaFeature, ParseError<StyleSheetParseError>> {
    let feature_name = input.expect_ident_cloned()?;

    // Boolean context: the feature name alone matches when its value is non-zero.
    // <https://drafts.csswg.org/mediaqueries-4/#mq-boolean-context>
    if input.try_parse(Parser::expect_colon).is_err() {
      return MediaFeature::boolean(&feature_name)
        .ok_or_else(|| CssParseError::custom(StyleSheetParseErrorKind::UnsupportedMediaFeature));
    }

    if feature_name.eq_ignore_ascii_case("prefers-color-scheme") {
      let scheme = input.expect_ident_cloned()?;
      return match_ignore_ascii_case! { scheme.as_ref(),
        "light" => Ok(Self::PrefersColorScheme(Some(ColorScheme::Light))),
        "dark" => Ok(Self::PrefersColorScheme(Some(ColorScheme::Dark))),
        _ => Err(
          CssParseError::from_basic_kind(BasicParseErrorKind::UnexpectedToken),
        ),
      };
    }

    if let Some(preference) = MediaPreference::from_name(&feature_name) {
      let value = input.expect_ident_cloned()?;

      return preference
        .value(&value)
        .map(|value| Self::Preference(preference, Some(value)))
        .ok_or_else(|| CssParseError::from_basic_kind(BasicParseErrorKind::UnexpectedToken));
    }

    if feature_name.eq_ignore_ascii_case("orientation") {
      let orientation = input.expect_ident_cloned()?;
      return match_ignore_ascii_case! { orientation.as_ref(),
        "portrait" => Ok(Self::Orientation(MediaOrientation::Portrait)),
        "landscape" => Ok(Self::Orientation(MediaOrientation::Landscape)),
        _ => Err(
          CssParseError::from_basic_kind(BasicParseErrorKind::UnexpectedToken),
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

    Self::from_value(name, comparison, value)
  }

  /// The range context of Media Queries Level 4, such as `(width >= 40em)` and
  /// `(400px < height <= 700px)`.
  /// <https://drafts.csswg.org/mediaqueries-4/#mq-range-context>
  fn parse_range<'i>(
    input: &mut Parser<'i>,
  ) -> Result<(MediaFeature, Option<MediaFeature>), ParseError<StyleSheetParseError>> {
    if let Ok(name) = input.try_parse(Parser::expect_ident_cloned) {
      let comparison = MediaFeatureComparison::parse(input)?;
      let value = MediaFeatureValue::parse(input)?;

      return Ok((Self::from_value(&name, comparison, value)?, None));
    }

    let lower_value = MediaFeatureValue::parse(input)?;
    let lower = MediaFeatureComparison::parse(input)?.flipped();
    let name = input.expect_ident_cloned()?;
    let lower_feature = Self::from_value(&name, lower, lower_value)?;

    if input.is_exhausted() {
      return Ok((lower_feature, None));
    }

    let upper = MediaFeatureComparison::parse(input)?;

    if lower.is_upper_bound() == upper.is_upper_bound() {
      return Err(CssParseError::custom(StyleSheetParseError::invalid_reason(
        "media range comparisons must point the same way",
      )));
    }

    let upper_value = MediaFeatureValue::parse(input)?;

    Ok((
      lower_feature,
      Some(Self::from_value(&name, upper, upper_value)?),
    ))
  }

  /// Whether it holds of a query target `size` across, the viewport's or a container's.
  fn matches(&self, viewport: Viewport, size: Size<Option<f32>>, sizing: &SizingContext) -> bool {
    let length = |comparison: &MediaFeatureComparison, value: &Length, axis: Option<f32>| {
      axis.is_some_and(|axis| {
        comparison.matches(axis, value.to_px(sizing, axis), LAYOUT_UNIT_EPSILON)
      })
    };

    match self {
      Self::Width(comparison, value) | Self::InlineSize(comparison, value) => {
        length(comparison, value, size.width)
      }
      Self::Height(comparison, value) | Self::BlockSize(comparison, value) => {
        length(comparison, value, size.height)
      }
      Self::Resolution(comparison, dppx) => {
        comparison.matches(viewport.effective_dpr(), *dppx, 0.0)
      }
      Self::AspectRatio(comparison, ratio) => {
        size.width.zip(size.height).is_some_and(|(width, height)| {
          comparison.matches(
            width * ratio.denominator,
            height * ratio.numerator,
            LAYOUT_UNIT_EPSILON,
          )
        })
      }
      Self::Orientation(orientation) => {
        size
          .width
          .zip(size.height)
          .is_some_and(|(width, height)| match orientation {
            MediaOrientation::Portrait => height >= width,
            MediaOrientation::Landscape => width > height,
          })
      }
      Self::PrefersColorScheme(scheme) => {
        scheme.is_none_or(|scheme| viewport.color_scheme == scheme)
      }
      Self::Preference(preference, value) => {
        let default = preference.values()[0];

        value.is_some_and(|value| value == default)
      }
      Self::Unknown => false,
    }
  }
}

impl MediaCondition {
  fn parse<'i>(input: &mut Parser<'i>) -> Result<Self, ParseError<StyleSheetParseError>> {
    Self::parse_with_or(input, true)
  }

  /// The `and`-only form a media type takes, which cannot carry a bare `or`.
  fn parse_without_or<'i>(
    input: &mut Parser<'i>,
  ) -> Result<Self, ParseError<StyleSheetParseError>> {
    Self::parse_with_or(input, false)
  }

  fn parse_with_or<'i>(
    input: &mut Parser<'i>,
    allow_or: bool,
  ) -> Result<Self, ParseError<StyleSheetParseError>> {
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
  fn parse_operands<'i>(
    input: &mut Parser<'i>,
    first: Self,
    keyword: &str,
  ) -> Result<Vec<Self>, ParseError<StyleSheetParseError>> {
    let mut operands = vec![first, Self::parse_in_parens(input)?];

    while input
      .try_parse(|input| input.expect_ident_matching(keyword))
      .is_ok()
    {
      operands.push(Self::parse_in_parens(input)?);
    }

    Ok(operands)
  }

  /// `( <media-condition> ) | ( <media-feature> ) | <general-enclosed>`, as Blink's
  /// `ConditionalParser::ConsumeInner` falls back to `ConsumeGeneralEnclosed`.
  fn parse_in_parens<'i>(input: &mut Parser<'i>) -> Result<Self, ParseError<StyleSheetParseError>> {
    let token = input.next()?.clone();

    if let Token::Function(_) = token {
      return input.parse_nested_block(|input| {
        while input.next().is_ok() {}
        Ok(Self::Feature(MediaFeature::Unknown))
      });
    }

    if token != Token::ParenthesisBlock {
      return Err(CssParseError::unexpected_token());
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

      if let Ok(condition) = input.try_parse(|input| {
        let condition = Self::parse(input)?;

        input.expect_exhausted()?;
        Ok::<_, ParseError<StyleSheetParseError>>(condition)
      }) {
        return Ok(condition);
      }

      while input.next().is_ok() {}
      Ok(Self::Feature(MediaFeature::Unknown))
    })
  }

  fn matches(&self, viewport: Viewport, size: Size<Option<f32>>, sizing: &SizingContext) -> bool {
    match self {
      Self::Feature(feature) => feature.matches(viewport, size, sizing),
      Self::Not(condition) => !condition.matches(viewport, size, sizing),
      Self::And(conditions) => conditions
        .iter()
        .all(|condition| condition.matches(viewport, size, sizing)),
      Self::Or(conditions) => conditions
        .iter()
        .any(|condition| condition.matches(viewport, size, sizing)),
    }
  }

  /// Whether a feature in it satisfies `test`.
  fn any_feature(&self, test: &impl Fn(&MediaFeature) -> bool) -> bool {
    match self {
      Self::Feature(feature) => test(feature),
      Self::Not(condition) => condition.any_feature(test),
      Self::And(conditions) | Self::Or(conditions) => conditions
        .iter()
        .any(|condition| condition.any_feature(test)),
    }
  }
}

impl MediaQuery {
  fn parse<'i>(input: &mut Parser<'i>) -> Result<MediaQuery, ParseError<StyleSheetParseError>> {
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
  fn parse_with_media_type<'i>(
    input: &mut Parser<'i>,
  ) -> Result<MediaQuery, ParseError<StyleSheetParseError>> {
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

    let is_match = media_type_matches
      && self
        .condition
        .as_ref()
        .is_none_or(|condition| condition.matches(viewport, viewport_size(viewport), sizing));

    is_match != self.negated
  }
}

/// The viewport's size, a media query's target.
fn viewport_size(viewport: Viewport) -> Size<Option<f32>> {
  Size {
    width: viewport.size.width.map(|width| width as f32),
    height: viewport.size.height.map(|height| height as f32),
  }
}

/// An `@container` rule's query: a comma-separated list of conditions, which holds when one does.
/// <https://drafts.csswg.org/css-conditional-5/#container-rule>
///
/// Approximate: `style()` and `scroll-state()` parse as `<general-enclosed>`, so a condition that
/// uses them holds of no container, where Blink evaluates them against the container.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ContainerQuery(Vec<ContainerCondition>);

impl ContainerQuery {
  pub(crate) fn parse<'i>(
    input: &mut Parser<'i>,
  ) -> Result<Self, ParseError<StyleSheetParseError>> {
    Ok(Self(
      input.parse_comma_separated(ContainerCondition::parse)?,
    ))
  }

  pub(crate) fn conditions(&self) -> &[ContainerCondition] {
    &self.0
  }
}

/// One condition of a container query: the name of the container it asks, if it names one, and a
/// size condition on that container, which a query that names its container may leave out, as
/// Blink's `ConsumeContainerQuery` takes a name alone.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ContainerCondition {
  pub(crate) name: Option<Box<str>>,
  condition: Option<MediaCondition>,
}

impl ContainerCondition {
  fn parse<'i>(input: &mut Parser<'i>) -> Result<Self, ParseError<StyleSheetParseError>> {
    let name = input.try_parse(ContainerName::parse_name).ok();
    let condition = if name.is_some() {
      input.try_parse(MediaCondition::parse).ok()
    } else {
      Some(MediaCondition::parse(input)?)
    };

    Ok(Self { name, condition })
  }

  /// Whether it asks about a size, which only a container whose `container-type` contains the axes
  /// it reads answers, where a name alone picks any container of that name.
  pub(crate) fn queries_size(&self) -> bool {
    self.condition.is_some()
  }

  /// Whether it reads the block axis, which only a `container-type: size` container answers.
  pub(crate) fn reads_block_axis(&self) -> bool {
    self.condition.as_ref().is_some_and(|condition| {
      condition.any_feature(&|feature| {
        matches!(
          feature,
          MediaFeature::Height(..)
            | MediaFeature::BlockSize(..)
            | MediaFeature::AspectRatio(..)
            | MediaFeature::Orientation(..)
        )
      })
    })
  }

  /// Whether a container whose content box is `size` across, an axis it does not contain left
  /// unknown, satisfies it, its relative lengths in the container's `sizing`, as Blink evaluates
  /// them against the container's computed values. A feature other than a size one is unknown to a
  /// container query, which then does not hold, as Blink's `ContainerSelector::SelectsAnyContainer`
  /// selects no container for it.
  pub(crate) fn matches(
    &self,
    size: Size<Option<f32>>,
    viewport: Viewport,
    sizing: &SizingContext,
  ) -> bool {
    self.condition.as_ref().is_none_or(|condition| {
      let unknown = condition.any_feature(&|feature| {
        matches!(
          feature,
          MediaFeature::Resolution(..)
            | MediaFeature::PrefersColorScheme(..)
            | MediaFeature::Preference(..)
            | MediaFeature::Unknown
        )
      });

      !unknown && condition.matches(viewport, size, sizing)
    })
  }
}

impl MediaQueryList {
  pub(crate) fn parse<'i>(
    input: &mut Parser<'i>,
  ) -> Result<Self, ParseError<StyleSheetParseError>> {
    Ok(Self {
      queries: input.parse_comma_separated(|input| {
        // `inline-size` and `block-size` are container features, which make a media query
        // unknown, as Blink's `MediaQueryExp` rejects them.
        let query = input
          .try_parse(MediaQuery::parse)
          .ok()
          .filter(|query| {
            input.is_exhausted()
              && !query.condition.as_ref().is_some_and(|condition| {
                condition.any_feature(&|feature| {
                  matches!(
                    feature,
                    MediaFeature::InlineSize(..)
                      | MediaFeature::BlockSize(..)
                      | MediaFeature::Unknown
                  )
                })
              })
          })
          .unwrap_or_else(MediaQuery::not_all);

        Self::skip_malformed_query(input)?;

        Ok(query)
      })?,
    })
  }

  /// Consumes the rest of a `not all` query, failing on a block or a stray closing delimiter.
  fn skip_malformed_query<'i>(
    input: &mut Parser<'i>,
  ) -> Result<(), ParseError<StyleSheetParseError>> {
    loop {
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
        return Err(CssParseError::unexpected_token());
      }
    }
  }

  /// Whether any query matches the viewport; empty lists always match.
  pub fn matches(&self, viewport: Viewport) -> bool {
    if self.queries.is_empty() {
      return true;
    }

    let sizing = SizingContext::for_media(viewport);

    self
      .queries
      .iter()
      .any(|query| query.matches(viewport, &sizing))
  }
}
