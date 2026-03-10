use std::mem::take;

use cssparser::Parser;

use crate::layout::style::{CssToken, FromCss, MakeComputed, ParseResult};
use crate::rendering::Sizing;

use super::{GridRepeatTrack, GridRepetitionCount, GridTrackSize};

/// A transparent wrapper around a list of `GridTemplateComponent`.
///
/// This exists to provide a distinct type for template component lists while
/// preserving JSON compatibility (serialized as a plain array) and clean TS types.
pub type GridTemplateComponents = Vec<GridTemplateComponent>;

pub(crate) trait GridTemplateComponentsExt {
  fn collect_components_and_names(
    &self,
    sizing: &Sizing,
  ) -> (Vec<taffy::GridTemplateComponent<String>>, Vec<Vec<String>>);
}

/// Represents a track sizing function or a list of line names between tracks
#[derive(Debug, Clone, PartialEq)]
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
  fn make_computed(&mut self, sizing: &Sizing) {
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
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Line name block: [name1 name2 ...]
    if input.try_parse(Parser::expect_square_bracket_block).is_ok() {
      let mut names: Vec<String> = Vec::new();
      input.parse_nested_block(|i| {
        while let Ok(name) = i.try_parse(Parser::expect_ident_cloned) {
          names.push(name.as_ref().to_owned());
        }
        Ok(())
      })?;
      return Ok(GridTemplateComponent::LineNames(names));
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
          // Start with any pending names from the previous track's trailing names
          let mut names: Vec<String> = take(&mut pending_leading_names);

          // Capture any additional leading square-bracketed names before the size
          while input.try_parse(Parser::expect_square_bracket_block).is_ok() {
            input.parse_nested_block(|i| {
              while let Ok(name) = i.try_parse(Parser::expect_ident_cloned) {
                names.push(name.as_ref().to_owned());
              }
              Ok(())
            })?;
          }

          // If we cannot parse a size, stop the loop
          let size = if let Ok(size) = input.try_parse(GridTrackSize::from_css) {
            size
          } else {
            break;
          };

          // Collect trailing names, but assign them to the next track
          while input.try_parse(Parser::expect_square_bracket_block).is_ok() {
            input.parse_nested_block(|i| {
              while let Ok(name) = i.try_parse(Parser::expect_ident_cloned) {
                pending_leading_names.push(name.as_ref().to_owned());
              }
              Ok(())
            })?;
          }

          tracks.push(GridRepeatTrack {
            size,
            names,
            end_names: None,
          });
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

    // Single track-size
    let size = GridTrackSize::from_css(input)?;
    Ok(GridTemplateComponent::Single(size))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("line-names"),
      CssToken::Token("repeat()"),
      CssToken::Token("minmax()"),
      CssToken::Token("length"),
    ]
  }
}

impl<'i> FromCss<'i> for GridTemplateComponents {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut components = Vec::new();
    while let Ok(component) = GridTemplateComponent::from_css(input) {
      components.push(component);
    }
    Ok(components)
  }

  fn valid_tokens() -> &'static [CssToken] {
    GridTemplateComponent::valid_tokens()
  }
}

impl GridTemplateComponentsExt for [GridTemplateComponent] {
  fn collect_components_and_names(
    &self,
    sizing: &Sizing,
  ) -> (Vec<taffy::GridTemplateComponent<String>>, Vec<Vec<String>>) {
    let mut track_components = Vec::new();
    let mut line_name_sets = Vec::new();
    let mut pending_line_names = Vec::new();

    for component in self {
      match component {
        GridTemplateComponent::LineNames(names) => {
          if !names.is_empty() {
            pending_line_names.extend_from_slice(names);
          }
        }
        GridTemplateComponent::Single(track_size) => {
          line_name_sets.push(take(&mut pending_line_names));
          track_components.push(taffy::GridTemplateComponent::Single(
            track_size.to_min_max(sizing),
          ));
        }
        GridTemplateComponent::Repeat(repetition, tracks) => {
          line_name_sets.push(take(&mut pending_line_names));

          let track_sizes = tracks
            .iter()
            .map(|track| track.size.to_min_max(sizing))
            .collect();
          let mut inner_line_names = tracks
            .iter()
            .map(|track| track.names.to_owned())
            .collect::<Vec<_>>();
          inner_line_names.push(
            tracks
              .last()
              .and_then(|track| track.end_names.clone())
              .unwrap_or_default(),
          );

          track_components.push(taffy::GridTemplateComponent::Repeat(
            taffy::GridTemplateRepetition {
              count: (*repetition).into(),
              tracks: track_sizes,
              line_names: inner_line_names,
            },
          ));
        }
      }
    }

    line_name_sets.push(pending_line_names);

    (track_components, line_name_sets)
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::style::{GridLength, GridRepetitionKeyword};

  use super::*;

  #[test]
  fn test_parse_template_component_repeat() {
    assert_eq!(
      GridTemplateComponent::from_str("repeat(auto-fill, [a] 1fr [b] 2fr)"),
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
}
