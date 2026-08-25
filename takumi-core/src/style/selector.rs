use std::{
  collections::HashMap,
  fmt::{self, Write},
  mem::take,
  ops::Deref,
};

use cssparser::*;
use precomputed_hash::PrecomputedHash;
use selectors::parser::{
  NonTSPseudoClass, ParseRelative, PseudoElement as PseudoElementTrait,
  SelectorImpl as SelectorImplTrait, SelectorList,
};

pub use crate::style::media_query::MediaQueryList;
use crate::{
  error::StyleSheetParseError,
  keyframes::parse_keyframe_prelude,
  style::{
    BreakpointOverrides, FromCssStr, KeyframeRule, KeyframesRule, Length, StyleDeclaration,
    StyleDeclarationBlock, supports::parse_supports_condition,
  },
};

/// A registered custom property from an `@property` rule.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyRule {
  /// Custom property name, including the leading `--`.
  pub name: String,
  /// Declared `syntax` descriptor.
  pub syntax: String,
  /// Whether the property inherits.
  pub inherits: bool,
  /// Declared `initial-value`, if any.
  pub initial_value: Option<String>,
  /// Media queries this rule is gated behind.
  pub media_queries: Vec<MediaQueryList>,
}

/// A name in a cascade layer path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum LayerName {
  /// An explicitly named layer.
  Named(String),
  /// An anonymous layer.
  Anonymous,
}

type LayerPath = Vec<LayerName>;

/// An ASCII-case-insensitive CSS identifier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Ident(String);

impl Deref for Ident {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl AsRef<str> for Ident {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl PartialEq<&str> for Ident {
  fn eq(&self, other: &&str) -> bool {
    self.0 == *other
  }
}

impl PartialEq<Ident> for &str {
  fn eq(&self, other: &Ident) -> bool {
    self == &other.0
  }
}
impl From<&str> for Ident {
  fn from(s: &str) -> Self {
    Self(s.to_owned())
  }
}

impl ToCss for Ident {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    serialize_identifier(&self.0, dest)
  }
}

impl PrecomputedHash for Ident {
  fn precomputed_hash(&self) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for byte in self.0.as_bytes() {
      hash ^= u32::from(byte.to_ascii_lowercase());
      hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
  }
}

/// Selector type bindings for the `selectors` crate.
#[derive(Debug, Clone)]
pub(crate) struct SelectorImpl;

// Most pseudo-classes are parsed but never matched, so rules with `:hover`, `::before`,
// etc. survive alongside their sibling selectors instead of getting dropped wholesale.
// `:lang()` is matched for real (see `crate::matching`) — it needs no live/interactive
// state, just the `lang` attribute inherited up the tree, which a static render has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PseudoClass {
  /// `:lang(en, fr, ...)` — one or more comma-separated BCP-47 basic-filtering ranges.
  Lang(Vec<Box<str>>),
  /// Any other pseudo-class, parsed but never matched.
  Other(Ident),
}

impl ToCss for PseudoClass {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match self {
      Self::Lang(ranges) => {
        dest.write_str(":lang(")?;
        for (i, range) in ranges.iter().enumerate() {
          if i > 0 {
            dest.write_str(", ")?;
          }
          dest.write_str(range)?;
        }
        dest.write_char(')')
      }
      Self::Other(name) => {
        dest.write_char(':')?;
        name.to_css(dest)
      }
    }
  }
}

impl NonTSPseudoClass for PseudoClass {
  type Impl = SelectorImpl;
  fn is_active_or_hover(&self) -> bool {
    false
  }
  fn is_user_action_state(&self) -> bool {
    false
  }
}

/// A pseudo-element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PseudoElement {
  /// `::before`.
  Before,
  /// `::after`.
  After,
  /// Any other pseudo-element, kept but never matched.
  Other(Ident),
}

impl PseudoElement {
  fn from_name(name: &str) -> Self {
    if name.eq_ignore_ascii_case("before") {
      Self::Before
    } else if name.eq_ignore_ascii_case("after") {
      Self::After
    } else {
      Self::Other(Ident::from(name))
    }
  }

