use cssparser::*;
use precomputed_hash::PrecomputedHash;
use selectors::parser::{
  Component, NonTSPseudoClass, ParseRelative, PseudoElement, Selector, SelectorImpl, SelectorList,
  SelectorParseErrorKind,
};
use std::{
  borrow::Cow,
  fmt::{self, Write},
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PropertyRule {
  pub name: String,
  pub syntax: String,
  pub inherits: bool,
  pub initial_value: String,
}

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
  text: String,
}

#[derive(Debug, Clone, Default)]
struct StyleSheetFragment {
  rules: Vec<CssRule>,
  keyframes: Vec<KeyframesRule>,
  property_rules: Vec<PropertyRule>,
}

impl StyleSheetFragment {
  fn extend(&mut self, other: Self) {
    self.rules.extend(other.rules);
    self.keyframes.extend(other.keyframes);
    self.property_rules.extend(other.property_rules);
  }
}

#[derive(Debug)]
enum StyleRuleBodyItem {
  Declarations(Box<StyleDeclarationBlock>),
  Rules(Vec<CssRule>),
}

fn parse_selector_list_text(
  selector_text: &str,
  parse_relative: ParseRelative,
) -> Result<SelectorList<TakumiSelectorImpl>, &'static str> {
  let mut input = ParserInput::new(selector_text);
  let mut parser = Parser::new(&mut input);
  let selectors = SelectorList::parse(&TakumiSelectorParser, &mut parser, parse_relative)
    .map_err(|_| "invalid selector")?;
  ensure_supported_selector_list(&selectors).map_err(|error| match error {
    CssSelectorParseError::UnsupportedSelectorFeature(message) => message,
    _ => "invalid selector",
  })?;
  Ok(selectors)
}

fn split_selector_list(selector_text: &str) -> Vec<String> {
  let mut selectors = Vec::new();
  let mut start = 0usize;
  let mut paren_depth = 0usize;
  let mut bracket_depth = 0usize;
  let mut brace_depth = 0usize;
  let mut string_delimiter: Option<char> = None;
  let mut escaped = false;

  for (index, ch) in selector_text.char_indices() {
    if let Some(delimiter) = string_delimiter {
      if escaped {
        escaped = false;
        continue;
      }

      match ch {
        '\\' => escaped = true,
        _ if ch == delimiter => string_delimiter = None,
        _ => {}
      }
      continue;
    }

    match ch {
      '"' | '\'' => string_delimiter = Some(ch),
      '(' => paren_depth += 1,
      ')' => paren_depth = paren_depth.saturating_sub(1),
      '[' => bracket_depth += 1,
      ']' => bracket_depth = bracket_depth.saturating_sub(1),
      '{' => brace_depth += 1,
      '}' => brace_depth = brace_depth.saturating_sub(1),
      ',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
        selectors.push(selector_text[start..index].trim().to_owned());
        start = index + ch.len_utf8();
      }
      _ => {}
    }
  }

  let trailing = selector_text[start..].trim();
  if !trailing.is_empty() {
    selectors.push(trailing.to_owned());
  }

  selectors
}

fn flatten_nested_selector_text(parent_selector_text: &str, nested_selector_text: &str) -> String {
  let parents = split_selector_list(parent_selector_text);
  let nested_selectors = split_selector_list(nested_selector_text);
  let mut flattened = Vec::new();

  for parent in &parents {
    for nested in &nested_selectors {
      let selector = if nested.contains('&') {
        nested.replace('&', parent)
      } else {
        format!("{parent} {nested}")
      };
      flattened.push(selector.trim().to_owned());
    }
  }

  flattened.join(", ")
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
  parent_selector_text: String,
  media_queries: &'a [MediaQueryList],
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
  type Prelude = String;
  type QualifiedRule = StyleRuleBodyItem;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    let start = input.position();
    let selectors = SelectorList::parse(&TakumiSelectorParser, input, ParseRelative::ForNesting)?;
    ensure_supported_selector_list(&selectors).map_err(|err| input.new_custom_error(err))?;
    Ok(input.slice_from(start).trim().to_owned())
  }

  fn parse_block<'t>(
    &mut self,
    selector_text: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let flattened_selector_text =
      flatten_nested_selector_text(&self.parent_selector_text, &selector_text);
    let rules = parse_style_rule_block(&flattened_selector_text, self.media_queries, input)?;
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
      &self.parent_selector_text,
      self.media_queries,
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

