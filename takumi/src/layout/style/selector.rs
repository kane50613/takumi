use cssparser::*;
use precomputed_hash::PrecomputedHash;
use selectors::parser::{
  Component, NonTSPseudoClass, ParseRelative, PseudoElement, Selector, SelectorImpl, SelectorList,
  SelectorParseErrorKind,
};
use std::{
  borrow::Cow,
  fmt::{self, Write},
  mem::take,
  rc::Rc,
};
use taffy::Size;

use crate::keyframes::{KeyframePreludeParseError, parse_keyframe_prelude};
use crate::{
  layout::{
    Viewport,
    style::{CalcArena, FromCss, KeyframeRule, KeyframesRule, Length, StyleDeclarationBlock},
  },
  rendering::Sizing,
};

#[derive(Debug, Clone)]
pub enum CssSelectorParseError<'i> {
  #[allow(dead_code)]
  Basic(BasicParseErrorKind<'i>),
  #[allow(dead_code)]
  Property(Cow<'i, str>),
  #[allow(dead_code)]
  Selector(SelectorParseErrorKind<'i>),
  #[allow(dead_code)]
  UnsupportedSelectorFeature(&'static str),
  #[allow(dead_code)]
  InvalidAtRule(&'static str),
}

impl<'i> From<SelectorParseErrorKind<'i>> for CssSelectorParseError<'i> {
  fn from(err: SelectorParseErrorKind<'i>) -> Self {
    CssSelectorParseError::Selector(err)
  }
}

impl<'i> From<Cow<'i, str>> for CssSelectorParseError<'i> {
  fn from(err: Cow<'i, str>) -> Self {
    CssSelectorParseError::Property(err)
  }
}

impl<'i> From<KeyframePreludeParseError<'i>> for CssSelectorParseError<'i> {
  fn from(_err: KeyframePreludeParseError<'i>) -> Self {
    Self::Basic(BasicParseErrorKind::QualifiedRuleInvalid)
  }
}

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
pub struct TakumiIdent(pub String);

impl From<&str> for TakumiIdent {
  fn from(s: &str) -> Self {
    Self(s.to_owned())
  }
}

impl AsRef<str> for TakumiIdent {
  fn as_ref(&self) -> &str {
    &self.0
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
pub struct TakumiSelectorImpl;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DummyPseudoClass {
  #[default]
  Hover,
}

impl ToCss for DummyPseudoClass {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match self {
      DummyPseudoClass::Hover => dest.write_str(":hover"),
    }
  }
}

impl NonTSPseudoClass for DummyPseudoClass {
  type Impl = TakumiSelectorImpl;
  fn is_active_or_hover(&self) -> bool {
    *self == DummyPseudoClass::Hover
  }
  fn is_user_action_state(&self) -> bool {
    true
  }
}

// TODO: support pseudo elements
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DummyPseudoElement {
  #[default]
  Noop,
}

impl ToCss for DummyPseudoElement {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match self {
      DummyPseudoElement::Noop => dest.write_str("::noop"),
    }
  }
}

impl PseudoElement for DummyPseudoElement {
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
  type NonTSPseudoClass = DummyPseudoClass;
  type PseudoElement = DummyPseudoElement;
}

struct TakumiSelectorParser;

impl<'i> selectors::Parser<'i> for TakumiSelectorParser {
  type Impl = TakumiSelectorImpl;
  type Error = CssSelectorParseError<'i>;

  fn parse_parent_selector(&self) -> bool {
    true
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
  Rules(Vec<CssRule>),
}

fn parse_selector_list<'i, 't>(
  input: &mut Parser<'i, 't>,
  parse_relative: ParseRelative,
) -> Result<SelectorList<TakumiSelectorImpl>, ParseError<'i, CssSelectorParseError<'i>>> {
  let selectors = SelectorList::parse(&TakumiSelectorParser, input, parse_relative)?;
  ensure_supported_selector_list(&selectors).map_err(|err| input.new_custom_error(err))?;
  Ok(selectors)
}

fn selector_contains_unsupported_features(selector: &Selector<TakumiSelectorImpl>) -> bool {
  selector
    .iter_raw_match_order()
    .any(|component| match component {
      Component::AttributeInNoNamespaceExists { .. }
      | Component::AttributeInNoNamespace { .. }
      | Component::AttributeOther(_) => true,
      Component::Negation(list) | Component::Is(list) | Component::Where(list) => list
        .slice()
        .iter()
        .any(selector_contains_unsupported_features),
      Component::Has(relatives) => relatives
        .iter()
        .any(|rel| selector_contains_unsupported_features(&rel.selector)),
      Component::Slotted(inner) => selector_contains_unsupported_features(inner),
      Component::Host(Some(inner)) => selector_contains_unsupported_features(inner),
      _ => false,
    })
}

fn ensure_supported_selector_list<'i>(
  selectors: &SelectorList<TakumiSelectorImpl>,
) -> Result<(), CssSelectorParseError<'i>> {
  if selectors
    .slice()
    .iter()
    .any(selector_contains_unsupported_features)
  {
    return Err(CssSelectorParseError::UnsupportedSelectorFeature(
      "attribute selectors are not supported",
    ));
  }

  Ok(())
}

