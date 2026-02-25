use cssparser::{
  BasicParseErrorKind, CowRcStr, DeclarationParser, ParseError, Parser, ParserInput,
  QualifiedRuleParser, RuleBodyParser, SourceLocation, StyleSheetParser, ToCss, parse_important,
};
use selectors::parser::{
  NonTSPseudoClass, PseudoElement, SelectorImpl, SelectorList, SelectorParseErrorKind,
};
use std::{
  borrow::Cow,
  fmt::{self, Write},
};

use crate::layout::style::{DeclarationMetadata, StyleDeclaration, StyleDeclarations};

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
    location: SourceLocation,
    name: cssparser::CowRcStr<'i>,
  ) -> Result<DummyPseudoClass, ParseError<'i, Self::Error>> {
    if name.eq_ignore_ascii_case("hover") {
      Ok(DummyPseudoClass::Hover)
    } else {
      Err(location.new_custom_error(CssSelectorParseError::Basic(
        BasicParseErrorKind::EndOfInput,
      )))
    }
  }

  fn parse_pseudo_element(
    &self,
    location: SourceLocation,
    name: cssparser::CowRcStr<'i>,
  ) -> Result<DummyPseudoElement, ParseError<'i, Self::Error>> {
    if name.eq_ignore_ascii_case("before") {
      Ok(DummyPseudoElement::Before)
    } else {
      Err(location.new_custom_error(CssSelectorParseError::Basic(
        BasicParseErrorKind::EndOfInput,
      )))
    }
  }
}

pub struct StyleDeclarationParser;

impl<'i> DeclarationParser<'i> for StyleDeclarationParser {
  type Declaration = StyleDeclaration;
  type Error = CssSelectorParseError<'i>;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &cssparser::ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let declaration = StyleDeclaration::parse(&name, input).map_err(ParseError::into)?;
    let important = input.try_parse(parse_important).is_ok();
    let metadata = DeclarationMetadata { important };

    Ok(declaration.with_metadata(metadata))
  }
}

impl<'i> cssparser::QualifiedRuleParser<'i> for StyleDeclarationParser {
  type Prelude = ();
  type QualifiedRule = StyleDeclaration;
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
    input: &mut Parser<'i, 't>,
  ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
    Err(input.new_custom_error(CssSelectorParseError::Basic(
      BasicParseErrorKind::EndOfInput,
    )))
  }
}

impl<'i> cssparser::AtRuleParser<'i> for StyleDeclarationParser {
  type Prelude = ();
  type AtRule = StyleDeclaration;
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

impl<'i> cssparser::RuleBodyItemParser<'i, StyleDeclaration, CssSelectorParseError<'i>>
  for StyleDeclarationParser
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
  pub normal_declarations: StyleDeclarations,
  pub important_declarations: StyleDeclarations,
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
    let mut normal_declarations = StyleDeclarations::new();
    let mut important_declarations = StyleDeclarations::new();
    let mut decl_parser = StyleDeclarationParser;
    let parser = RuleBodyParser::new(input, &mut decl_parser);
    for res in parser {
      match res {
        Ok(declaration) => {
          if declaration.metadata.important {
            important_declarations.push(declaration);
          } else {
            normal_declarations.push(declaration);
          }
        }
        Err((_error, _declaration)) => continue,
      }
    }
    Ok(CssRule {
      selectors,
      normal_declarations,
      important_declarations,
    })
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

    let rule_list_parser = StyleSheetParser::new(&mut parser, &mut rule_parser);

    for rule in rule_list_parser {
      match rule {
        Ok(rule) => rules.push(rule),
        Err((_error, _slice)) => continue,
      }
    }

    Self { rules }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{CssValue, Length, Style, apply_style_declarations};

  fn style_from_declarations(declarations: &[StyleDeclaration]) -> Style {
    let mut style = Style::default();
    apply_style_declarations(declarations, &mut style);
    style
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
      style_from_declarations(&rule.normal_declarations).width,
      CssValue::Value(Length::Px(100.0))
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
      style_from_declarations(&sheet.rules[0].normal_declarations).width,
      CssValue::Value(Length::Px(10.0))
    );
    assert_eq!(
      style_from_declarations(&sheet.rules[1].normal_declarations).height,
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
      style_from_declarations(&sheet.rules[0].normal_declarations).width,
      CssValue::Value(Length::Px(12.0))
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
      style_from_declarations(&rule.important_declarations).width,
      CssValue::Value(Length::Px(10.0))
    );
    assert_eq!(
      style_from_declarations(&rule.normal_declarations).height,
      CssValue::Value(Length::Px(20.0))
    );
  }

  #[test]
  fn test_parse_stylesheet_shorthand_clears_prior_longhand() {
    let sheet = StyleSheet::parse(
      r#"
        .a { padding-left: 4px; padding: 10px; }
      "#,
    );

    let style = style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(style.padding_left, CssValue::Unset);
  }
}
