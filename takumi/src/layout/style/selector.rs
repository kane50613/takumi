use cssparser::*;
use precomputed_hash::PrecomputedHash;
use selectors::parser::{
  NonTSPseudoClass, ParseRelative, PseudoElement, SelectorImpl, SelectorList,
  SelectorParseErrorKind,
};
use std::{
  collections::HashMap,
  fmt::{self, Write},
  mem::take,
  ops::Deref,
  rc::Rc,
};
use taffy::Size;

use crate::{
  error::StyleSheetParseError,
  keyframes::parse_keyframe_prelude,
  layout::{
    Viewport,
    style::{
      CalcArena, FromCss, KeyframeRule, KeyframesRule, LengthDefaultsToZero, StyleDeclarationBlock,
    },
  },
  rendering::Sizing,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PropertyRule {
  pub name: String,
  pub syntax: String,
  pub inherits: bool,
  pub initial_value: Option<String>,
  pub media_queries: Vec<MediaQueryList>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum LayerName {
  Named(String),
  Anonymous,
}

type LayerPath = Vec<LayerName>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TakumiIdent(String);

impl Deref for TakumiIdent {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl AsRef<str> for TakumiIdent {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl PartialEq<&str> for TakumiIdent {
  fn eq(&self, other: &&str) -> bool {
    self.0 == *other
  }
}

impl PartialEq<TakumiIdent> for &str {
  fn eq(&self, other: &TakumiIdent) -> bool {
    self == &other.0
  }
}
impl From<&str> for TakumiIdent {
  fn from(s: &str) -> Self {
    Self(s.to_owned())
  }
}

impl ToCss for TakumiIdent {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    serialize_identifier(&self.0, dest)
  }
}

impl PrecomputedHash for TakumiIdent {
  fn precomputed_hash(&self) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for byte in self.0.as_bytes() {
      hash ^= u32::from(byte.to_ascii_lowercase());
      hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
  }
}

#[derive(Debug, Clone)]
pub(crate) struct TakumiSelectorImpl;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UnsupportedPseudoClass {}

impl ToCss for UnsupportedPseudoClass {
  fn to_css<W>(&self, _dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match *self {}
  }
}

impl NonTSPseudoClass for UnsupportedPseudoClass {
  type Impl = TakumiSelectorImpl;
  fn is_active_or_hover(&self) -> bool {
    match *self {}
  }
  fn is_user_action_state(&self) -> bool {
    match *self {}
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UnsupportedPseudoElement {}

impl ToCss for UnsupportedPseudoElement {
  fn to_css<W>(&self, _dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match *self {}
  }
}

impl PseudoElement for UnsupportedPseudoElement {
  type Impl = TakumiSelectorImpl;
}

impl SelectorImpl for TakumiSelectorImpl {
  type ExtraMatchingData<'a> = ();
  type AttrValue = TakumiIdent;
  type Identifier = TakumiIdent;
  type LocalName = TakumiIdent;
  type NamespaceUrl = TakumiIdent;
  type NamespacePrefix = TakumiIdent;
  type BorrowedNamespaceUrl = TakumiIdent;
  type BorrowedLocalName = TakumiIdent;
  type NonTSPseudoClass = UnsupportedPseudoClass;
  type PseudoElement = UnsupportedPseudoElement;
}

struct TakumiSelectorParser;

impl<'i> selectors::Parser<'i> for TakumiSelectorParser {
  type Impl = TakumiSelectorImpl;
  type Error = StyleSheetParseError;

  fn parse_parent_selector(&self) -> bool {
    true
  }

  fn parse_host(&self) -> bool {
    true
  }

  fn parse_non_ts_pseudo_class(
    &self,
    location: SourceLocation,
    name: CowRcStr<'i>,
  ) -> Result<<Self::Impl as SelectorImpl>::NonTSPseudoClass, ParseError<'i, Self::Error>> {
    Err(
      location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(
        name,
      )),
    )
  }

  fn parse_pseudo_element(
    &self,
    location: SourceLocation,
    name: CowRcStr<'i>,
  ) -> Result<<Self::Impl as SelectorImpl>::PseudoElement, ParseError<'i, Self::Error>> {
    Err(
      location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(
        name,
      )),
    )
  }
}

#[derive(Debug, Clone)]
struct ParsedSelectors {
  selectors: SelectorList<TakumiSelectorImpl>,
}

#[derive(Debug, Clone, Default)]
struct StyleSheetFragment {
  rules: Vec<CssRule>,
  keyframes: Vec<KeyframesRule>,
  property_rules: Vec<PropertyRule>,
  declared_layers: Vec<LayerPath>,
}

impl StyleSheetFragment {
  fn extend(&mut self, other: Self) {
    self.rules.extend(other.rules);
    self.keyframes.extend(other.keyframes);
    self.property_rules.extend(other.property_rules);
    self.declared_layers.extend(other.declared_layers);
  }
}

#[derive(Debug)]
enum StyleRuleBodyItem {
  Declarations(Box<StyleDeclarationBlock>),
  Rules(StyleSheetFragment),
}

pub(crate) struct StyleDeclarationParser;

impl<'i> DeclarationParser<'i> for StyleDeclarationParser {
  type Declaration = StyleDeclarationBlock;
  type Error = StyleSheetParseError;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let mut declarations = StyleDeclarationBlock::parse(&name, input).map_err(ParseError::into)?;
    let important = input.try_parse(parse_important).is_ok();
    if important {
      for declaration in &declarations.declarations {
        declarations.importance.insert_declaration(declaration);
      }
    }
    Ok(declarations)
  }
}

impl<'i> QualifiedRuleParser<'i> for StyleDeclarationParser {
  type Prelude = ();
  type QualifiedRule = StyleDeclarationBlock;
  type Error = StyleSheetParseError;
}

impl<'i> AtRuleParser<'i> for StyleDeclarationParser {
  type Prelude = ();
  type AtRule = StyleDeclarationBlock;
  type Error = StyleSheetParseError;
}

impl<'i> RuleBodyItemParser<'i, StyleDeclarationBlock, StyleSheetParseError>
  for StyleDeclarationParser
{
  fn parse_qualified(&self) -> bool {
    false
  }
  fn parse_declarations(&self) -> bool {
    true
  }
}

struct PropertyRuleDeclarationParser;

impl<'i> DeclarationParser<'i> for PropertyRuleDeclarationParser {
  type Declaration = (String, String);
  type Error = StyleSheetParseError;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let start = input.position();
    while input.next_including_whitespace_and_comments().is_ok() {}
    Ok((name.to_string(), input.slice_from(start).trim().to_owned()))
  }
}