pub struct StyleDeclarationParser;

impl<'i> DeclarationParser<'i> for StyleDeclarationParser {
  type Declaration = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;

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
  type Error = CssSelectorParseError<'i>;
}

impl<'i> AtRuleParser<'i> for StyleDeclarationParser {
  type Prelude = ();
  type AtRule = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;
}

impl<'i> RuleBodyItemParser<'i, StyleDeclarationBlock, CssSelectorParseError<'i>>
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
  type Error = CssSelectorParseError<'i>;

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
  type Error = CssSelectorParseError<'i>;
}

impl<'i> AtRuleParser<'i> for PropertyRuleDeclarationParser {
  type Prelude = ();
  type AtRule = (String, String);
  type Error = CssSelectorParseError<'i>;
}

impl<'i> RuleBodyItemParser<'i, (String, String), CssSelectorParseError<'i>>
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
}

impl<'i> DeclarationParser<'i> for NestedStyleRuleParser<'_> {
  type Declaration = StyleRuleBodyItem;
  type Error = CssSelectorParseError<'i>;

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
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    parse_selector_list(input, ParseRelative::ForNesting)
  }

  fn parse_block<'t>(
    &mut self,
    nested_selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let selectors = nested_selectors.replace_parent_selector(&self.parent_selectors);
    let rules = parse_style_rule_block(selectors, self.media_queries, self.layer.as_ref(), input)?;
    Ok(StyleRuleBodyItem::Rules(rules))
  }
}

impl<'i> AtRuleParser<'i> for NestedStyleRuleParser<'_> {
  type Prelude = AtRulePrelude;
  type AtRule = StyleRuleBodyItem;
  type Error = CssSelectorParseError<'i>;

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
    let rules = parse_nested_at_rule_block(
      &self.parent_selectors,
      self.media_queries,
      self.layer.as_ref(),
      prelude,
      input,
    )?;
    Ok(StyleRuleBodyItem::Rules(rules))
  }
}

impl<'i> RuleBodyItemParser<'i, StyleRuleBodyItem, CssSelectorParseError<'i>>
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
  type Error = CssSelectorParseError<'i>;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let declarations = StyleDeclarationBlock::parse(&name, input).map_err(ParseError::into)?;
    let _ = input.try_parse(parse_important);
    Ok(declarations)
  }
}

impl<'i> QualifiedRuleParser<'i> for KeyframeDeclarationParser {
  type Prelude = ();
  type QualifiedRule = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;
}

impl<'i> AtRuleParser<'i> for KeyframeDeclarationParser {
  type Prelude = ();
  type AtRule = StyleDeclarationBlock;
  type Error = CssSelectorParseError<'i>;
}

impl<'i> RuleBodyItemParser<'i, StyleDeclarationBlock, CssSelectorParseError<'i>>
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
  type Error = CssSelectorParseError<'i>;

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
    let parser = RuleBodyParser::new(input, &mut declaration_parser);
    let mut declarations = StyleDeclarationBlock::default();
    for block in parser.filter_map(Result::ok) {
      declarations.append(block);
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
  type Error = CssSelectorParseError<'i>;
}