struct TakumiRuleParser;

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
  Layer,
  Media(MediaQueryList),
  Property(String),
  Supports(bool),
}

fn parse_fragment(input: &mut Parser<'_, '_>) -> StyleSheetFragment {
  let mut parser = TakumiRuleParser;
  StyleSheetParser::new(input, &mut parser)
    .filter_map(Result::ok)
    .fold(StyleSheetFragment::default(), |mut fragment, nested| {
      fragment.extend(nested);
      fragment
    })
}

#[derive(Debug, Clone)]
pub struct CssRule {
  #[cfg_attr(not(test), allow(dead_code))]
  pub selector_text: String,
  pub selectors: SelectorList<TakumiSelectorImpl>,
  pub normal_declarations: StyleDeclarationBlock,
  pub important_declarations: StyleDeclarationBlock,
  pub media_queries: Vec<MediaQueryList>,
}

fn parse_property_rule<'i, 't>(
  property_name: &str,
  input: &mut Parser<'i, 't>,
) -> Result<PropertyRule, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut parser = PropertyRuleDeclarationParser;
  let mut syntax = None;
  let mut inherits = None;
  let mut initial_value = None;

  for entry in RuleBodyParser::new(input, &mut parser).filter_map(Result::ok) {
    match entry.0.as_str() {
      "syntax" => syntax = Some(entry.1),
      "inherits" => {
        let value = if entry.1.eq_ignore_ascii_case("true") {
          true
        } else if entry.1.eq_ignore_ascii_case("false") {
          false
        } else {
          return Err(input.new_custom_error(CssSelectorParseError::InvalidAtRule(
            "@property inherits must be true or false",
          )));
        };
        inherits = Some(value);
      }
      "initial-value" => initial_value = Some(entry.1),
      _ => {}
    }
  }

  let syntax = syntax.unwrap_or_else(|| "*".to_owned());
  let initial_value = initial_value.unwrap_or_default();

  Ok(PropertyRule {
    name: property_name.to_owned(),
    syntax,
    inherits: inherits.unwrap_or(true),
    initial_value,
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

fn parse_supports_and<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<bool, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut result = parse_supports_not(input)?;
  while input
    .try_parse(|input| input.expect_ident_matching("and"))
    .is_ok()
  {
    result &= parse_supports_not(input)?;
  }
  Ok(result)
}

fn parse_supports_condition<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<bool, ParseError<'i, CssSelectorParseError<'i>>> {
  let mut result = parse_supports_and(input)?;
  while input
    .try_parse(|input| input.expect_ident_matching("or"))
    .is_ok()
  {
    result |= parse_supports_and(input)?;
  }
  Ok(result)
}

fn parse_at_rule_prelude<'i, 't>(
  name: CowRcStr<'i>,
  input: &mut Parser<'i, 't>,
) -> Result<AtRulePrelude, ParseError<'i, CssSelectorParseError<'i>>> {
  if name.eq_ignore_ascii_case("layer") {
    while input
      .try_parse(
        |input| -> Result<String, ParseError<'i, CssSelectorParseError<'i>>> {
          let location = input.current_source_location();
          match input.next()? {
            Token::Ident(value) | Token::QuotedString(value) => Ok(value.to_string()),
            token => Err(location.new_unexpected_token_error(token.clone())),
          }
        },
      )
      .is_ok()
    {
      if input.try_parse(Parser::expect_comma).is_err() {
        break;
      }
    }
    return Ok(AtRulePrelude::Layer);
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

fn parse_style_rule_block<'i, 't>(
  selector_text: &str,
  media_queries: &[MediaQueryList],
  input: &mut Parser<'i, 't>,
) -> Result<Vec<CssRule>, ParseError<'i, CssSelectorParseError<'i>>> {
  let selectors =
    parse_selector_list_text(selector_text, ParseRelative::No).map_err(|message| {
      input.new_custom_error(CssSelectorParseError::UnsupportedSelectorFeature(message))
    })?;
  let mut normal_declarations = StyleDeclarationBlock::default();
  let mut important_declarations = StyleDeclarationBlock::default();
  let mut nested_rules = Vec::new();
  let mut parser = NestedStyleRuleParser {
    parent_selector_text: selector_text.to_owned(),
    media_queries,
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
      Ok(StyleRuleBodyItem::Rules(mut rules)) => nested_rules.append(&mut rules),
      Err((_error, _body)) => continue,
    }
  }

  if normal_declarations.declarations.is_empty() && important_declarations.declarations.is_empty() {
    return Ok(nested_rules);
  }

  let mut rules = Vec::with_capacity(nested_rules.len() + 1);
  rules.push(CssRule {
    selector_text: selector_text.to_owned(),
    selectors,
    normal_declarations,
    important_declarations,
    media_queries: media_queries.to_vec(),
  });
  rules.append(&mut nested_rules);
  Ok(rules)
}

fn parse_nested_at_rule_block<'i, 't>(
  parent_selector_text: &str,
  media_queries: &[MediaQueryList],
  prelude: AtRulePrelude,
  input: &mut Parser<'i, 't>,
) -> Result<Vec<CssRule>, ParseError<'i, CssSelectorParseError<'i>>> {
  match prelude {
    AtRulePrelude::Layer => Ok(parse_fragment(input).rules),
    AtRulePrelude::Media(media_query) => {
      let mut merged_media_queries = media_queries.to_vec();
      merged_media_queries.push(media_query);
      parse_style_rule_block(parent_selector_text, &merged_media_queries, input)
    }
    AtRulePrelude::Supports(true) => {
      parse_style_rule_block(parent_selector_text, media_queries, input)
    }
    AtRulePrelude::Supports(false) => {
      let mut parser = NestedStyleRuleParser {
        parent_selector_text: parent_selector_text.to_owned(),
        media_queries,
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
    let start = input.position();
    let selectors = SelectorList::parse(&TakumiSelectorParser, input, ParseRelative::No)?;
    ensure_supported_selector_list(&selectors).map_err(|err| input.new_custom_error(err))?;
    Ok(ParsedSelectors {
      text: input.slice_from(start).trim().to_owned(),
    })
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    Ok(StyleSheetFragment {
      rules: parse_style_rule_block(&selectors.text, &[], input)?,
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
      AtRulePrelude::Layer => Ok(parse_fragment(input)),
      AtRulePrelude::Keyframes(name) => {
        let mut parser = KeyframeRuleParser;
        let rule_list_parser = StyleSheetParser::new(input, &mut parser);
        let keyframes = rule_list_parser.filter_map(Result::ok).collect::<Vec<_>>();

        Ok(StyleSheetFragment {
          keyframes: vec![KeyframesRule { name, keyframes }],
          ..StyleSheetFragment::default()
        })
      }
      AtRulePrelude::Media(media_query) => {
        let mut fragment = parse_fragment(input);

        for rule in &mut fragment.rules {
          rule.media_queries.push(media_query.clone());
        }

        Ok(fragment)
      }
      AtRulePrelude::Supports(is_supported) => {
        if !is_supported {
          let mut parser = TakumiRuleParser;
          for _ in StyleSheetParser::new(input, &mut parser) {}
          return Ok(StyleSheetFragment::default());
        }

        Ok(parse_fragment(input))
      }
      AtRulePrelude::Property(name) => Ok(StyleSheetFragment {
        property_rules: vec![parse_property_rule(&name, input)?],
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
      AtRulePrelude::Layer => Ok(StyleSheetFragment::default()),
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
    let mut rule_parser = TakumiRuleParser;
    let mut rules = Vec::new();
    let mut keyframes = Vec::new();
    let mut property_rules = Vec::new();

    let rule_list_parser = StyleSheetParser::new(&mut parser, &mut rule_parser);

    for fragment in rule_list_parser.filter_map(Result::ok) {
      rules.extend(fragment.rules);
      keyframes.extend(fragment.keyframes);
      property_rules.extend(fragment.property_rules);
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