impl<'i> QualifiedRuleParser<'i> for PropertyRuleDeclarationParser {
  type Prelude = ();
  type QualifiedRule = (String, String);
  type Error = StyleSheetParseError;
}

impl<'i> AtRuleParser<'i> for PropertyRuleDeclarationParser {
  type Prelude = ();
  type AtRule = (String, String);
  type Error = StyleSheetParseError;
}

impl<'i> RuleBodyItemParser<'i, (String, String), StyleSheetParseError>
  for PropertyRuleDeclarationParser
{
  fn parse_qualified(&self) -> bool {
    false
  }

  fn parse_declarations(&self) -> bool {
    true
  }
}

struct NestedStyleRuleParser<'a> {
  parent_selectors: SelectorList<TakumiSelectorImpl>,
  media_queries: &'a [MediaQueryList],
  layer: Option<LayerPath>,
  lossy: bool,
}

impl<'i> DeclarationParser<'i> for NestedStyleRuleParser<'_> {
  type Declaration = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let mut parser = StyleDeclarationParser;
    parser
      .parse_value(name, input, state)
      .map(Box::new)
      .map(StyleRuleBodyItem::Declarations)
  }
}

impl<'i> QualifiedRuleParser<'i> for NestedStyleRuleParser<'_> {
  type Prelude = SelectorList<TakumiSelectorImpl>;
  type QualifiedRule = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    SelectorList::parse(&TakumiSelectorParser, input, ParseRelative::ForNesting)
  }

  fn parse_block<'t>(
    &mut self,
    nested_selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let selectors = nested_selectors.replace_parent_selector(&self.parent_selectors);
    let fragment = parse_style_rule_block(
      selectors,
      self.media_queries,
      self.layer.as_ref(),
      self.lossy,
      input,
    )?;
    Ok(StyleRuleBodyItem::Rules(fragment))
  }
}

impl<'i> AtRuleParser<'i> for NestedStyleRuleParser<'_> {
  type Prelude = AtRulePrelude;
  type AtRule = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    parse_at_rule_prelude(name, input)
  }

  fn parse_block<'t>(
    &mut self,
    prelude: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
    let fragment = parse_nested_at_rule_block(
      &self.parent_selectors,
      self.media_queries,
      self.layer.as_ref(),
      self.lossy,
      prelude,
      input,
    )?;
    Ok(StyleRuleBodyItem::Rules(fragment))
  }
}

impl<'i> RuleBodyItemParser<'i, StyleRuleBodyItem, StyleSheetParseError>
  for NestedStyleRuleParser<'_>
{
  fn parse_qualified(&self) -> bool {
    true
  }

  fn parse_declarations(&self) -> bool {
    true
  }
}

struct KeyframeDeclarationParser;

impl<'i> DeclarationParser<'i> for KeyframeDeclarationParser {
  type Declaration = StyleDeclarationBlock;
  type Error = StyleSheetParseError;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let declarations = StyleDeclarationBlock::parse(&name, input).map_err(ParseError::into)?;
    // !important flag is ignored in keyframe declartion.
    // Ref: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/At-rules/@keyframes#!important_in_a_keyframe
    input.try_parse(parse_important).ok();
    Ok(declarations)
  }
}

impl<'i> QualifiedRuleParser<'i> for KeyframeDeclarationParser {
  type Prelude = ();
  type QualifiedRule = StyleDeclarationBlock;
  type Error = StyleSheetParseError;
}

impl<'i> AtRuleParser<'i> for KeyframeDeclarationParser {
  type Prelude = ();
  type AtRule = StyleDeclarationBlock;
  type Error = StyleSheetParseError;
}

impl<'i> RuleBodyItemParser<'i, StyleDeclarationBlock, StyleSheetParseError>
  for KeyframeDeclarationParser
{
  fn parse_qualified(&self) -> bool {
    false
  }

  fn parse_declarations(&self) -> bool {
    true
  }
}

struct KeyframeRuleParser;

impl<'i> QualifiedRuleParser<'i> for KeyframeRuleParser {
  type Prelude = Vec<f32>;
  type QualifiedRule = KeyframeRule;
  type Error = StyleSheetParseError;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    parse_keyframe_prelude(input)
  }

  fn parse_block<'t>(
    &mut self,
    offsets: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let mut declaration_parser = KeyframeDeclarationParser;
    let mut declarations = StyleDeclarationBlock::default();
    for result in RuleBodyParser::new(input, &mut declaration_parser) {
      match result {
        Ok(block) => declarations.append(block),
        Err((error, _)) => return Err(error),
      }
    }

    Ok(KeyframeRule {
      offsets,
      declarations,
    })
  }
}

impl<'i> AtRuleParser<'i> for KeyframeRuleParser {
  type Prelude = ();
  type AtRule = KeyframeRule;
  type Error = StyleSheetParseError;
}

struct RuleParser {
  current_layer: Option<LayerPath>,
  lossy: bool,
}

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaOrientation {
  Portrait,
  Landscape,
}

#[derive(Debug, Clone, PartialEq)]
enum MediaFeature {
  Width(MediaFeatureComparison, LengthDefaultsToZero),
  Height(MediaFeatureComparison, LengthDefaultsToZero),
  Orientation(MediaOrientation),
}