  fn as_str(&self) -> &str {
    match self {
      Self::Before => "before",
      Self::After => "after",
      Self::Other(name) => name,
    }
  }
}

impl ToCss for PseudoElement {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    dest.write_str("::")?;
    serialize_identifier(self.as_str(), dest)
  }
}

impl PseudoElementTrait for PseudoElement {
  type Impl = SelectorImpl;
}

impl SelectorImplTrait for SelectorImpl {
  type ExtraMatchingData<'a> = ();
  type AttrValue = Ident;
  type Identifier = Ident;
  type LocalName = Ident;
  type NamespaceUrl = Ident;
  type NamespacePrefix = Ident;
  type BorrowedNamespaceUrl = Ident;
  type BorrowedLocalName = Ident;
  type NonTSPseudoClass = PseudoClass;
  type PseudoElement = PseudoElement;
}

struct TakumiSelectorParser;

impl<'i> selectors::Parser<'i> for TakumiSelectorParser {
  type Impl = SelectorImpl;
  type Error = StyleSheetParseError;

  fn parse_parent_selector(&self) -> bool {
    true
  }

  fn parse_host(&self) -> bool {
    true
  }

  fn parse_is_and_where(&self) -> bool {
    true
  }

  fn parse_non_ts_pseudo_class(
    &self,
    _location: SourceLocation,
    name: CowRcStr<'i>,
  ) -> Result<<Self::Impl as SelectorImplTrait>::NonTSPseudoClass, ParseError<'i, Self::Error>> {
    Ok(PseudoClass::Other(Ident::from(&*name)))
  }

  fn parse_non_ts_functional_pseudo_class<'t>(
    &self,
    name: CowRcStr<'i>,
    parser: &mut Parser<'i, 't>,
    _after_part: bool,
  ) -> Result<<Self::Impl as SelectorImplTrait>::NonTSPseudoClass, ParseError<'i, Self::Error>> {
    if name.eq_ignore_ascii_case("lang") {
      let ranges = parser.parse_comma_separated(|input| {
        let token = input.next()?.clone();
        match &token {
          Token::Ident(ident) => Ok(ident.as_ref().into()),
          Token::QuotedString(s) => Ok(s.as_ref().into()),
          _ => Err(input.new_unexpected_token_error(token)),
        }
      })?;
      return Ok(PseudoClass::Lang(ranges));
    }

    while parser.next_including_whitespace_and_comments().is_ok() {}
    Ok(PseudoClass::Other(Ident::from(&*name)))
  }

  fn parse_pseudo_element(
    &self,
    _location: SourceLocation,
    name: CowRcStr<'i>,
  ) -> Result<<Self::Impl as SelectorImplTrait>::PseudoElement, ParseError<'i, Self::Error>> {
    Ok(PseudoElement::from_name(&name))
  }

  fn parse_functional_pseudo_element<'t>(
    &self,
    name: CowRcStr<'i>,
    arguments: &mut Parser<'i, 't>,
  ) -> Result<<Self::Impl as SelectorImplTrait>::PseudoElement, ParseError<'i, Self::Error>> {
    while arguments.next_including_whitespace_and_comments().is_ok() {}
    Ok(PseudoElement::Other(Ident::from(&*name)))
  }
}

#[derive(Debug, Clone, Default)]
struct StyleSheetFragment {
  rules: Vec<CssRule>,
  keyframes: Vec<KeyframesRule>,
  property_rules: Vec<PropertyRule>,
  declared_layers: Vec<LayerPath>,
  preflight: bool,
}

impl StyleSheetFragment {
  fn extend(&mut self, other: Self) {
    self.rules.extend(other.rules);
    self.keyframes.extend(other.keyframes);
    self.property_rules.extend(other.property_rules);
    self.declared_layers.extend(other.declared_layers);
    self.preflight |= other.preflight;
  }
}

#[derive(Debug)]
enum StyleRuleBodyItem {
  Declarations(Box<StyleDeclarationBlock>),
  Rules(StyleSheetFragment),
}

