//! Declarations kept as their source text, for readers that parse the values themselves.

#[cfg(feature = "svg-sizing")]
use cssparser::RuleBodyParser;
use cssparser::{
  AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserState, QualifiedRuleParser,
  RuleBodyItemParser, Token, parse_important,
};

use crate::error::StyleSheetParseError;

/// A `name: value` declaration, its value as written without the `!important` annotation.
pub(crate) struct RawDeclaration<'i> {
  pub(crate) name: CowRcStr<'i>,
  pub(crate) value: &'i str,
  pub(crate) important: bool,
}

#[cfg(feature = "svg-sizing")]
impl<'i> RawDeclaration<'i> {
  /// The valid declarations of a declaration list such as a `style` attribute, in source order.
  pub(crate) fn parse_list(text: &'i str) -> Vec<Self> {
    let mut parser = Parser::new(text);

    RuleBodyParser::new(&mut parser, &mut RawDeclarationParser)
      .filter_map(Result::ok)
      .collect()
  }
}

pub(crate) struct RawDeclarationParser;

impl<'i> DeclarationParser<'i> for RawDeclarationParser {
  type Declaration = RawDeclaration<'i>;
  type Error = StyleSheetParseError;

  fn parse_value(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<Self::Error>> {
    let start = input.position();
    let mut end = start;
    let mut important = false;

    while !input.is_exhausted() {
      if input
        .try_parse(|input| {
          parse_important(input)?;
          input.expect_exhausted()
        })
        .is_ok()
      {
        important = true;
        break;
      }

      if matches!(
        input.next()?,
        Token::Function(_)
          | Token::ParenthesisBlock
          | Token::SquareBracketBlock
          | Token::CurlyBracketBlock
      ) {
        input.parse_nested_block(|block| {
          while block.next().is_ok() {}
          Ok::<_, ParseError<Self::Error>>(())
        })?;
      }
      end = input.position();
    }

    Ok(RawDeclaration {
      name,
      value: input.slice(start..end).trim(),
      important,
    })
  }
}

impl<'i> AtRuleParser<'i> for RawDeclarationParser {
  type Prelude = ();
  type AtRule = RawDeclaration<'i>;
  type Error = StyleSheetParseError;
}

impl<'i> QualifiedRuleParser<'i> for RawDeclarationParser {
  type Prelude = ();
  type QualifiedRule = RawDeclaration<'i>;
  type Error = StyleSheetParseError;
}

impl<'i> RuleBodyItemParser<'i, RawDeclaration<'i>, StyleSheetParseError> for RawDeclarationParser {
  fn parse_declarations(&self) -> bool {
    true
  }

  fn parse_qualified(&self) -> bool {
    false
  }
}

#[cfg(all(test, feature = "svg-sizing"))]
mod tests {
  use super::RawDeclaration;

  #[test]
  fn a_list_keeps_whole_function_values_and_importance() {
    let declarations: Vec<_> =
      RawDeclaration::parse_list("fill:rgb(255,255,0); stroke: url(#a) red !important;opacity:.5")
        .iter()
        .map(|declaration| {
          (
            declaration.name.to_string(),
            declaration.value,
            declaration.important,
          )
        })
        .collect();

    assert_eq!(
      declarations,
      [
        ("fill".to_owned(), "rgb(255,255,0)", false),
        ("stroke".to_owned(), "url(#a) red", true),
        ("opacity".to_owned(), ".5", false),
      ]
    );
  }
}