#[derive(Debug, Clone, PartialEq)]
struct MediaQuery {
  media_type: MediaType,
  features: Vec<MediaFeature>,
  negated: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct MediaQueryList {
  queries: Vec<MediaQuery>,
}

impl MediaFeature {
  fn matches(&self, viewport: Viewport, sizing: &Sizing) -> bool {
    match self {
      Self::Width(comparison, value) => viewport.width.is_some_and(|width| {
        compare_media_feature(*comparison, width as f32, value.to_px(sizing, width as f32))
      }),
      Self::Height(comparison, value) => viewport.height.is_some_and(|height| {
        compare_media_feature(
          *comparison,
          height as f32,
          value.to_px(sizing, height as f32),
        )
      }),
      Self::Orientation(MediaOrientation::Portrait) => viewport
        .width
        .zip(viewport.height)
        .is_some_and(|(width, height)| height >= width),
      Self::Orientation(MediaOrientation::Landscape) => viewport
        .width
        .zip(viewport.height)
        .is_some_and(|(width, height)| width > height),
    }
  }
}

impl MediaQuery {
  fn matches(&self, viewport: Viewport, sizing: &Sizing) -> bool {
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
  pub(crate) fn matches(&self, viewport: Viewport) -> bool {
    if self.queries.is_empty() {
      return true;
    }

    let sizing = Sizing {
      viewport,
      container_size: Size::NONE,
      font_size: viewport.font_size,
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
      media_type = parse_media_type(input.expect_ident_cloned()?);
      has_explicit_media_type = true;
    } else if keyword.eq_ignore_ascii_case("only") {
      media_type = parse_media_type(input.expect_ident_cloned()?);
      has_explicit_media_type = true;
    } else {
      media_type = parse_media_type(keyword);
      has_explicit_media_type = true;
    }
  }

  if input
    .try_parse(|input| parse_media_feature_block(input, &mut features))
    .is_ok()
  {
    while input
      .try_parse(|input| input.expect_ident_matching("and"))
      .is_ok()
    {
      parse_media_feature_block(input, &mut features)?;
    }
  } else if has_explicit_media_type {
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
  input.expect_colon()?;

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

  let length = LengthDefaultsToZero::from_css(input).map_err(ParseError::into)?;

  if feature_name.eq_ignore_ascii_case("width")
    || feature_name.eq_ignore_ascii_case("min-width")
    || feature_name.eq_ignore_ascii_case("max-width")
  {
    Ok(MediaFeature::Width(comparison, length))
  } else if feature_name.eq_ignore_ascii_case("height")
    || feature_name.eq_ignore_ascii_case("min-height")
    || feature_name.eq_ignore_ascii_case("max-height")
  {
    Ok(MediaFeature::Height(comparison, length))
  } else {
    Err(input.new_custom_error(StyleSheetParseError::unsupported_media_feature()))
  }
}

#[derive(Debug, Clone)]
enum AtRulePrelude {
  Keyframes(String),
  Layer(Vec<LayerPath>),
  Media(MediaQueryList),
  Property(String),
  Supports(bool),
}

fn parse_fragment_with_mode<'i, 't>(
  input: &mut Parser<'i, 't>,
  parser: &mut RuleParser,
) -> Result<StyleSheetFragment, ParseError<'i, StyleSheetParseError>> {
  let mut fragment = StyleSheetFragment::default();
  let lossy = parser.lossy;
  for nested in StyleSheetParser::new(input, parser) {
    match nested {
      Ok(nested) => fragment.extend(nested),
      Err((error, _)) => {
        if lossy {
          continue;
        }
        return Err(error);
      }
    }
  }

  Ok(fragment)
}

#[derive(Debug, Clone)]
pub(crate) struct CssRule {
  pub(crate) selectors: SelectorList<TakumiSelectorImpl>,
  pub(crate) normal_declarations: StyleDeclarationBlock,
  pub(crate) important_declarations: StyleDeclarationBlock,
  pub(crate) media_queries: Vec<MediaQueryList>,
  pub(crate) layer: Option<LayerPath>,
  pub(crate) layer_order: Option<usize>,
}

fn parse_property_rule<'i, 't>(
  property_name: String,
  input: &mut Parser<'i, 't>,
) -> Result<PropertyRule, ParseError<'i, StyleSheetParseError>> {
  let mut parser = PropertyRuleDeclarationParser;
  let mut syntax = None;
  let mut inherits = None;
  let mut initial_value = None;
  let mut invalid_inherits = false;

  for result in RuleBodyParser::new(input, &mut parser) {
    let (name, value) = match result {
      Ok(value) => value,
      Err((error, _)) => return Err(error),
    };

    if name.eq_ignore_ascii_case("syntax") {
      syntax = Some(value);
      continue;
    }

    if name.eq_ignore_ascii_case("inherits") {
      if value.eq_ignore_ascii_case("true") {
        inherits = Some(true);
        continue;
      }

      if value.eq_ignore_ascii_case("false") {
        inherits = Some(false);
        continue;
      }

      invalid_inherits = true;
      continue;
    }

    if name.eq_ignore_ascii_case("initial-value") {
      initial_value = Some(value);
    }
  }

  if invalid_inherits {
    return Err(input.new_custom_error(StyleSheetParseError::property_inherits_must_be_boolean()));
  }

  let Some(syntax) = syntax else {
    return Err(input.new_custom_error(StyleSheetParseError::missing_property_syntax()));
  };
  let Some(inherits) = inherits else {
    return Err(input.new_custom_error(StyleSheetParseError::missing_property_inherits()));
  };

  Ok(PropertyRule {
    name: property_name,
    syntax,
    inherits,
    initial_value,
    media_queries: Vec::new(),
  })
}

fn supports_declaration<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<bool, ParseError<'i, StyleSheetParseError>> {
  let name = input.expect_ident_cloned()?;
  input.expect_colon()?;
  let declaration = StyleDeclarationBlock::parse(&name, input).map_err(ParseError::into)?;
  Ok(!declaration.declarations.is_empty() && input.is_exhausted())
}

fn parse_supports_in_parens<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<bool, ParseError<'i, StyleSheetParseError>> {
  let location = input.current_source_location();
  match input.next()? {
    Token::ParenthesisBlock => input.parse_nested_block(|input| {
      let state = input.state();
      if let Ok(result) = parse_supports_condition(input)
        && input.is_exhausted()
      {
        return Ok(result);
      }

      input.reset(&state);
      supports_declaration(input)
    }),
    token => Err(location.new_unexpected_token_error(token.clone())),
  }
}

fn parse_supports_not<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<bool, ParseError<'i, StyleSheetParseError>> {
  if input
    .try_parse(|input| input.expect_ident_matching("not"))
    .is_ok()
  {
    return Ok(!parse_supports_not(input)?);
  }

  parse_supports_in_parens(input)
}

fn parse_supports_condition<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<bool, ParseError<'i, StyleSheetParseError>> {
  let mut result = parse_supports_not(input)?;
  let mut operator = None;

  loop {
    if input
      .try_parse(|input| input.expect_ident_matching("and"))
      .is_ok()
    {
      if matches!(operator, Some(false)) {
        return Err(
          input.new_custom_error(StyleSheetParseError::supports_mixed_and_or_without_parentheses()),
        );
      }
      operator = Some(true);
      result &= parse_supports_not(input)?;
      continue;
    }

    if input
      .try_parse(|input| input.expect_ident_matching("or"))
      .is_ok()
    {
      if matches!(operator, Some(true)) {
        return Err(
          input.new_custom_error(StyleSheetParseError::supports_mixed_and_or_without_parentheses()),
        );
      }
      operator = Some(false);
      result |= parse_supports_not(input)?;
      continue;
    }

    break;
  }

  Ok(result)
}