macro_rules! impl_parser_traits {
  ($parser:ty, $item:ty) => {
    impl<'i> QualifiedRuleParser<'i> for $parser {
      type Prelude = ();
      type QualifiedRule = $item;
      type Error = StyleSheetParseError;
    }

    impl<'i> AtRuleParser<'i> for $parser {
      type Prelude = ();
      type AtRule = $item;
      type Error = StyleSheetParseError;
    }

    impl<'i> RuleBodyItemParser<'i, $item, StyleSheetParseError> for $parser {
      fn parse_qualified(&self) -> bool {
        false
      }
      fn parse_declarations(&self) -> bool {
        true
      }
    }
  };
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

impl_parser_traits!(StyleDeclarationParser, StyleDeclarationBlock);

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

impl_parser_traits!(PropertyRuleDeclarationParser, (String, String));

struct NestedStyleRuleParser<'a> {
  parent_selectors: SelectorList<SelectorImpl>,
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
  type Prelude = SelectorList<SelectorImpl>;
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

impl_parser_traits!(KeyframeDeclarationParser, StyleDeclarationBlock);

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

#[derive(Debug, Clone)]
enum AtRulePrelude {
  Keyframes(String),
  Layer(Vec<LayerPath>),
  Media(MediaQueryList),
  Property(String),
  Supports(bool),
  /// Tailwind's `@theme`, read as the `:root` rule it compiles to. Modifiers
  /// are accepted but read the same way: takumi resolves `var()` through the
  /// cascade at computed time, so `reference` and `inline` collapse into the
  /// emitted rule.
  Theme,
  /// `@import "tailwindcss"`, which here only turns Preflight on: the built-in
  /// scales already back every utility and `tw` is already the last layer.
  TailwindImport,
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

/// A style rule with its selectors and declaration blocks. Opaque: its
/// `selectors` crate innards stay out of takumi's public API.
#[derive(Debug, Clone)]
pub(crate) struct CssRule {
  /// Selectors this rule applies to.
  pub(crate) selectors: SelectorList<SelectorImpl>,
  /// Declarations without `!important`.
  pub(crate) normal_declarations: StyleDeclarationBlock,
  /// Declarations marked `!important`.
  pub(crate) important_declarations: StyleDeclarationBlock,
  /// Media queries gating this rule.
  pub(crate) media_queries: Vec<MediaQueryList>,
  /// Cascade layer this rule belongs to.
  pub(crate) layer: Option<LayerPath>,
  /// Resolved ordinal of the rule's layer.
  pub(crate) layer_order: Option<usize>,
}

impl CssRule {
  /// The selectors this rule applies to.
  pub(crate) fn selectors(&self) -> &SelectorList<SelectorImpl> {
    &self.selectors
  }
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

/// Parses a `@theme` block body: declarations landing on `:root`, plus any
/// `@keyframes` the theme carries, the way Tailwind's own `theme.css` pairs
/// `--animate-*` tokens with their keyframes.
struct ThemeBlockParser<'a> {
  media_queries: &'a [MediaQueryList],
  lossy: bool,
}

enum ThemeBodyItem {
  Declarations(Box<StyleDeclarationBlock>),
  Keyframes(StyleSheetFragment),
}

impl<'i> DeclarationParser<'i> for ThemeBlockParser<'_> {
  type Declaration = ThemeBodyItem;
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
      .map(ThemeBodyItem::Declarations)
  }
}

impl<'i> AtRuleParser<'i> for ThemeBlockParser<'_> {
  type Prelude = AtRulePrelude;
  type AtRule = ThemeBodyItem;
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
      AtRulePrelude::Keyframes(name) => {
        parse_keyframes_block(name, self.media_queries, self.lossy, input)
          .map(ThemeBodyItem::Keyframes)
      }
      _ => Err(input.new_custom_error(StyleSheetParseError::unsupported_nested_at_rule())),
    }
  }
}

