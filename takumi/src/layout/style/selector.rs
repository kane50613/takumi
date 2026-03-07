use cssparser::*;
use precomputed_hash::PrecomputedHash;
use selectors::parser::{
  Component, NonTSPseudoClass, ParseRelative, PseudoElement, Selector, SelectorImpl, SelectorList,
  SelectorParseErrorKind,
};
use std::{
  borrow::Cow,
  fmt::{self, Write},
};

use crate::layout::style::StyleDeclarationBlock;

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
        declarations
          .importance_set
          .insert(declaration.longhand_id());
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

#[derive(Debug, Clone)]
pub struct KeyframeRule {
  pub offsets: Vec<f32>,
  pub declarations: StyleDeclarationBlock,
}

#[derive(Debug, Clone)]
pub struct KeyframesRule {
  pub name: String,
  pub keyframes: Vec<KeyframeRule>,
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
    let mut offsets = Vec::new();

    loop {
      offsets.push(parse_keyframe_offset(input)?);
      if input.try_parse(Parser::expect_comma).is_err() {
        break;
      }
    }

    Ok(offsets)
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

fn parse_keyframe_offset<'i>(
  input: &mut Parser<'i, '_>,
) -> Result<f32, ParseError<'i, CssSelectorParseError<'i>>> {
  if input
    .try_parse(|parser| parser.expect_ident_matching("from"))
    .is_ok()
  {
    return Ok(0.0);
  }

  if input
    .try_parse(|parser| parser.expect_ident_matching("to"))
    .is_ok()
  {
    return Ok(1.0);
  }

  let token = input.next()?;
  let Token::Percentage { unit_value, .. } = token else {
    return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
  };

  let offset = *unit_value;
  if !(0.0..=1.0).contains(&offset) {
    return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
  }

  Ok(offset)
}

pub struct TakumiRuleParser;

#[derive(Debug, Clone)]
pub struct CssRule {
  pub selectors: SelectorList<TakumiSelectorImpl>,
  pub normal_declarations: StyleDeclarationBlock,
  pub important_declarations: StyleDeclarationBlock,
}

#[derive(Debug, Clone)]
pub enum StyleSheetRule {
  Style(Box<CssRule>),
  Keyframes(KeyframesRule),
}

impl<'i> QualifiedRuleParser<'i> for TakumiRuleParser {
  type Prelude = SelectorList<TakumiSelectorImpl>;
  type QualifiedRule = StyleSheetRule;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    let selectors = SelectorList::parse(&TakumiSelectorParser, input, ParseRelative::No)?;
    ensure_supported_selector_list(&selectors).map_err(|err| input.new_custom_error(err))?;
    Ok(selectors)
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let mut normal_declarations = StyleDeclarationBlock::default();
    let mut important_declarations = StyleDeclarationBlock::default();
    let mut decl_parser = StyleDeclarationParser;
    let parser = RuleBodyParser::new(input, &mut decl_parser);
    for res in parser {
      match res {
        Ok(declarations) => {
          if declarations.importance_set.is_empty() {
            normal_declarations.append(declarations);
          } else {
            important_declarations.append(declarations);
          }
        }
        Err((_error, _declaration)) => continue,
      }
    }
    Ok(StyleSheetRule::Style(Box::new(CssRule {
      selectors,
      normal_declarations,
      important_declarations,
    })))
  }
}