fn parse_at_rule_prelude<'i, 't>(
  name: CowRcStr<'i>,
  input: &mut Parser<'i, 't>,
) -> Result<AtRulePrelude, ParseError<'i, StyleSheetParseError>> {
  if name.eq_ignore_ascii_case("layer") {
    let mut layer_names = input
      .try_parse(|input| input.parse_comma_separated(parse_layer_name))
      .unwrap_or_default();
    if layer_names.is_empty() {
      layer_names.push(vec![LayerName::Anonymous]);
    }
    return Ok(AtRulePrelude::Layer(layer_names));
  }

  if name.eq_ignore_ascii_case("keyframes") {
    return Ok(AtRulePrelude::Keyframes(
      input.expect_ident_or_string()?.to_string(),
    ));
  }

  if name.eq_ignore_ascii_case("media") {
    return Ok(AtRulePrelude::Media(MediaQueryList {
      queries: input.parse_comma_separated(parse_media_query)?,
    }));
  }

  if name.eq_ignore_ascii_case("supports") {
    return parse_supports_condition(input).map(AtRulePrelude::Supports);
  }

  if name.eq_ignore_ascii_case("property") {
    let property_name = input.expect_ident_or_string()?.to_string();
    if !property_name.starts_with("--") {
      return Err(
        input.new_custom_error(StyleSheetParseError::property_name_must_be_custom_property()),
      );
    }
    return Ok(AtRulePrelude::Property(property_name));
  }

  Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)))
}

fn parse_layer_name<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<LayerPath, ParseError<'i, StyleSheetParseError>> {
  let mut segments = Vec::new();

  loop {
    let location = input.current_source_location();
    let segment = match input.next()? {
      Token::Ident(value) | Token::QuotedString(value) => value.to_string(),
      token => return Err(location.new_unexpected_token_error(token.clone())),
    };
    segments.push(LayerName::Named(segment));

    if input.try_parse(|input| input.expect_delim('.')).is_err() {
      break;
    }
  }

  Ok(segments)
}

fn extend_layer_name(
  current_layer: Option<&LayerPath>,
  layer_name: &[LayerName],
) -> Option<LayerPath> {
  if layer_name == [LayerName::Anonymous] {
    let mut nested_layer = current_layer.cloned().unwrap_or_default();
    nested_layer.push(LayerName::Anonymous);
    return Some(nested_layer);
  }

  let mut combined = current_layer.cloned().unwrap_or_default();
  combined.extend(layer_name.iter().cloned());
  Some(combined)
}

fn ensure_single_layer_name<'i>(
  layer_names: &[LayerPath],
  input: &Parser<'i, '_>,
) -> Result<(), ParseError<'i, StyleSheetParseError>> {
  if layer_names.len() <= 1 {
    return Ok(());
  }

  Err(input.new_custom_error(StyleSheetParseError::layer_block_multiple_names()))
}

fn parse_style_rule_block<'i, 't>(
  selectors: SelectorList<TakumiSelectorImpl>,
  media_queries: &[MediaQueryList],
  layer: Option<&LayerPath>,
  lossy: bool,
  input: &mut Parser<'i, 't>,
) -> Result<StyleSheetFragment, ParseError<'i, StyleSheetParseError>> {
  let mut normal_declarations = StyleDeclarationBlock::default();
  let mut important_declarations = StyleDeclarationBlock::default();
  let layer = layer.cloned();
  let mut fragment = StyleSheetFragment::default();
  let mut parser = NestedStyleRuleParser {
    parent_selectors: selectors.clone(),
    media_queries,
    layer: layer.clone(),
    lossy,
  };

  for result in RuleBodyParser::new(input, &mut parser) {
    match result {
      Err((error, _)) => {
        if lossy {
          continue;
        }
        return Err(error);
      }
      Ok(StyleRuleBodyItem::Declarations(declarations)) => {
        let declarations = *declarations;
        if declarations.importance.is_empty() {
          normal_declarations.append(declarations);
        } else {
          important_declarations.append(declarations);
        }
      }
      Ok(StyleRuleBodyItem::Rules(nested_rules)) => {
        if !normal_declarations.declarations.is_empty()
          || !important_declarations.declarations.is_empty()
        {
          fragment.rules.push(CssRule {
            selectors: selectors.clone(),
            normal_declarations: take(&mut normal_declarations),
            important_declarations: take(&mut important_declarations),
            media_queries: media_queries.to_vec(),
            layer: layer.clone(),
            layer_order: None,
          });
        }
        fragment.extend(nested_rules);
      }
    }
  }

  if normal_declarations.declarations.is_empty() && important_declarations.declarations.is_empty() {
    return Ok(fragment);
  }

  fragment.rules.push(CssRule {
    selectors,
    normal_declarations,
    important_declarations,
    media_queries: media_queries.to_vec(),
    layer,
    layer_order: None,
  });
  Ok(fragment)
}

fn parse_nested_at_rule_block<'i, 't>(
  parent_selectors: &SelectorList<TakumiSelectorImpl>,
  media_queries: &[MediaQueryList],
  current_layer: Option<&LayerPath>,
  lossy: bool,
  prelude: AtRulePrelude,
  input: &mut Parser<'i, 't>,
) -> Result<StyleSheetFragment, ParseError<'i, StyleSheetParseError>> {
  match prelude {
    AtRulePrelude::Layer(layer_names) => {
      ensure_single_layer_name(&layer_names, input)?;
      let Some(layer_name) = layer_names.into_iter().next() else {
        return Ok(StyleSheetFragment::default());
      };
      let nested_layer = extend_layer_name(current_layer, &layer_name);
      parse_style_rule_block(
        parent_selectors.clone(),
        media_queries,
        nested_layer.as_ref(),
        lossy,
        input,
      )
    }
    AtRulePrelude::Media(media_query) => {
      let mut merged_media_queries = media_queries.to_vec();
      merged_media_queries.push(media_query);
      parse_style_rule_block(
        parent_selectors.clone(),
        &merged_media_queries,
        current_layer,
        lossy,
        input,
      )
    }
    AtRulePrelude::Supports(true) => parse_style_rule_block(
      parent_selectors.clone(),
      media_queries,
      current_layer,
      lossy,
      input,
    ),
    AtRulePrelude::Supports(false) => {
      let mut parser = NestedStyleRuleParser {
        parent_selectors: parent_selectors.clone(),
        media_queries,
        layer: current_layer.cloned(),
        lossy,
      };
      for _ in RuleBodyParser::new(input, &mut parser).flatten() {}
      Ok(StyleSheetFragment::default())
    }
    AtRulePrelude::Keyframes(_) | AtRulePrelude::Property(_) => {
      Err(input.new_custom_error(StyleSheetParseError::unsupported_nested_at_rule()))
    }
  }
}