impl<'i> QualifiedRuleParser<'i> for ThemeBlockParser<'_> {
  type Prelude = ();
  type QualifiedRule = ThemeBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    Err(input.new_custom_error(StyleSheetParseError::unsupported_nested_at_rule()))
  }
}

impl<'i> RuleBodyItemParser<'i, ThemeBodyItem, StyleSheetParseError> for ThemeBlockParser<'_> {
  fn parse_qualified(&self) -> bool {
    false
  }

  fn parse_declarations(&self) -> bool {
    true
  }
}

fn parse_theme_block<'i, 't>(
  media_queries: &[MediaQueryList],
  layer: Option<&LayerPath>,
  lossy: bool,
  input: &mut Parser<'i, 't>,
) -> Result<StyleSheetFragment, ParseError<'i, StyleSheetParseError>> {
  let mut root_input = ParserInput::new(":root");
  let selectors = SelectorList::parse(
    &TakumiSelectorParser,
    &mut Parser::new(&mut root_input),
    ParseRelative::No,
  )
  .map_err(|_| input.new_custom_error(StyleSheetParseError::unsupported_nested_at_rule()))?;

  let mut normal_declarations = StyleDeclarationBlock::default();
  let mut important_declarations = StyleDeclarationBlock::default();
  let mut fragment = StyleSheetFragment::default();
  let mut parser = ThemeBlockParser {
    media_queries,
    lossy,
  };

  for result in RuleBodyParser::new(input, &mut parser) {
    match result {
      Ok(ThemeBodyItem::Declarations(block)) => {
        let block = *block;

        if block.importance.is_empty() {
          normal_declarations.append(block);
        } else {
          important_declarations.append(block);
        }
      }
      Ok(ThemeBodyItem::Keyframes(keyframes)) => fragment.extend(keyframes),
      Err((error, _)) => {
        if lossy {
          continue;
        }
        return Err(error);
      }
    }
  }

  if !normal_declarations.declarations.is_empty() || !important_declarations.declarations.is_empty()
  {
    fragment.rules.push(CssRule {
      selectors,
      normal_declarations,
      important_declarations,
      media_queries: media_queries.to_vec(),
      layer: layer.cloned(),
      layer_order: None,
    });
  }

  Ok(fragment)
}

fn parse_keyframes_block<'i, 't>(
  name: String,
  media_queries: &[MediaQueryList],
  lossy: bool,
  input: &mut Parser<'i, 't>,
) -> Result<StyleSheetFragment, ParseError<'i, StyleSheetParseError>> {
  let mut parser = KeyframeRuleParser;
  let mut keyframes = Vec::new();
  for keyframe in StyleSheetParser::new(input, &mut parser) {
    match keyframe {
      Ok(keyframe) => keyframes.push(keyframe),
      Err((error, _)) => {
        if lossy {
          continue;
        }
        return Err(error);
      }
    }
  }

  let mut rule = KeyframesRule::builder()
    .name(name)
    .keyframes(keyframes)
    .build();
  rule.media_queries = media_queries.to_vec();

  Ok(StyleSheetFragment {
    keyframes: vec![rule],
    ..StyleSheetFragment::default()
  })
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
    return MediaQueryList::parse(input).map(AtRulePrelude::Media);
  }

  if name.eq_ignore_ascii_case("supports") {
    return parse_supports_condition(input).map(AtRulePrelude::Supports);
  }

  if name.eq_ignore_ascii_case("import") {
    let location = input.current_source_location();
    let target = match input.next()? {
      Token::QuotedString(value) => value.clone(),
      token => return Err(location.new_unexpected_token_error(token.clone())),
    };

    if target.eq_ignore_ascii_case("tailwindcss") && input.is_exhausted() {
      return Ok(AtRulePrelude::TailwindImport);
    }

    return Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)));
  }

  if name.eq_ignore_ascii_case("theme") {
    while !input.is_exhausted() {
      let location = input.current_source_location();
      let modifier = input.expect_ident()?.clone();
      match_ignore_ascii_case! {&modifier,
        "reference" | "default" | "static" | "inline" => {},
        _ => return Err(location.new_unexpected_token_error(Token::Ident(modifier))),
      }
    }
    return Ok(AtRulePrelude::Theme);
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

fn extend_layer_name(current_layer: Option<&LayerPath>, layer_name: &[LayerName]) -> LayerPath {
  if layer_name == [LayerName::Anonymous] {
    let mut nested_layer = current_layer.cloned().unwrap_or_default();
    nested_layer.push(LayerName::Anonymous);
    return nested_layer;
  }

  let mut combined = current_layer.cloned().unwrap_or_default();
  combined.extend(layer_name.iter().cloned());
  combined
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
  selectors: SelectorList<SelectorImpl>,
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
  parent_selectors: &SelectorList<SelectorImpl>,
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
        Some(&nested_layer),
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
    AtRulePrelude::Theme => parse_theme_block(media_queries, current_layer, lossy, input),
    AtRulePrelude::Keyframes(_) | AtRulePrelude::Property(_) | AtRulePrelude::TailwindImport => {
      Err(input.new_custom_error(StyleSheetParseError::unsupported_nested_at_rule()))
    }
  }
}

