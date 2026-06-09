use cssparser::*;

use crate::{error::StyleSheetParseError, style::StyleDeclarationBlock};

fn parse_supports_declaration<'i, 't>(
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
      parse_supports_declaration(input)
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

pub(crate) fn parse_supports_condition<'i, 't>(
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

  if !input.is_exhausted() {
    return Err(input.new_error_for_next_token());
  }

  Ok(result)
}