impl<'i> QualifiedRuleParser<'i> for RuleParser {
  type Prelude = ParsedSelectors;
  type QualifiedRule = StyleSheetFragment;
  type Error = StyleSheetParseError;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    Ok(ParsedSelectors {
      selectors: SelectorList::parse(&TakumiSelectorParser, input, ParseRelative::No)?,
    })
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    parse_style_rule_block(
      selectors.selectors,
      &[],
      self.current_layer.as_ref(),
      self.lossy,
      input,
    )
  }
}

impl<'i> AtRuleParser<'i> for RuleParser {
  type Prelude = AtRulePrelude;
  type AtRule = StyleSheetFragment;
  type Error = StyleSheetParseError;

  fn parse_prelude<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    parse_at_rule_prelude(name, input)
  }

  fn parse_block<'t>(
    &mut self,
    prelude: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
    match prelude {
      AtRulePrelude::Layer(layer_names) => {
        ensure_single_layer_name(&layer_names, input)?;
        let declared_layers = layer_names
          .iter()
          .filter_map(|layer_name| extend_layer_name(self.current_layer.as_ref(), layer_name))
          .collect::<Vec<_>>();
        let Some(layer_name) = layer_names.into_iter().next() else {
          return Ok(StyleSheetFragment {
            declared_layers,
            ..StyleSheetFragment::default()
          });
        };
        let nested_layer = extend_layer_name(self.current_layer.as_ref(), &layer_name);
        let mut fragment = parse_fragment_with_mode(
          input,
          &mut RuleParser {
            current_layer: nested_layer.clone(),
            lossy: self.lossy,
          },
        )?;
        fragment.declared_layers.splice(0..0, declared_layers);
        Ok(fragment)
      }
      AtRulePrelude::Keyframes(name) => {
        let mut parser = KeyframeRuleParser;
        let mut keyframes = Vec::new();
        for keyframe in StyleSheetParser::new(input, &mut parser) {
          match keyframe {
            Ok(keyframe) => keyframes.push(keyframe),
            Err((error, _)) => {
              if self.lossy {
                continue;
              }
              return Err(error);
            }
          }
        }

        Ok(StyleSheetFragment {
          keyframes: vec![
            KeyframesRule::builder()
              .name(name)
              .keyframes(keyframes)
              .build(),
          ],
          ..StyleSheetFragment::default()
        })
      }
      AtRulePrelude::Media(media_query) => {
        let mut fragment = parse_fragment_with_mode(
          input,
          &mut RuleParser {
            current_layer: self.current_layer.clone(),
            lossy: self.lossy,
          },
        )?;

        for rule in &mut fragment.rules {
          rule.media_queries.push(media_query.clone());
        }
        for keyframes in &mut fragment.keyframes {
          keyframes.media_queries.push(media_query.clone());
        }
        for property_rule in &mut fragment.property_rules {
          property_rule.media_queries.push(media_query.clone());
        }

        Ok(fragment)
      }
      AtRulePrelude::Supports(is_supported) => {
        if !is_supported {
          let mut parser = RuleParser {
            current_layer: self.current_layer.clone(),
            lossy: self.lossy,
          };
          for _ in StyleSheetParser::new(input, &mut parser) {}
          return Ok(StyleSheetFragment::default());
        }

        parse_fragment_with_mode(
          input,
          &mut RuleParser {
            current_layer: self.current_layer.clone(),
            lossy: self.lossy,
          },
        )
      }
      AtRulePrelude::Property(name) => Ok(StyleSheetFragment {
        property_rules: vec![parse_property_rule(name, input)?],
        ..StyleSheetFragment::default()
      }),
    }
  }

  fn rule_without_block(
    &mut self,
    prelude: Self::Prelude,
    _start: &ParserState,
  ) -> Result<Self::AtRule, ()> {
    match prelude {
      AtRulePrelude::Layer(layer_names) => Ok(StyleSheetFragment {
        declared_layers: layer_names
          .into_iter()
          .filter_map(|layer_name| extend_layer_name(self.current_layer.as_ref(), &layer_name))
          .collect(),
        ..StyleSheetFragment::default()
      }),
      _ => Err(()),
    }
  }
}

/// Defines a stylesheet with rules, keyframes, and property rules.
#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
  pub(crate) rules: Vec<CssRule>,
  pub(crate) keyframes: Vec<KeyframesRule>,
  pub(crate) property_rules: Vec<PropertyRule>,
}

impl From<Vec<KeyframesRule>> for StyleSheet {
  fn from(keyframes: Vec<KeyframesRule>) -> Self {
    Self {
      keyframes,
      ..Default::default()
    }
  }
}

impl StyleSheet {
  /// Extends the stylesheet with keyframes.
  pub fn extend_keyframes(&mut self, keyframes: Vec<KeyframesRule>) {
    self.keyframes.extend(keyframes);
  }

  /// Parses a list of stylesheets.
  pub fn parse_list<I, S>(stylesheets: I) -> Result<Self, StyleSheetParseError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let mut combined_css = String::new();

    for css in stylesheets {
      combined_css.push_str(css.as_ref());
    }

