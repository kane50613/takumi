use cssparser::Parser;

use crate::layout::style::{CssSyntaxKind, CssToken, FromCss, GridPlacement, ParseResult};

/// Represents the `grid-area` shorthand.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GridArea {
  /// The grid row start line.
  pub row_start: GridPlacement,
  /// The grid column start line.
  pub column_start: GridPlacement,
  /// The grid row end line.
  pub row_end: GridPlacement,
  /// The grid column end line.
  pub column_end: GridPlacement,
}

impl<'i> FromCss<'i> for GridArea {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Keyword("span"),
    CssToken::Syntax(CssSyntaxKind::Ident),
    CssToken::Syntax(CssSyntaxKind::Number),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = vec![GridPlacement::from_css(input)?];

    while values.len() < 4 && input.try_parse(|parser| parser.expect_delim('/')).is_ok() {
      values.push(GridPlacement::from_css(input)?);
    }

    if !input.is_exhausted() {
      return Err(Self::unexpected_token_error(
        input.current_source_location(),
        input.next()?,
      ));
    }

    let row_start = values[0].clone();
    let column_start = match values.get(1) {
      Some(value) => value.clone(),
      None if matches!(row_start, GridPlacement::Named(_)) => row_start.clone(),
      None => GridPlacement::default(),
    };
    let row_end = match values.get(2) {
      Some(value) => value.clone(),
      None if matches!(row_start, GridPlacement::Named(_)) => row_start.clone(),
      None => GridPlacement::default(),
    };
    let column_end = match values.get(3) {
      Some(value) => value.clone(),
      None if matches!(column_start, GridPlacement::Named(_)) => column_start.clone(),
      None => GridPlacement::default(),
    };

    Ok(Self {
      row_start,
      column_start,
      row_end,
      column_end,
    })
  }
}
