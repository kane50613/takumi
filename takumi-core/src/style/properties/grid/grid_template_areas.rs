use std::{fmt, mem::take};

use cssparser::{Parser, Token};
use rustc_hash::FxHashMap;

use super::{at_value_end, write_space_separated};
use crate::style::{FromCss, MakeComputed, ParseResult, ToCss, unexpected_token};

/// Represents a `grid-template-areas` value: each row's area names, `.` for a cell no area holds.
#[derive(Default, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GridTemplateAreas(pub Vec<Vec<String>>);

impl MakeComputed for GridTemplateAreas {}

/// The rows of a `grid-template-areas` value as they are parsed, with the rectangle each named
/// area covers so far, Blink's `NamedGridAreaMap`.
#[derive(Default)]
pub(crate) struct NamedGridAreas {
  rows: Vec<Vec<String>>,
  /// Each area's end row and its start and end columns.
  areas: FxHashMap<String, (usize, usize, usize)>,
}

impl NamedGridAreas {
  /// Blink's `ParseGridTemplateAreasRow`: appends the row `text` names, unless it has no names,
  /// has another column count than the rows before it, or makes an area that is not a rectangle.
  pub(crate) fn push_row(&mut self, text: &str) -> bool {
    let Some(names) = column_names(text) else {
      return false;
    };

    if names.is_empty()
      || self
        .rows
        .first()
        .is_some_and(|first| first.len() != names.len())
    {
      return false;
    }

    let row = self.rows.len();
    let mut column = 0;

    while column < names.len() {
      let name = &names[column];
      let end = column
        + names[column..]
          .iter()
          .take_while(|other| *other == name)
          .count();

      if name != "." {
        match self.areas.get_mut(name) {
          Some(area) if *area == (row, column, end) => area.0 = row + 1,
          Some(_) => return false,
          None => {
            self.areas.insert(name.clone(), (row + 1, column, end));
          }
        }
      }
      column = end;
    }
    self.rows.push(names);
    true
  }

  pub(crate) fn finish(self) -> GridTemplateAreas {
    GridTemplateAreas(self.rows)
  }
}

/// Blink's `ParseGridTemplateAreasColumnNames`: the names a row's string spells, each run of `.`
/// one null cell, or none when it holds a character a name cannot.
fn column_names(text: &str) -> Option<Vec<String>> {
  let mut names = Vec::new();
  let mut name = String::new();

  for character in text.chars() {
    if matches!(character, ' ' | '\t' | '\n' | '\r' | '\x0C') {
      if !name.is_empty() {
        names.push(take(&mut name));
      }
      continue;
    }
    if character == '.' {
      if name == "." {
        continue;
      }
      if !name.is_empty() {
        names.push(take(&mut name));
      }
    } else {
      if !is_name_code_point(character) {
        return None;
      }
      if name == "." {
        names.push(take(&mut name));
      }
    }
    name.push(character);
  }
  if !name.is_empty() {
    names.push(name);
  }
  Some(names)
}

/// <https://drafts.csswg.org/css-syntax-3/#name-code-point>
fn is_name_code_point(character: char) -> bool {
  character.is_ascii_alphanumeric() || matches!(character, '_' | '-') || !character.is_ascii()
}

impl<'i> FromCss<'i> for GridTemplateAreas {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if let Ok(ident) = input.try_parse(Parser::expect_ident_cloned) {
      if ident.as_ref() == "none" {
        return Ok(GridTemplateAreas(Vec::new()));
      }
      return Err(unexpected_token!(&Token::Ident(ident)));
    }

    let mut areas = NamedGridAreas::default();

    while !at_value_end(input) {
      let row = input.expect_string()?;

      if !areas.push_row(row) {
        return Err(unexpected_token!(&Token::QuotedString(row.clone())));
      }
    }

    Ok(areas.finish())
  }

  const VALID_TOKENS: &'static [&'static str] = &["<string>"];
}

impl ToCss for GridTemplateAreas {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.0.is_empty() {
      dest.write_str("none")
    } else {
      let mut first = true;
      for row in &self.0 {
        if !first {
          dest.write_str(" ")?;
        }
        first = false;
        dest.write_str("\"")?;
        write_space_separated(dest, row)?;
        dest.write_str("\"")?;
      }
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  fn rows(css: &str) -> Option<Vec<Vec<String>>> {
    GridTemplateAreas::from_css_str(css)
      .ok()
      .map(|areas| areas.0)
  }

  #[test]
  fn dots_split_names_and_runs_collapse() {
    assert_eq!(
      rows("\"a...b ..c\""),
      Some(vec![vec![
        "a".to_owned(),
        ".".to_owned(),
        "b".to_owned(),
        ".".to_owned(),
        "c".to_owned(),
      ]])
    );
  }

  #[test]
  fn areas_must_be_rectangles() {
    assert!(rows("\"a a\" \"a b\"").is_none());
    assert!(rows("\"a b a\"").is_none());
    assert!(rows("\"a b\" \"c d\" \"a b\"").is_none());
    assert!(rows("\"a a b\" \"a a c\"").is_some());
  }

  #[test]
  fn rows_need_the_same_column_count_and_a_name() {
    assert!(rows("\"a b\" \"c\"").is_none());
    assert!(rows("\"  \"").is_none());
    assert!(rows("\"a ?\"").is_none());
  }

  #[test]
  fn rows_are_strings() {
    assert!(rows("\"first\" none").is_none());
    assert!(rows("first").is_none());
  }
}