struct TakumiRuleParser {
  current_layer: Option<LayerPath>,
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
  Width(MediaFeatureComparison, Length<false>),
  Height(MediaFeatureComparison, Length<false>),
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
  match comparison {
    MediaFeatureComparison::Equal => (actual - expected).abs() <= f32::EPSILON,
    MediaFeatureComparison::Min => actual >= expected,
    MediaFeatureComparison::Max => actual <= expected,
  }
}

fn parse_media_query_list<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<MediaQueryList, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut queries = Vec::new();

  loop {
    queries.push(parse_media_query(input)?);

    if input.try_parse(Parser::expect_comma).is_err() {
      break;
    }
  }

  Ok(MediaQueryList { queries })
}

fn parse_media_query<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<MediaQuery, ParseError<'i, CssSelectorParseError<'i>>> {
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
) -> Result<(), ParseError<'i, CssSelectorParseError<'i>>> {
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
) -> Result<MediaFeature, ParseError<'i, CssSelectorParseError<'i>>> {
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

  let length = Length::<false>::from_css(input).map_err(ParseError::into)?;

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
    Err(
      input.new_custom_error(CssSelectorParseError::UnsupportedSelectorFeature(
        "unsupported media feature",
      )),
    )
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

fn parse_fragment(
  input: &mut Parser<'_, '_>,
  current_layer: Option<&LayerPath>,
) -> StyleSheetFragment {
  let mut parser = TakumiRuleParser {
    current_layer: current_layer.cloned(),
  };
  StyleSheetParser::new(input, &mut parser)
    .filter_map(Result::ok)
    .fold(StyleSheetFragment::default(), |mut fragment, nested| {
      fragment.extend(nested);
      fragment
    })
}

#[derive(Debug, Clone)]
pub struct CssRule {
  pub selectors: SelectorList<TakumiSelectorImpl>,
  pub normal_declarations: StyleDeclarationBlock,
  pub important_declarations: StyleDeclarationBlock,
  pub media_queries: Vec<MediaQueryList>,
  pub layer: Option<LayerPath>,
  pub layer_order: Option<usize>,
}

fn parse_property_rule<'i, 't>(
  property_name: String,
  input: &mut Parser<'i, 't>,
) -> Result<PropertyRule, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut parser = PropertyRuleDeclarationParser;
  let mut syntax = None;
  let mut inherits = None;
  let mut initial_value = None;

  for entry in RuleBodyParser::new(input, &mut parser).filter_map(Result::ok) {
    let (name, value) = entry;
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

      return Err(input.new_custom_error(CssSelectorParseError::InvalidAtRule(
        "@property inherits must be true or false",
      )));
    }

    if name.eq_ignore_ascii_case("initial-value") {
      initial_value = Some(value);
    }
  }

  let Some(syntax) = syntax else {
    return Err(input.new_custom_error(CssSelectorParseError::InvalidAtRule(
      "missing `@property` syntax",
    )));
  };
  let Some(inherits) = inherits else {
    return Err(input.new_custom_error(CssSelectorParseError::InvalidAtRule(
      "missing `@property` inherits",
    )));
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
) -> Result<bool, ParseError<'i, CssSelectorParseError<'i>>> {
  let name = input.expect_ident_cloned()?;
  input.expect_colon()?;
  let declaration = StyleDeclarationBlock::parse(&name, input).map_err(ParseError::into)?;
  Ok(!declaration.declarations.is_empty() && input.is_exhausted())
}