impl<'i> QualifiedRuleParser<'i> for RuleParser {
  type Prelude = SelectorList<SelectorImpl>;
  type QualifiedRule = StyleSheetFragment;
  type Error = StyleSheetParseError;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    SelectorList::parse(&TakumiSelectorParser, input, ParseRelative::No)
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    parse_style_rule_block(
      selectors,
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
          .map(|layer_name| extend_layer_name(self.current_layer.as_ref(), layer_name))
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
            current_layer: Some(nested_layer),
            lossy: self.lossy,
          },
        )?;
        fragment.declared_layers.splice(0..0, declared_layers);
        Ok(fragment)
      }
      AtRulePrelude::Keyframes(name) => parse_keyframes_block(name, &[], self.lossy, input),
      AtRulePrelude::Theme => {
        parse_theme_block(&[], self.current_layer.as_ref(), self.lossy, input)
      }
      // `@import "tailwindcss"` ends at its semicolon; a block after it is invalid.
      AtRulePrelude::TailwindImport => {
        Err(input.new_custom_error(StyleSheetParseError::unsupported_nested_at_rule()))
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
          .map(|layer_name| extend_layer_name(self.current_layer.as_ref(), &layer_name))
          .collect(),
        ..StyleSheetFragment::default()
      }),
      AtRulePrelude::TailwindImport => Ok(StyleSheetFragment {
        preflight: true,
        ..StyleSheetFragment::default()
      }),
      _ => Err(()),
    }
  }
}

/// Defines a stylesheet with rules, keyframes, and property rules.
#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
  /// Style rules in source order.
  pub(crate) rules: Vec<CssRule>,
  /// `@keyframes` rules.
  pub(crate) keyframes: Vec<KeyframesRule>,
  /// `@property` rules.
  pub(crate) property_rules: Vec<PropertyRule>,
  /// Number of distinct cascade layers.
  pub(crate) layer_count: usize,
  /// Widths from unconditional `:root` `--breakpoint-*` declarations, which
  /// gate `tw` variants before the cascade runs and so resolve at parse time.
  pub(crate) breakpoints: BreakpointOverrides,
  /// Whether `@import "tailwindcss"` turned Preflight on, dropping the UA
  /// preset cosmetics.
  pub(crate) preflight: bool,
}

impl From<Vec<KeyframesRule>> for StyleSheet {
  fn from(keyframes: Vec<KeyframesRule>) -> Self {
    Self {
      keyframes,
      layer_count: 0,
      ..Default::default()
    }
  }
}

impl From<Vec<PropertyRule>> for StyleSheet {
  fn from(property_rules: Vec<PropertyRule>) -> Self {
    Self {
      property_rules,
      ..Default::default()
    }
  }
}

impl StyleSheet {
  /// The `@property` registrations declared by this stylesheet.
  pub(crate) fn property_rules(&self) -> &[PropertyRule] {
    &self.property_rules
  }

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
      return Self::default();
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
    let mut preflight = false;