    Self::parse(&combined_css)
  }

  /// Parses a list of stylesheets while discarding invalid rules.
  pub fn parse_list_loosy<I, S>(stylesheets: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let mut combined_css = String::new();

    for css in stylesheets {
      combined_css.push_str(css.as_ref());
    }

    Self::parse_loosy(&combined_css)
  }

  /// Parses a list of owned stylesheets while discarding invalid rules.
  pub fn parse_owned_list_loosy(stylesheets: Vec<String>) -> Self {
    if stylesheets.is_empty() {
      return Self::default();
    }

    if stylesheets.len() == 1 {
      return Self::parse_loosy(&stylesheets[0]);
    }

    let mut combined_css = String::with_capacity(stylesheets.iter().map(String::len).sum());

    for css in stylesheets {
      combined_css.push_str(&css);
    }

    Self::parse_loosy(&combined_css)
  }

  /// Parses a stylesheet.
  pub fn parse(css: &str) -> Result<Self, StyleSheetParseError> {
    Self::parse_with_mode(css, false)
  }

  /// Parses a stylesheet while discarding invalid rules.
  pub fn parse_loosy(css: &str) -> Self {
    let Ok(stylesheet) = Self::parse_with_mode(css, true) else {
      unreachable!();
    };
    stylesheet
  }

  fn parse_with_mode(css: &str, lossy: bool) -> Result<Self, StyleSheetParseError> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rule_parser = RuleParser {
      current_layer: None,
      lossy,
    };

    let mut rules = Vec::new();
    let mut keyframes = Vec::new();
    let mut property_rules = Vec::new();
    let mut declared_layers = Vec::new();

    for fragment in StyleSheetParser::new(&mut parser, &mut rule_parser) {
      match fragment {
        Ok(fragment) => {
          rules.extend(fragment.rules);
          keyframes.extend(fragment.keyframes);
          property_rules.extend(fragment.property_rules);
          declared_layers.extend(fragment.declared_layers);
        }
        Err((error, context)) => {
          if lossy {
            continue;
          }
          return Err(StyleSheetParseError::from_parse_error(css, context, error));
        }
      }
    }

    let mut layer_order = HashMap::<LayerPath, usize>::new();

    for layer_name in declared_layers {
      let next_order = layer_order.len();
      layer_order.entry(layer_name).or_insert(next_order);
    }

    for rule in &rules {
      if let Some(layer_name) = &rule.layer {
        let next_order = layer_order.len();
        layer_order.entry(layer_name.clone()).or_insert(next_order);
      }
    }

    for rule in &mut rules {
      rule.layer_order = rule
        .layer
        .as_ref()
        .and_then(|layer_name| layer_order.get(layer_name).copied());
    }

    rules.retain(|rule| {
      !rule.normal_declarations.declarations.is_empty()
        || !rule.important_declarations.declarations.is_empty()
    });

    Ok(Self {
      rules,
      keyframes,
      property_rules,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use cssparser::ToCss;

  use crate::layout::style::{Color, ColorInput, ComputedStyle, Length, Style, StyleDeclaration};

  fn computed_style_from_declarations(declarations: &StyleDeclarationBlock) -> ComputedStyle {
    let mut style = Style::default();
    for declaration in &declarations.declarations {
      declaration.merge_into_ref(&mut style);
    }
    style.inherit(&ComputedStyle::default())
  }

  fn selector_text(rule: &CssRule) -> String {
    rule.selectors.to_css_string()
  }

  fn parse_stylesheet(css: &str) -> StyleSheet {
    let result = StyleSheet::parse(css);
    assert!(result.is_ok(), "expected stylesheet to parse: {result:?}");
    let Ok(stylesheet) = result else {
      unreachable!();
    };
    stylesheet
  }

  fn parse_stylesheet_loosy(css: &str) -> StyleSheet {
    StyleSheet::parse_loosy(css)
  }

  fn assert_lossy_parse_keeps_single_valid_rule(css: &str) {
    let sheet = parse_stylesheet_loosy(css);
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_parse_stylesheet() {
    let css = r#"
            .box {
                width: 100px;
                color: red;
            }
        "#;
    let sheet = parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    let rule = &sheet.rules[0];

    assert_eq!(rule.selectors.slice().len(), 1);
    assert_eq!(
      computed_style_from_declarations(&rule.normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_compound_selectors_specificity() {
    let sheet = parse_stylesheet(
      r#"
        div.box { width: 10px; }
        #hero .label { height: 20px; }
      "#,
    );
    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
    assert_eq!(sheet.rules[1].selectors.slice().len(), 1);
    assert!(sheet.rules[0].selectors.slice()[0].specificity() > 0);
    assert!(
      sheet.rules[1].selectors.slice()[0].specificity()
        > sheet.rules[0].selectors.slice()[0].specificity()
    );
  }

  #[test]
  fn test_parse_stylesheet_multiple_rules() {
    let sheet = parse_stylesheet(
      r#"
        .a { width: 10px; }
        .b { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(10.0)
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[1].normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_multiple_selectors_in_rule() {
    let sheet = parse_stylesheet(
      r#"
        .a, .b { width: 12px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].selectors.slice().len(), 2);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(12.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_universal_selector() {
    let sheet = parse_stylesheet(
      r#"
        * { width: 100px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), "*");
    assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_important_declaration() {
    let sheet = parse_stylesheet(
      r#"
        .a { width: 10px !important; height: 20px; }
      "#,
    );

    let rule = &sheet.rules[0];
    assert_eq!(
      computed_style_from_declarations(&rule.important_declarations).width,
      Length::Px(10.0)
    );
    assert_eq!(
      computed_style_from_declarations(&rule.normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_shorthand_clears_prior_longhand() {
    let sheet = parse_stylesheet(
      r#"
        .a { padding-left: 4px; padding: 10px; }
      "#,
    );

    let declarations = &sheet.rules[0].normal_declarations;
    assert_eq!(declarations.declarations.len(), 5);
    assert_eq!(
      declarations.declarations[0],
      StyleDeclaration::padding_left(Length::Px(4.0))
    );
    assert_eq!(
      declarations.declarations[1],
      StyleDeclaration::padding_top(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[2],
      StyleDeclaration::padding_right(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[3],
      StyleDeclaration::padding_bottom(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[4],
      StyleDeclaration::padding_left(Length::Px(10.0))
    );
  }

  #[test]
  fn test_parse_stylesheet_webkit_alias_property() {
    let sheet = parse_stylesheet(
      r#"
        .a { -webkit-text-fill-color: rgb(255, 0, 0); }
      "#,
    );

    let style = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(
      style.webkit_text_fill_color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_stylesheet_unknown_property_does_not_drop_supported_declarations() {
    let sheet = parse_stylesheet(
      r#"
        .a { --local-token: 1; width: 14px; unsupported-prop: 2; height: 6px; }
      "#,
    );

    let style = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(style.width, Length::Px(14.0));
    assert_eq!(style.height, Length::Px(6.0));
  }

  #[test]
  fn test_attribute_selector_rule_is_preserved() {
    let sheet = parse_stylesheet_loosy(
      r#"
        [data-kind="hero"] { width: 10px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
  }

  #[test]
  fn test_parse_stylesheet_accepts_attribute_selector() {
    let result = StyleSheet::parse(
      r#"
        [data-kind="hero"] { width: 10px; }
      "#,
    );

    assert!(result.is_ok());
  }

  #[test]
  fn test_unsupported_pseudo_selector_rule_is_rejected() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .a:hover { width: 10px; }
      "#,
    );

    assert!(sheet.rules.is_empty());
  }

  #[test]
  fn test_parse_keyframes_rule() {
    let sheet = parse_stylesheet(
      r#"
        @keyframes fade {
          from { opacity: 0; }
          50% { opacity: 0.5; }
          to { opacity: 1; }
        }
      "#,
    );

    assert!(sheet.rules.is_empty());
    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].name, "fade");
    assert_eq!(sheet.keyframes[0].keyframes.len(), 3);
    assert_eq!(sheet.keyframes[0].keyframes[0].offsets, vec![0.0]);
    assert_eq!(sheet.keyframes[0].keyframes[1].offsets, vec![0.5]);
    assert_eq!(sheet.keyframes[0].keyframes[2].offsets, vec![1.0]);
  }

  #[test]
  fn test_parse_media_rule_with_viewport_features() {
    let sheet = parse_stylesheet(
      r#"
        @media screen and (min-width: 600px) and (orientation: landscape) {
          .card { width: 100px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert!(sheet.keyframes.is_empty());
    assert!(
      sheet.rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new(Some(800), Some(600))))
    );
    assert!(
      !sheet.rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new(Some(500), Some(800))))
    );
  }

  #[test]
  fn test_parse_media_rule_with_comma_list() {
    let sheet = parse_stylesheet(
      r#"
        @media (max-width: 480px), (min-width: 1024px) {
          .card { width: 100px; }
        }
      "#,
    );

    let Some(media) = sheet.rules[0].media_queries.first() else {
      unreachable!("expected media queries on parsed rule");
    };
    assert!(media.matches(Viewport::new(Some(400), Some(800))));
    assert!(media.matches(Viewport::new(Some(1280), Some(800))));
    assert!(!media.matches(Viewport::new(Some(800), Some(800))));
  }

  #[test]
  fn test_parse_media_rule_applies_to_keyframes_and_property_rules() {
    let sheet = parse_stylesheet(
      r#"
        @media (min-width: 600px) {
          @keyframes fade {
            from { opacity: 0; }
            to { opacity: 1; }
          }

          @property --box-size {
            syntax: "<length>";
            inherits: false;
            initial-value: 10px;
          }
        }
      "#,
    );

    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.property_rules.len(), 1);
    assert!(
      sheet.keyframes[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new(Some(800), Some(600))))
    );
    assert!(
      sheet.property_rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new(Some(800), Some(600))))
    );
  }

  #[test]
  fn test_parse_nested_rule_is_flattened() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          width: 100px;
          .title { height: 20px; }
          & > .icon { width: 12px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 3);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(selector_text(&sheet.rules[1]), ":is(.card) .title");
    assert_eq!(selector_text(&sheet.rules[2]), ":is(.card) > .icon");
  }

  #[test]
  fn test_parse_nested_rule_cross_product_for_selector_lists() {
    let sheet = parse_stylesheet(
      r#"
        .card, .panel {
          & .title, & .subtitle { width: 12px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(
      selector_text(&sheet.rules[0]),
      ":is(.card, .panel) .title, :is(.card, .panel) .subtitle"
    );
  }

  #[test]
  fn test_parse_nested_rule_uses_is_wrapper_for_multi_parent_lists() {
    let sheet = parse_stylesheet(
      r#"
        .card, .panel {
          & + .item { width: 12px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ":is(.card, .panel) + .item");
  }

  #[test]
  fn test_parse_nested_media_and_supports_rules() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          @media (min-width: 600px) {
            @supports (display: grid) {
              width: 100px;
            }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(sheet.rules[0].media_queries.len(), 1);
    assert!(
      sheet.rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new(Some(800), Some(600))))
    );
  }

  #[test]
  fn test_parse_multiple_nested_media_queries_accumulate() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          @media (min-width: 600px) {
            @media (orientation: landscape) {
              width: 100px;
            }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].media_queries.len(), 2);
    assert!(sheet.rules[0].media_queries[0].matches(Viewport::new(Some(800), Some(600))));
    assert!(sheet.rules[0].media_queries[1].matches(Viewport::new(Some(800), Some(600))));
    assert!(!sheet.rules[0].media_queries[1].matches(Viewport::new(Some(500), Some(800))));
  }

  #[test]
  fn test_parse_supports_rule_filters_unsupported_declarations() {
    let sheet = parse_stylesheet(
      r#"
        @supports (display: grid) {
          .card { width: 100px; }
        }

        @supports (unknown-prop: nope) {
          .card { height: 20px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_parse_supports_not_and_or_conditions() {
    let sheet = parse_stylesheet(
      r#"
        @supports (display: grid) and (not (unknown-prop: nope)) {
          .grid { width: 10px; }
        }

        @supports (unknown-prop: nope) or (display: flex) {
          .flex { height: 20px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(selector_text(&sheet.rules[0]), ".grid");
    assert_eq!(selector_text(&sheet.rules[1]), ".flex");
  }

  #[test]
  fn test_parse_supports_mixed_and_or_requires_parentheses() {
    let sheet = parse_stylesheet_loosy(
      r#"
        @supports (display: grid) and (color: red) or (display: flex) {
          .invalid { width: 10px; }
        }

        .valid { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".valid");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_property_rule() {
    let sheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 1);
    assert_eq!(sheet.property_rules[0].name, "--box-size");
    assert_eq!(sheet.property_rules[0].syntax, "\"<length>\"");
    assert!(!sheet.property_rules[0].inherits);
    assert_eq!(
      sheet.property_rules[0].initial_value,
      Some("10px".to_owned())
    );
  }

  #[test]
  fn test_parse_property_rule_descriptors_case_insensitively() {
    let sheet = parse_stylesheet(
      r#"
        @property --box-size {
          SYNTAX: "<length>";
          InHeRiTs: false;
          INITIAL-VALUE: 10px;
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 1);
    assert_eq!(sheet.property_rules[0].name, "--box-size");
    assert_eq!(sheet.property_rules[0].syntax, "\"<length>\"");
    assert!(!sheet.property_rules[0].inherits);
    assert_eq!(
      sheet.property_rules[0].initial_value,
      Some("10px".to_owned())
    );
  }

  #[test]
  fn test_parse_property_rule_requires_initial_value_for_typed_syntax() {
    let sheet = parse_stylesheet(
      r#"
        @property --tw-rotate-x {
          syntax: "*";
          inherits: false;
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 1);
    assert_eq!(sheet.property_rules[0].name, "--tw-rotate-x");
    assert_eq!(sheet.property_rules[0].syntax, "\"*\"");
    assert!(!sheet.property_rules[0].inherits);
    assert_eq!(sheet.property_rules[0].initial_value, None);

    let sheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 1);
    assert_eq!(sheet.property_rules[0].initial_value, None);
  }

  #[test]
  fn test_parse_property_rule_supports_extended_syntaxes() {
    let sheet = parse_stylesheet(
      r#"
        @property --accent {
          syntax: "<length> | <color>";
          inherits: false;
          initial-value: red;
        }
        @property --display-state {
          syntax: "none | auto";
          inherits: false;
          initial-value: none;
        }
        @property --fade-duration {
          syntax: "<time>";
          inherits: false;
          initial-value: 150ms;
        }
        @property --move {
          syntax: "<transform-function>";
          inherits: false;
          initial-value: translate(10px, 20px);
        }
        @property --curve {
          syntax: "<easing-function>";
          inherits: false;
          initial-value: ease-in-out;
        }
        @property --fx {
          syntax: "<filter-function>";
          inherits: false;
          initial-value: blur(4px);
        }
        @property --bg {
          syntax: "<image>";
          inherits: false;
          initial-value: linear-gradient(red, blue);
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 7);
    assert_eq!(sheet.property_rules[0].syntax, "\"<length> | <color>\"");
    assert_eq!(sheet.property_rules[1].syntax, "\"none | auto\"");
    assert_eq!(sheet.property_rules[2].syntax, "\"<time>\"");
    assert_eq!(sheet.property_rules[3].syntax, "\"<transform-function>\"");
    assert_eq!(sheet.property_rules[4].syntax, "\"<easing-function>\"");
    assert_eq!(sheet.property_rules[5].syntax, "\"<filter-function>\"");
    assert_eq!(sheet.property_rules[6].syntax, "\"<image>\"");
  }

  #[test]
  fn test_lossy_parse_rejects_invalid_property_rules() {
    for css in [
      r#"
        @property color {
          syntax: "<color>";
          inherits: false;
          initial-value: red;
        }

        .card { width: 100px; }
      "#,
      r#"
        @property --box-size {
          inherits: false;
          initial-value: 10px;
        }

        .card { width: 100px; }
      "#,
      r#"
        @property --accent {
          syntax: "<color>";
          initial-value: red;
        }

        .card { width: 100px; }
      "#,
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: maybe;
          initial-value: 10px;
        }

        .card { width: 100px; }
      "#,
    ] {
      let sheet = parse_stylesheet_loosy(css);

      assert!(sheet.property_rules.is_empty());
      assert_eq!(sheet.rules.len(), 1);
      assert_eq!(selector_text(&sheet.rules[0]), ".card");
      assert_eq!(
        computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
        Length::Px(100.0)
      );
    }
  }

  #[test]
  fn test_parse_stylesheet_returns_error_for_invalid_property_rule() {
    let result = StyleSheet::parse(
      r#"
        @property --box-size {
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );

    assert!(result.is_err());
  }

  #[test]
  fn test_property_rule_computationally_dependent_initial_value_is_preserved() {
    let sheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: var(--fallback);
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 1);
    assert_eq!(
      sheet.property_rules[0].initial_value,
      Some("var(--fallback)".to_owned())
    );
  }

  #[test]
  fn test_parse_layer_rule_without_block() {
    let sheet = parse_stylesheet(
      r#"
        @layer theme, base, components, utilities;
        @layer utilities {
          .card { width: 100px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      sheet.rules[0].layer.as_ref(),
      Some(&vec![LayerName::Named("utilities".to_owned())])
    );
    assert_eq!(sheet.rules[0].layer_order, Some(3));
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_parse_nested_layers_are_transparent() {
    let sheet = parse_stylesheet(
      r#"
        @layer theme {
          @layer components {
            .card { width: 100px; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      sheet.rules[0].layer.as_ref(),
      Some(&vec![
        LayerName::Named("theme".to_owned()),
        LayerName::Named("components".to_owned()),
      ])
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_parse_nested_layer_inside_style_rule_preserves_parent_selector() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          @layer theme {
            width: 100px;
            .title { height: 20px; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(selector_text(&sheet.rules[1]), ":is(.card) .title");
    assert_eq!(
      sheet.rules[0].layer.as_ref(),
      Some(&vec![LayerName::Named("theme".to_owned())])
    );
    assert_eq!(
      sheet.rules[1].layer.as_ref(),
      Some(&vec![LayerName::Named("theme".to_owned())])
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[1].normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_anonymous_nested_layer_has_distinct_order() {
    let sheet = parse_stylesheet(
      r#"
        @layer theme {
          .parent { width: 10px; }

          @layer {
            .child { width: 20px; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(
      sheet.rules[0].layer.as_ref(),
      Some(&vec![LayerName::Named("theme".to_owned())])
    );
    assert_eq!(
      sheet.rules[1].layer.as_ref(),
      Some(&vec![
        LayerName::Named("theme".to_owned()),
        LayerName::Anonymous,
      ])
    );
    assert_ne!(sheet.rules[0].layer_order, sheet.rules[1].layer_order);
  }

  #[test]
  fn test_parse_layer_block_rejects_multiple_names() {
    let sheet = parse_stylesheet_loosy(
      r#"
        @layer theme, components {
          .invalid { width: 10px; }
        }

        .valid { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".valid");
    assert_eq!(sheet.rules[0].layer, None);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_nested_rules_preserves_source_order() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          width: 100px;
          & .title { color: red; }
          height: 20px;
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 3);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
    assert_eq!(selector_text(&sheet.rules[1]), ":is(.card) .title");
    assert_eq!(selector_text(&sheet.rules[2]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[2].normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_nested_unsupported_supports_rule_is_discarded() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          width: 100px;
          @supports (unknown-prop: nope) {
            height: 20px;
            & .title { color: red; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");

    let computed = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(computed.width, Length::Px(100.0));
    assert_eq!(computed.height, Length::Auto);
  }

  #[test]
  fn test_nested_keyframes_rule_is_rejected() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .card {
          width: 100px;
          @keyframes pulse {
            from { opacity: 0; }
            to { opacity: 1; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.keyframes.len(), 0);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_nested_property_rule_is_rejected() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .card {
          width: 100px;
          @property --box-size {
            syntax: "<length>";
            inherits: false;
            initial-value: 10px;
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert!(sheet.property_rules.is_empty());
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_lossy_parse_rejects_unknown_rules_and_keeps_valid_siblings() {
    for css in [
      r#"
        @media (resolution: 2dppx) {
          .card { width: 10px; }
        }

        .card { width: 100px; }
      "#,
      r#"
        @unknown something {
          .card { width: 10px; }
        }

        .card { width: 100px; }
      "#,
    ] {
      assert_lossy_parse_keeps_single_valid_rule(css);
    }
  }
}