fn parse_supports_in_parens<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<bool, ParseError<'i, CssSelectorParseError<'i>>> {
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
) -> Result<bool, ParseError<'i, CssSelectorParseError<'i>>> {
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
) -> Result<bool, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut result = parse_supports_not(input)?;
  let mut operator = None;

  loop {
    if input
      .try_parse(|input| input.expect_ident_matching("and"))
      .is_ok()
    {
      if matches!(operator, Some(false)) {
        return Err(input.new_custom_error(CssSelectorParseError::InvalidAtRule(
          "@supports cannot mix `and` and `or` without parentheses",
        )));
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
        return Err(input.new_custom_error(CssSelectorParseError::InvalidAtRule(
          "@supports cannot mix `and` and `or` without parentheses",
        )));
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
) -> Result<AtRulePrelude, ParseError<'i, CssSelectorParseError<'i>>> {
  if name.eq_ignore_ascii_case("layer") {
    let mut layer_names = Vec::new();
    while let Ok(layer_name) = input.try_parse(parse_layer_name) {
      layer_names.push(layer_name);
      if input.try_parse(Parser::expect_comma).is_err() {
        break;
      }
    }
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
    return parse_media_query_list(input).map(AtRulePrelude::Media);
  }

  if name.eq_ignore_ascii_case("supports") {
    return parse_supports_condition(input).map(AtRulePrelude::Supports);
  }

  if name.eq_ignore_ascii_case("property") {
    let property_name = input.expect_ident_or_string()?.to_string();
    if !property_name.starts_with("--") {
      return Err(input.new_custom_error(CssSelectorParseError::InvalidAtRule(
        "@property name must be a custom property",
      )));
    }
    return Ok(AtRulePrelude::Property(property_name));
  }

  Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)))
}

fn parse_layer_name<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<LayerPath, ParseError<'i, CssSelectorParseError<'i>>> {
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
) -> Result<(), ParseError<'i, CssSelectorParseError<'i>>> {
  if layer_names.len() <= 1 {
    return Ok(());
  }

  Err(input.new_custom_error(CssSelectorParseError::InvalidAtRule(
    "@layer blocks accept at most one name",
  )))
}

fn parse_style_rule_block<'i, 't>(
  selectors: SelectorList<TakumiSelectorImpl>,
  media_queries: &[MediaQueryList],
  layer: Option<&LayerPath>,
  input: &mut Parser<'i, 't>,
) -> Result<Vec<CssRule>, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut normal_declarations = StyleDeclarationBlock::default();
  let mut important_declarations = StyleDeclarationBlock::default();
  let layer = layer.cloned();
  let mut rules = Vec::new();
  let mut parser = NestedStyleRuleParser {
    parent_selectors: selectors.clone(),
    media_queries,
    layer: layer.clone(),
  };

  for result in RuleBodyParser::new(input, &mut parser) {
    match result {
      Ok(StyleRuleBodyItem::Declarations(declarations)) => {
        let declarations = *declarations;
        if declarations.importance.is_empty() {
          normal_declarations.append(declarations);
        } else {
          important_declarations.append(declarations);
        }
      }
      Ok(StyleRuleBodyItem::Rules(mut nested_rules)) => {
        if !normal_declarations.declarations.is_empty()
          || !important_declarations.declarations.is_empty()
        {
          rules.push(CssRule {
            selectors: selectors.clone(),
            normal_declarations: take(&mut normal_declarations),
            important_declarations: take(&mut important_declarations),
            media_queries: media_queries.to_vec(),
            layer: layer.clone(),
            layer_order: None,
          });
        }
        rules.append(&mut nested_rules);
      }
      Err((_error, _body)) => continue,
    }
  }

  if normal_declarations.declarations.is_empty() && important_declarations.declarations.is_empty() {
    return Ok(rules);
  }

  rules.push(CssRule {
    selectors,
    normal_declarations,
    important_declarations,
    media_queries: media_queries.to_vec(),
    layer,
    layer_order: None,
  });
  Ok(rules)
}

