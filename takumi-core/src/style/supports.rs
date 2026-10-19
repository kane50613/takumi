use cssparser::ParseError as CssParseError;
use cssparser::*;

use crate::{
  error::{StyleSheetParseError, StyleSheetParseErrorKind},
  style::StyleDeclarationBlock,
};

fn parse_supports_declaration<'i>(
  input: &mut Parser<'i>,
) -> Result<bool, ParseError<StyleSheetParseError>> {
  let name = input.expect_ident_cloned()?;
  input.expect_colon()?;
  let declaration = StyleDeclarationBlock::parse(&name, input).map_err(ParseError::into)?;
  Ok(!declaration.declarations.is_empty() && input.is_exhausted())
}

fn parse_supports_in_parens<'i>(
  input: &mut Parser<'i>,
) -> Result<bool, ParseError<StyleSheetParseError>> {
  match input.next()? {
    Token::ParenthesisBlock => input.parse_nested_block(|input| {
      let state = input.state();
      if let Ok(result) = parse_supports_condition(input)
        && input.is_exhausted()
      {
        return Ok(result);
      }

      input.reset(&state);
      parse_supports_declaration(input)
    }),
    _ => Err(CssParseError::unexpected_token()),
  }
}

fn parse_supports_not<'i>(
  input: &mut Parser<'i>,
) -> Result<bool, ParseError<StyleSheetParseError>> {
  if input
    .try_parse(|input| input.expect_ident_matching("not"))
    .is_ok()
  {
    return Ok(!parse_supports_not(input)?);
  }

  parse_supports_in_parens(input)
}

pub(crate) fn parse_supports_condition<'i>(
  input: &mut Parser<'i>,
) -> Result<bool, ParseError<StyleSheetParseError>> {
  let mut result = parse_supports_not(input)?;
  let mut operator = None;

  while let Ok(is_and) = input.try_parse(parse_supports_operator) {
    if operator.is_some_and(|operator| operator != is_and) {
      return Err(CssParseError::custom(
        StyleSheetParseErrorKind::SupportsMixedAndOrWithoutParentheses,
      ));
    }

    operator = Some(is_and);

    let operand = parse_supports_not(input)?;

    if is_and {
      result &= operand;
    } else {
      result |= operand;
    }
  }

  if !input.is_exhausted() {
    return Err(input.new_error_for_next_token());
  }

  Ok(result)
}

/// Reads `and` as `true` and `or` as `false`.
fn parse_supports_operator<'i>(
  input: &mut Parser<'i>,
) -> Result<bool, ParseError<StyleSheetParseError>> {
  let ident = input.expect_ident()?;

  match_ignore_ascii_case! { ident.as_ref(),
    "and" => Ok(true),
    "or" => Ok(false),
    _ => Err(CssParseError::unexpected_token()),
  }
}