    for fragment in StyleSheetParser::new(&mut parser, &mut rule_parser) {
      match fragment {
        Ok(fragment) => {
          rules.extend(fragment.rules);
          keyframes.extend(fragment.keyframes);
          property_rules.extend(fragment.property_rules);
          declared_layers.extend(fragment.declared_layers);
          preflight |= fragment.preflight;
        }
        Err((error, context)) => {
          if lossy {
            continue;
          }
          return Err(StyleSheetParseError::from_parse_error(context, error));
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
      breakpoints: collect_breakpoints(&rules),
      rules,
      keyframes,
      property_rules,
      layer_count: layer_order.len(),
      preflight,
    })
  }
}

/// `--breakpoint-*` widths from unconditional `:root` rules. Variants gate at
/// parse time, so only statically-known declarations can re-size them.
fn collect_breakpoints(rules: &[CssRule]) -> BreakpointOverrides {
  let mut breakpoints = BreakpointOverrides::default();

  for rule in rules {
    if !rule.media_queries.is_empty() || selector_list_text(&rule.selectors) != ":root" {
      continue;
    }

    for declaration in rule.normal_declarations.declarations.iter() {
      if let StyleDeclaration::CustomProperty(name, value) = declaration
        && let Some(token) = name.strip_prefix("--breakpoint-")
        && let Ok(width) = Length::from_css_str(value)
        // Only units `Breakpoint::matches` resolves; anything else would
        // shadow the built-in width with a dead one.
        && matches!(width, Length::Rem(_) | Length::Px(_) | Length::Vw(_))
      {
        breakpoints.insert(token.to_owned(), width);
      }
    }
  }

  breakpoints
}

fn selector_list_text(selectors: &SelectorList<SelectorImpl>) -> String {
  selectors
    .slice()
    .iter()
    .map(cssparser::ToCss::to_css_string)
    .collect::<Vec<_>>()
    .join(", ")
}

#[cfg(test)]
mod tests {
  use cssparser::ToCss;