fn parse_nested_at_rule_block<'i, 't>(
  parent_selectors: &SelectorList<TakumiSelectorImpl>,
  media_queries: &[MediaQueryList],
  current_layer: Option<&LayerPath>,
  prelude: AtRulePrelude,
  input: &mut Parser<'i, 't>,
) -> Result<Vec<CssRule>, ParseError<'i, CssSelectorParseError<'i>>> {
  match prelude {
    AtRulePrelude::Layer(layer_names) => {
      ensure_single_layer_name(&layer_names, input)?;
      let Some(layer_name) = layer_names.into_iter().next() else {
        return Ok(Vec::new());
      };
      let nested_layer = extend_layer_name(current_layer, &layer_name);
      parse_style_rule_block(
        parent_selectors.clone(),
        media_queries,
        nested_layer.as_ref(),
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
        input,
      )
    }
    AtRulePrelude::Supports(true) => parse_style_rule_block(
      parent_selectors.clone(),
      media_queries,
      current_layer,
      input,
    ),
    AtRulePrelude::Supports(false) => {
      let mut parser = NestedStyleRuleParser {
        parent_selectors: parent_selectors.clone(),
        media_queries,
        layer: current_layer.cloned(),
      };
      for _ in RuleBodyParser::new(input, &mut parser) {}
      Ok(Vec::new())
    }
    AtRulePrelude::Keyframes(_) | AtRulePrelude::Property(_) => Err(input.new_custom_error(
      CssSelectorParseError::InvalidAtRule("unsupported nested at-rule"),
    )),
  }
}

impl<'i> QualifiedRuleParser<'i> for TakumiRuleParser {
  type Prelude = ParsedSelectors;
  type QualifiedRule = StyleSheetFragment;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    Ok(ParsedSelectors {
      selectors: parse_selector_list(input, ParseRelative::No)?,
    })
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    Ok(StyleSheetFragment {
      rules: parse_style_rule_block(selectors.selectors, &[], self.current_layer.as_ref(), input)?,
      ..StyleSheetFragment::default()
    })
  }
}

impl<'i> AtRuleParser<'i> for TakumiRuleParser {
  type Prelude = AtRulePrelude;
  type AtRule = StyleSheetFragment;
  type Error = CssSelectorParseError<'i>;

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
        let mut fragment = parse_fragment(input, nested_layer.as_ref());
        fragment.declared_layers.splice(0..0, declared_layers);
        Ok(fragment)
      }
      AtRulePrelude::Keyframes(name) => {
        let mut parser = KeyframeRuleParser;
        let rule_list_parser = StyleSheetParser::new(input, &mut parser);
        let keyframes = rule_list_parser.filter_map(Result::ok).collect::<Vec<_>>();

        Ok(StyleSheetFragment {
          keyframes: vec![KeyframesRule {
            name,
            keyframes,
            media_queries: Vec::new(),
          }],
          ..StyleSheetFragment::default()
        })
      }
      AtRulePrelude::Media(media_query) => {
        let mut fragment = parse_fragment(input, self.current_layer.as_ref());

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
          let mut parser = TakumiRuleParser {
            current_layer: self.current_layer.clone(),
          };
          for _ in StyleSheetParser::new(input, &mut parser) {}
          return Ok(StyleSheetFragment::default());
        }

        Ok(parse_fragment(input, self.current_layer.as_ref()))
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

#[derive(Debug, Clone, Default)]
pub(crate) struct StyleSheet {
  pub rules: Vec<CssRule>,
  pub keyframes: Vec<KeyframesRule>,
  pub property_rules: Vec<PropertyRule>,
}

impl StyleSheet {
  pub(crate) fn parse_list<'a, I>(stylesheets: I) -> impl Iterator<Item = Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    stylesheets.into_iter().map(Self::parse)
  }

  pub(crate) fn parse(css: &str) -> Self {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rule_parser = TakumiRuleParser {
      current_layer: None,
    };
    let mut rules = Vec::new();
    let mut keyframes = Vec::new();
    let mut property_rules = Vec::new();
    let mut declared_layers = Vec::new();

    let rule_list_parser = StyleSheetParser::new(&mut parser, &mut rule_parser);

    for fragment in rule_list_parser.filter_map(Result::ok) {
      rules.extend(fragment.rules);
      keyframes.extend(fragment.keyframes);
      property_rules.extend(fragment.property_rules);
      declared_layers.extend(fragment.declared_layers);
    }

    let mut layer_order = std::collections::HashMap::<LayerPath, usize>::new();
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

    Self {
      rules,
      keyframes,
      property_rules,
    }
  }
}

#[cfg(test)]
#[path = "selector_tests.rs"]
mod tests;