impl<'i> AtRuleParser<'i> for TakumiRuleParser {
  type Prelude = String;
  type AtRule = StyleSheetRule;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    if !name.eq_ignore_ascii_case("keyframes") {
      return Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)));
    }

    Ok(input.expect_ident_or_string()?.to_string())
  }

  fn parse_block<'t>(
    &mut self,
    name: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
    let mut parser = KeyframeRuleParser;
    let rule_list_parser = StyleSheetParser::new(input, &mut parser);
    let keyframes = rule_list_parser.filter_map(Result::ok).collect::<Vec<_>>();

    Ok(StyleSheetRule::Keyframes(KeyframesRule { name, keyframes }))
  }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StyleSheet {
  pub rules: Vec<CssRule>,
  pub keyframes: Vec<KeyframesRule>,
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

    let rule_list_parser = StyleSheetParser::new(&mut parser, &mut rule_parser);

    for rule in rule_list_parser {
      match rule {
        Ok(StyleSheetRule::Style(rule)) => rules.push(*rule),
        Ok(StyleSheetRule::Keyframes(rule)) => keyframes.push(rule),
        Err((_error, _slice)) => continue,
      }
    }

    Self { rules, keyframes }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{
    Color, ColorInput, Length, ResolvedStyle, Style, StyleDeclaration, StyleDeclarationBlock,
  };

  fn resolved_style_from_declarations(declarations: &StyleDeclarationBlock) -> ResolvedStyle {
    let mut style = Style::default();
    for declaration in &declarations.declarations {
      declaration.merge_into(&mut style);
    }
    style.inherit(&ResolvedStyle::default())
  }

  #[test]
  fn test_parse_stylesheet() {
    let css = r#"
            .box {
                width: 100px;
                color: red;
            }
        "#;
    let sheet = StyleSheet::parse(css);
    assert_eq!(sheet.rules.len(), 1);
    let rule = &sheet.rules[0];

    assert_eq!(rule.selectors.slice().len(), 1);
    assert_eq!(
      resolved_style_from_declarations(&rule.normal_declarations).width,
      Length::Px(100.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_compound_selectors_specificity() {
    let sheet = StyleSheet::parse(
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
    let sheet = StyleSheet::parse(
      r#"
        .a { width: 10px; }
        .b { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(
      resolved_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(10.0)
    );
    assert_eq!(
      resolved_style_from_declarations(&sheet.rules[1].normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_multiple_selectors_in_rule() {
    let sheet = StyleSheet::parse(
      r#"
        .a, .b { width: 12px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].selectors.slice().len(), 2);
    assert_eq!(
      resolved_style_from_declarations(&sheet.rules[0].normal_declarations).width,
      Length::Px(12.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_important_declaration() {
    let sheet = StyleSheet::parse(
      r#"
        .a { width: 10px !important; height: 20px; }
      "#,
    );

    let rule = &sheet.rules[0];
    assert_eq!(
      resolved_style_from_declarations(&rule.important_declarations).width,
      Length::Px(10.0)
    );
    assert_eq!(
      resolved_style_from_declarations(&rule.normal_declarations).height,
      Length::Px(20.0)
    );
  }

  #[test]
  fn test_parse_stylesheet_shorthand_clears_prior_longhand() {
    let sheet = StyleSheet::parse(
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
    let sheet = StyleSheet::parse(
      r#"
        .a { -webkit-text-fill-color: rgb(255, 0, 0); }
      "#,
    );

    let style = resolved_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(
      style.webkit_text_fill_color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_stylesheet_unknown_property_does_not_drop_supported_declarations() {
    let sheet = StyleSheet::parse(
      r#"
        .a { --local-token: 1; width: 14px; unsupported-prop: 2; height: 6px; }
      "#,
    );

    let style = resolved_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(style.width, Length::Px(14.0));
    assert_eq!(style.height, Length::Px(6.0));
  }

  #[test]
  fn test_unsupported_attribute_selector_rule_is_rejected() {
    let sheet = StyleSheet::parse(
      r#"
        [data-kind="hero"] { width: 10px; }
      "#,
    );

    assert!(sheet.rules.is_empty());
  }

  #[test]
  fn test_unsupported_pseudo_selector_rule_is_rejected() {
    let sheet = StyleSheet::parse(
      r#"
        .a:hover { width: 10px; }
      "#,
    );

    assert!(sheet.rules.is_empty());
  }

  #[test]
  fn test_parse_keyframes_rule() {
    let sheet = StyleSheet::parse(
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
}
