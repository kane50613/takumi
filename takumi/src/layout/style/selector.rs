use cssparser::{
  BasicParseErrorKind, CowRcStr, DeclarationParser, ParseError, Parser, ParserInput,
  QualifiedRuleParser, RuleBodyParser, SourceLocation, StyleSheetParser, ToCss,
};
use selectors::parser::{
  NonTSPseudoClass, PseudoElement, SelectorImpl, SelectorList, SelectorParseErrorKind,
};
use std::{
  borrow::Cow,
  fmt::{self, Write},
};

use crate::layout::style::Style;

#[derive(Debug, Clone)]
pub enum CssSelectorParseError<'i> {
  Basic(BasicParseErrorKind<'i>),
  Property(Cow<'i, str>),
  Selector(SelectorParseErrorKind<'i>),
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

impl std::convert::AsRef<str> for TakumiIdent {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl ToCss for TakumiIdent {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    cssparser::serialize_identifier(&self.0, dest)
  }
}

impl precomputed_hash::PrecomputedHash for TakumiIdent {
  fn precomputed_hash(&self) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for byte in self.0.as_bytes() {
      hash ^= u32::from(*byte);
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DummyPseudoElement {
  #[default]
  Before,
}

impl ToCss for DummyPseudoElement {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match self {
      DummyPseudoElement::Before => dest.write_str("::before"),
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

pub struct TakumiSelectorParser;

impl<'i> selectors::Parser<'i> for TakumiSelectorParser {
  type Impl = TakumiSelectorImpl;
  type Error = CssSelectorParseError<'i>;

  fn parse_non_ts_pseudo_class(
    &self,
    _location: SourceLocation,
    name: cssparser::CowRcStr<'i>,
  ) -> Result<DummyPseudoClass, ParseError<'i, Self::Error>> {
    if name.eq_ignore_ascii_case("hover") {
      Ok(DummyPseudoClass::Hover)
    } else {
      Err(
        cssparser::SourceLocation::default().new_custom_error(CssSelectorParseError::Basic(
          BasicParseErrorKind::EndOfInput,
        )),
      )
    }
  }

  fn parse_pseudo_element(
    &self,
    _location: SourceLocation,
    name: cssparser::CowRcStr<'i>,
  ) -> Result<DummyPseudoElement, ParseError<'i, Self::Error>> {
    if name.eq_ignore_ascii_case("before") {
      Ok(DummyPseudoElement::Before)
    } else {
      Err(
        cssparser::SourceLocation::default().new_custom_error(CssSelectorParseError::Basic(
          BasicParseErrorKind::EndOfInput,
        )),
      )
    }
  }
}

pub struct StyleDeclarationParser<'a> {
  pub style: &'a mut Style,
}

impl<'a, 'i> DeclarationParser<'i> for StyleDeclarationParser<'a> {
  type Declaration = ();
  type Error = CssSelectorParseError<'i>;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &cssparser::ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    self
      .style
      .apply_css_property(&name, input)
      .map_err(ParseError::into)?;

    Ok(())
  }
}

impl<'a, 'i> cssparser::QualifiedRuleParser<'i> for StyleDeclarationParser<'a> {
  type Prelude = ();
  type QualifiedRule = ();
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    Err(input.new_custom_error(CssSelectorParseError::Basic(
      BasicParseErrorKind::EndOfInput,
    )))
  }

  fn parse_block<'t>(
    &mut self,
    _prelude: Self::Prelude,
    _location: &cssparser::ParserState,
    _input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    Ok(())
  }
}

impl<'a, 'i> cssparser::AtRuleParser<'i> for StyleDeclarationParser<'a> {
  type Prelude = ();
  type AtRule = ();
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    Err(input.new_custom_error(CssSelectorParseError::Basic(
      BasicParseErrorKind::AtRuleInvalid(name),
    )))
  }
}

impl<'a, 'i> cssparser::RuleBodyItemParser<'i, (), CssSelectorParseError<'i>>
  for StyleDeclarationParser<'a>
{
  fn parse_qualified(&self) -> bool {
    false
  }
  fn parse_declarations(&self) -> bool {
    true
  }
}

pub struct TakumiRuleParser;

#[derive(Debug, Clone)]
pub struct CssRule {
  pub selectors: SelectorList<TakumiSelectorImpl>,
  pub style: Style,
}

impl<'i> QualifiedRuleParser<'i> for TakumiRuleParser {
  type Prelude = SelectorList<TakumiSelectorImpl>;
  type QualifiedRule = CssRule;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    SelectorList::parse(
      &TakumiSelectorParser,
      input,
      selectors::parser::ParseRelative::No,
    )
  }

  fn parse_block<'t>(
    &mut self,
    selectors: Self::Prelude,
    _location: &cssparser::ParserState,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    let mut style = Style::default();
    let mut decl_parser = StyleDeclarationParser { style: &mut style };
    let parser = RuleBodyParser::new(input, &mut decl_parser);
    for res in parser {
      if let Err((error, _declaration)) = res {
        return Err(error);
      }
    }
    Ok(CssRule { selectors, style })
  }
}

impl<'i> cssparser::AtRuleParser<'i> for TakumiRuleParser {
  type Prelude = ();
  type AtRule = CssRule;
  type Error = CssSelectorParseError<'i>;

  fn parse_prelude<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
    Err(input.new_custom_error(CssSelectorParseError::Basic(
      BasicParseErrorKind::AtRuleInvalid(name),
    )))
  }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StyleSheet {
  pub rules: Vec<CssRule>,
}

impl From<String> for StyleSheet {
  fn from(css: String) -> Self {
    Self::parse(&css)
  }
}

impl From<&str> for StyleSheet {
  fn from(css: &str) -> Self {
    Self::parse(css)
  }
}

impl StyleSheet {
  pub(crate) fn parse(css: &str) -> Self {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rule_parser = TakumiRuleParser;
    let mut rules = Vec::new();

    let rule_list_parser = StyleSheetParser::new(&mut parser, &mut rule_parser);

    for rule in rule_list_parser.flatten() {
      rules.push(rule);
    }

    Self { rules }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{CssValue, Length};

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
    assert_eq!(rule.style.width, CssValue::Value(Length::Px(100.0)));
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
      sheet.rules[0].style.width,
      CssValue::Value(Length::Px(10.0))
    );
    assert_eq!(
      sheet.rules[1].style.height,
      CssValue::Value(Length::Px(20.0))
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
      sheet.rules[0].style.width,
      CssValue::Value(Length::Px(12.0))
    );
  }

  #[test]
  fn test_parse_stylesheet_malformed_css_skips_invalid_rule() {
    let sheet = StyleSheet::parse(
      r#"
        .a { width: 10px; }
        . { color: red; }
        .b { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(
      sheet.rules[0].style.width,
      CssValue::Value(Length::Px(10.0))
    );
    assert_eq!(
      sheet.rules[1].style.height,
      CssValue::Value(Length::Px(20.0))
    );
  }
}
