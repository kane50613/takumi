use std::collections::HashMap;

use cssparser::{Parser, Token};

use crate::layout::style::{CssToken, FromCss, ParseResult};

/// Represents `grid-template-areas` value
///
/// Supports either a 2D matrix of area names (use "." for empty) or a CSS string value
/// like: "a a ." "b b c"
#[derive(Default, Debug, Clone, PartialEq)]
pub struct GridTemplateAreas(pub Vec<Vec<String>>);

impl From<GridTemplateAreas> for Vec<taffy::GridTemplateArea<String>> {
  fn from(value: GridTemplateAreas) -> Self {
    if value.0.is_empty() {
      return Vec::new();
    }

    let mut bounds: HashMap<&str, (usize, usize, usize, usize)> = HashMap::new();
    for (r, row) in value.0.iter().enumerate() {
      for (c, cell) in row.iter().enumerate() {
        if cell == "." {
          continue;
        }

        let entry = bounds.entry(cell.as_str()).or_insert((r, r, c, c));
        entry.0 = entry.0.min(r);
        entry.1 = entry.1.max(r);
        entry.2 = entry.2.min(c);
        entry.3 = entry.3.max(c);
      }
    }

    let mut areas: Vec<taffy::GridTemplateArea<String>> = Vec::with_capacity(bounds.len());
    for (name, (rmin, rmax, cmin, cmax)) in bounds.into_iter() {
      areas.push(taffy::GridTemplateArea {
        name: name.to_string(),
        row_start: (rmin as u16) + 1,
        row_end: (rmax as u16) + 2,
        column_start: (cmin as u16) + 1,
        column_end: (cmax as u16) + 2,
      });
    }
    areas
  }
}

impl<'i> FromCss<'i> for GridTemplateAreas {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    if let Ok(ident) = input.try_parse(Parser::expect_ident_cloned) {
      let ident_str = ident.as_ref();
      if ident_str == "none" {
        return Ok(GridTemplateAreas(Vec::new()));
      }
      return Err(Self::unexpected_token_error(location, &Token::Ident(ident)));
    }

    let mut rows: Vec<Vec<String>> = Vec::new();

    while !input.is_exhausted() {
      let token = input.next()?;
      match token {
        Token::QuotedString(row) => {
          let row_str: &str = row.as_ref();
          let cols: Vec<String> = row_str
            .split_whitespace()
            .map(ToString::to_string)
            .collect();
          if cols.is_empty() {
            return Err(Self::unexpected_token_error(
              location,
              &Token::QuotedString(row.clone()),
            ));
          }
          rows.push(cols);
        }
        Token::WhiteSpace(_) => continue,
        _ => {
          return Err(Self::unexpected_token_error(location, token));
        }
      }
    }

    // Validate consistent column counts across rows
    if let Some(width) = rows.first().map(Vec::len)
      && rows.iter().any(|r| r.len() != width)
    {
      // Create a parse error for inconsistent row lengths
      return Err(Self::unexpected_token_error(
        input.current_source_location(),
        &Token::Ident("inconsistent-rows".into()),
      ));
    }

    Ok(GridTemplateAreas(rows))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("string")]
  }
}
