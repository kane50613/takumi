use cssparser::Parser;

use super::parse_line_names;
use crate::style::{
  CssSyntaxKind, CssToken, FromCss, GridTrackSize, MakeComputed, ParseResult, SizingContext, ToCss,
};

/// Represents a grid repeat track
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GridRepeatTrack {
  /// The size of the grid track
  pub size: GridTrackSize,
  /// The names for the line preceding this track within the repeat() clause
  pub names: Vec<String>,
  /// The names for the final line after the last track within the repeat() clause
  /// Only set on the last track of the repeat fragment. For other tracks this is None.
  pub end_names: Option<Vec<String>>,
}

impl MakeComputed for GridRepeatTrack {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.size.make_computed(sizing);
  }
}

impl<'i> FromCss<'i> for GridRepeatTrack {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Collect any leading line name blocks: [name1 name2]
    let mut names: Vec<String> = Vec::new();

    while input.try_parse(Parser::expect_square_bracket_block).is_ok() {
      names.extend(parse_line_names(input)?);
    }

    // Parse the track size
    let size = GridTrackSize::from_css(input)?;

    // Collect any trailing line name blocks
    while input.try_parse(Parser::expect_square_bracket_block).is_ok() {
      names.extend(parse_line_names(input)?);
    }

    Ok(GridRepeatTrack {
      size,
      names,
      end_names: None,
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::LineNames),
    CssToken::Syntax(CssSyntaxKind::TrackSize),
  ];
}

impl ToCss for GridRepeatTrack {
  fn to_css<W: std::fmt::Write>(&self, dest: &mut W) -> std::fmt::Result {
    if !self.names.is_empty() {
      dest.write_str("[")?;
      let mut first = true;
      for name in &self.names {
        if !first {
          dest.write_str(" ")?;
        }
        first = false;
        dest.write_str(name)?;
      }
      dest.write_str("] ")?;
    }
    self.size.to_css(dest)?;
    if let Some(end_names) = &self.end_names
      && !end_names.is_empty()
    {
      dest.write_str(" [")?;
      let mut first = true;
      for name in end_names {
        if !first {
          dest.write_str(" ")?;
        }
        first = false;
        dest.write_str(name)?;
      }
      dest.write_str("]")?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use crate::style::GridLength;

  use super::*;
  use cssparser::{Parser, ParserInput};

  #[test]
  fn test_parse_repeat_track_with_names() {
    let mut parser_input = ParserInput::new("[a b] 1fr [c]");
    let mut parser = Parser::new(&mut parser_input);
    let track = GridRepeatTrack::from_css(&mut parser);

    assert_eq!(
      track,
      Ok(GridRepeatTrack {
        size: GridTrackSize::Fixed(GridLength::Fr(1.0)),
        names: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        end_names: None,
      })
    );
  }
}
