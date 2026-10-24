//! Declarations kept as their source text, for readers that parse the values themselves.

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