  use super::*;
  use crate::{
    style::{Color, ColorInput, ComputedStyle, Length, Style, StyleDeclaration},
    viewport::Viewport,
  };

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
    result.unwrap_or_default()
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
  fn test_ignored_pseudo_selector_rule_is_kept_but_never_matches() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .a:hover { width: 10px; }
        .a, .a:hover { height: 20px; }
        .a::before { color: red; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 3);
    assert_eq!(selector_text(&sheet.rules[0]), ".a:hover");
    assert_eq!(selector_text(&sheet.rules[1]), ".a, .a:hover");
    assert_eq!(selector_text(&sheet.rules[2]), ".a::before");
  }

  #[test]
  fn test_lang_pseudo_class_round_trips_its_ranges() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .a:lang(en) { color: blue; }
        .a:lang(zh-Hant, "ja") { color: red; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(selector_text(&sheet.rules[0]), ".a:lang(en)");
    assert_eq!(selector_text(&sheet.rules[1]), ".a:lang(zh-Hant, ja)");
  }

  #[test]
  fn test_is_and_where_selectors_are_accepted() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .a:where(.b) div { background: red; }
        .a:is(.b, .c) { color: blue; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(selector_text(&sheet.rules[0]), ".a:where(.b) div");
    assert_eq!(selector_text(&sheet.rules[1]), ".a:is(.b, .c)");
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
        .is_some_and(|media| media.matches(Viewport::new((800, 600))))
    );
    assert!(
      !sheet.rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new((500, 800))))
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
    assert!(media.matches(Viewport::new((400, 800))));
    assert!(media.matches(Viewport::new((1280, 800))));
    assert!(!media.matches(Viewport::new((800, 800))));
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
        .is_some_and(|media| media.matches(Viewport::new((800, 600))))
    );
    assert!(
      sheet.property_rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new((800, 600))))
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
        .is_some_and(|media| media.matches(Viewport::new((800, 600))))
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
    assert!(sheet.rules[0].media_queries[0].matches(Viewport::new((800, 600))));
    assert!(sheet.rules[0].media_queries[1].matches(Viewport::new((800, 600))));
    assert!(!sheet.rules[0].media_queries[1].matches(Viewport::new((500, 800))));
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

  #[test]
  fn test_theme_block_reads_as_root_rule() {
    let sheet = parse_stylesheet("@theme { --color-brand: #ff0000; }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ":root");
    assert_eq!(
      sheet.rules[0].normal_declarations.declarations.as_slice(),
      &[StyleDeclaration::CustomProperty(
        "--color-brand".into(),
        "#ff0000".into()
      )]
    );
  }

  /// Tailwind inlines a reference token's value into utilities instead of
  /// emitting `:root`. Here utilities resolve `var()` through the cascade, so
  /// the token still has to land on `:root` for `bg-brand` to see it.
  #[test]
  fn test_theme_reference_still_emits_root() {
    let sheet = parse_stylesheet("@theme reference { --color-brand: #ff0000; }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ":root");
  }

  /// A custom property swallows `!important` into its value, so only a
  /// non-custom declaration exercises the split.
  #[test]
  fn test_theme_splits_important_declarations() {
    let sheet = parse_stylesheet("@theme { --x: 1px; width: 10px !important; }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].normal_declarations.declarations.len(), 1);
    assert_eq!(sheet.rules[0].important_declarations.declarations.len(), 1);
  }

  #[test]
  fn test_theme_compiler_modifiers_are_accepted() {
    let sheet = parse_stylesheet("@theme default static inline { --x: 1px; }");
    assert_eq!(sheet.rules.len(), 1);
  }

  #[test]
  fn test_theme_prefix_modifier_is_rejected() {
    assert_lossy_parse_keeps_single_valid_rule(
      r#"
        @theme prefix(tw) { --x: 1px; }

        .card { width: 100px; }
      "#,
    );
  }

  #[test]
  fn test_keyframes_inside_theme() {
    let sheet = parse_stylesheet(
      r#"
        @theme {
          --animate-wobble: wobble 1s linear infinite;
          @keyframes wobble {
            to { width: 10px; }
          }
        }
      "#,
    );
    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].name, "wobble");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ":root");
  }

  #[test]
  fn test_breakpoint_variables_are_collected() {
    let sheet = parse_stylesheet(
      r#"
        @theme { --breakpoint-3xl: 120rem; }
        :root { --breakpoint-md: 30rem; }
        @media (min-width: 5000px) { :root { --breakpoint-lg: 10rem; } }
        .card { --breakpoint-xl: 10rem; }
      "#,
    );

    assert_eq!(sheet.breakpoints.get("3xl"), Some(&Length::Rem(120.0)));
    assert_eq!(sheet.breakpoints.get("md"), Some(&Length::Rem(30.0)));
    assert_eq!(sheet.breakpoints.get("lg"), None);
    assert_eq!(sheet.breakpoints.get("xl"), None);
  }

  /// A width `Breakpoint::matches` cannot resolve stays out of the overrides,
  /// so the variant keeps its built-in width instead of never matching.
  #[test]
  fn test_breakpoint_with_unresolvable_unit_is_ignored() {
    let sheet = parse_stylesheet(":root { --breakpoint-md: 48em; }");

    assert_eq!(sheet.breakpoints.get("md"), None);
  }

  #[test]
  fn test_tailwind_import_turns_preflight_on() {
    let sheet = parse_stylesheet(r#"@import "tailwindcss"; .card { width: 100px; }"#);
    assert!(sheet.preflight);
    assert_eq!(sheet.rules.len(), 1);
  }

  #[test]
  fn test_other_imports_are_dropped() {
    let sheet = parse_stylesheet_loosy(r#"@import "./app.css"; .card { width: 100px; }"#);
    assert!(!sheet.preflight);
    assert_eq!(sheet.rules.len(), 1);
  }

  #[test]
  fn test_theme_inside_media_is_gated() {
    let sheet = parse_stylesheet("@media (min-width: 5000px) { @theme { --x: red; } }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].media_queries.len(), 1);
  }
}
