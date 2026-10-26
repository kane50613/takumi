use std::{fmt, mem::take};

use cssparser::Parser;

use super::write_space_separated;
use crate::style::{
  FromCss, GridRepeatTrack, GridRepetitionCount, GridTrackSize, MakeComputed, ParseResult,
  SizingContext, ToCss,
};

/// An ordered list of [`GridTemplateComponent`] values.
pub type GridTemplateComponents = Vec<GridTemplateComponent>;

/// Parses a `[name1 name2 ...]` line-name block's body; caller consumes the opening bracket.
pub(crate) fn parse_line_names<'i>(input: &mut Parser<'i>) -> ParseResult<'i, Vec<String>> {
  input.parse_nested_block(|i| {
    let mut names = Vec::new();
    while let Ok(name) = i.try_parse(Parser::expect_ident_cloned) {
      names.push(name.as_ref().to_owned());
    }
    Ok(names)
  })
}

/// Represents a track sizing function or a list of line names between tracks
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum GridTemplateComponent {
  /// A list of line names that apply to the current grid line (e.g., [a b])
  LineNames(Vec<String>),
  /// A single non-repeated track
  Single(GridTrackSize),
  /// Automatically generate grid tracks to fit the available space using the specified definite track lengths
  /// Only valid if every track in template (not just the repetition) has a fixed size.
  Repeat(GridRepetitionCount, Vec<GridRepeatTrack>),
}

impl MakeComputed for GridTemplateComponent {
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      GridTemplateComponent::Single(size) => size.make_computed(sizing),
      GridTemplateComponent::Repeat(_, tracks) => {
        for track in tracks.iter_mut() {
          track.make_computed(sizing);
        }
      }
      _ => {}
    }
  }
}

impl<'i> FromCss<'i> for GridTemplateComponent {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input.try_parse(Parser::expect_square_bracket_block).is_ok() {
      return Ok(GridTemplateComponent::LineNames(parse_line_names(input)?));
    }

    if input
      .try_parse(|i| i.expect_function_matching("repeat"))
      .is_ok()
    {
      return input.parse_nested_block(|input| {
        let repetition = GridRepetitionCount::from_css(input)?;
        input.expect_comma()?;

        let mut tracks: Vec<GridRepeatTrack> = Vec::new();
        // Names encountered after a size belong to the NEXT track in repeat() context
        let mut pending_leading_names: Vec<String> = Vec::new();
        loop {
          let mut names: Vec<String> = take(&mut pending_leading_names);

          while input.try_parse(Parser::expect_square_bracket_block).is_ok() {
            names.extend(parse_line_names(input)?);
          }

          let Ok(size) = input.try_parse(GridTrackSize::from_css) else {
            pending_leading_names = names;
            break;
          };

          // Collect trailing names, but assign them to the next track
          while input.try_parse(Parser::expect_square_bracket_block).is_ok() {
            pending_leading_names.extend(parse_line_names(input)?);
          }

          tracks.push(GridRepeatTrack {
            size,
            names,
            end_names: None,
          });
        }

        if tracks.is_empty() {
          return Err(input.new_error_for_next_token());
        }

        // Any remaining pending names after the final size are the trailing names of the repeat fragment
        if !pending_leading_names.is_empty()
          && let Some(last) = tracks.last_mut()
        {
          last.end_names = Some(take(&mut pending_leading_names));
        }

        Ok(GridTemplateComponent::Repeat(repetition, tracks))
      });
    }

    let size = GridTrackSize::from_css(input)?;
    Ok(GridTemplateComponent::Single(size))
  }

  const VALID_TOKENS: &'static [&'static str] =
    &["<line-names>", "<repeat()>", "<minmax()>", "<length>"];
}

impl<'i> FromCss<'i> for GridTemplateComponents {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let mut components = Vec::new();
    while let Ok(component) = input.try_parse(GridTemplateComponent::from_css) {
      components.push(component);
    }
    Ok(components)
  }

  const VALID_TOKENS: &'static [&'static str] = GridTemplateComponent::VALID_TOKENS;
}

impl ToCss for GridTemplateComponent {
  const LIST_SEPARATOR: &'static str = " ";
  const EMPTY_LIST_KEYWORD: Option<&'static str> = Some("none");

  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::LineNames(names) => {
        dest.write_str("[")?;
        write_space_separated(dest, names)?;
        dest.write_str("]")
      }
      Self::Single(size) => size.to_css(dest),
      Self::Repeat(count, tracks) => {
        dest.write_str("repeat(")?;
        count.to_css(dest)?;
        dest.write_str(", ")?;
        write_space_separated(dest, tracks)?;
        dest.write_str(")")
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{FromCssStr, GridLength, GridRepetitionKeyword};

  #[test]
  fn components_serialize_space_separated() {
    let components = GridTemplateComponents::from_css_str("50px 100px").unwrap();
    assert_eq!(components.to_css_string(), "50px 100px");
  }

  #[test]
  fn empty_components_serialize_as_none() {
    assert_eq!(GridTemplateComponents::default().to_css_string(), "none");
  }

  #[test]
  fn test_parse_template_component_repeat() {
    assert_eq!(
      GridTemplateComponent::from_css_str("repeat(auto-fill, [a] 1fr [b] 2fr)"),
      Ok(GridTemplateComponent::Repeat(
        GridRepetitionCount::Keyword(GridRepetitionKeyword::AutoFill),
        vec![
          GridRepeatTrack {
            names: vec!["a".to_string()],
            size: GridTrackSize::Fixed(GridLength::Fr(1.0)),
            end_names: None
          },
          GridRepeatTrack {
            names: vec!["b".to_string()],
            size: GridTrackSize::Fixed(GridLength::Fr(2.0)),
            end_names: None
          }
        ]
      ))
    );
  }

  #[test]
  fn test_parse_template_component_repeat_end_names() {
    assert_eq!(
      GridTemplateComponent::from_css_str("repeat(2, 1fr [a b])"),
      Ok(GridTemplateComponent::Repeat(
        GridRepetitionCount::Count(2),
        vec![GridRepeatTrack {
          names: Vec::new(),
          size: GridTrackSize::Fixed(GridLength::Fr(1.0)),
          end_names: Some(vec!["a".to_string(), "b".to_string()])
        }]
      ))
    );
  }

  #[test]
  fn test_parse_repeat_without_track_size_is_error() {
    assert!(GridTemplateComponent::from_css_str("repeat(2, [a])").is_err());
  }
}
