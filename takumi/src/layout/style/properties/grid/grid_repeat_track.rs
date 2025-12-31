use cssparser::Parser;

use crate::layout::style::{CssToken, FromCss, GridTrackSize, ParseResult};

/// Represents a grid repeat track
#[derive(Debug, Clone, PartialEq)]
pub struct GridRepeatTrack {
  /// The size of the grid track
  pub size: GridTrackSize,
  /// The names for the line preceding this track within the repeat() clause
  pub names: Vec<String>,
  /// The names for the final line after the last track within the repeat() clause
  /// Only set on the last track of the repeat fragment. For other tracks this is None.
  pub end_names: Option<Vec<String>>,
}

impl<'i> FromCss<'i> for GridRepeatTrack {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Collect any leading line name blocks: [name1 name2]
    let mut names: Vec<String> = Vec::new();

    while input.try_parse(Parser::expect_square_bracket_block).is_ok() {
      input.parse_nested_block(|i| {
        while let Ok(name) = i.try_parse(Parser::expect_ident_cloned) {
          names.push(name.as_ref().to_owned());
        }
        Ok(())
      })?;
    }

    // Parse the track size
    let size = GridTrackSize::from_css(input)?;

    // Collect any trailing line name blocks
    while input.try_parse(Parser::expect_square_bracket_block).is_ok() {
      input.parse_nested_block(|i| {
        while let Ok(name) = i.try_parse(Parser::expect_ident_cloned) {
          names.push(name.as_ref().to_owned());
        }
        Ok(())
      })?;
    }

    Ok(GridRepeatTrack {
      size,
      names,
      end_names: None,
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("line-names"), CssToken::Token("track-size")]
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::style::GridLength;

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
