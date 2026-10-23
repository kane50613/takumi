use cssparser::{Parser, Token};

use super::{NamedGridAreas, at_value_end, parse_line_names};
use crate::style::{
  FromCss, GridLength, GridTemplateAreas, GridTemplateComponent, GridTemplateComponents,
  GridTrackSize, Length, ParseResult, unexpected_token,
};

/// Represents the `grid-template` shorthand.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GridTemplate {
  /// The `grid-template-rows` value, `None` for `none`.
  pub rows: Option<GridTemplateComponents>,
  /// The `grid-template-columns` value, `None` for `none`.
  pub columns: Option<GridTemplateComponents>,
  /// The `grid-template-areas` value, `None` for `none`.
  pub areas: Option<GridTemplateAreas>,
}

impl GridTemplate {
  /// Blink's `ConsumeGridTemplateRowsAndAreasAndColumns`:
  /// `[ <line-names>? <string> <track-size>? <line-names>? ]+ [ / <explicit-track-list> ]?`.
  fn from_areas<'i>(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let mut areas = NamedGridAreas::default();
    let mut rows = Vec::new();
    let mut columns = None;

    loop {
      if input.try_parse(Parser::expect_square_bracket_block).is_ok() {
        let names = parse_line_names(input)?;

        match rows.last_mut() {
          Some(GridTemplateComponent::LineNames(trailing)) => trailing.extend(names),
          _ => rows.push(GridTemplateComponent::LineNames(names)),
        }
      }

      let text = input.expect_string()?.clone();

      if !areas.push_row(&text) {
        return Err(unexpected_token!(&Token::QuotedString(text)));
      }
      rows.push(GridTemplateComponent::Single(
        input
          .try_parse(GridTrackSize::from_css)
          .unwrap_or(GridTrackSize::Fixed(GridLength::Unit(Length::Auto))),
      ));
      if input.try_parse(Parser::expect_square_bracket_block).is_ok() {
        rows.push(GridTemplateComponent::LineNames(parse_line_names(input)?));
      }
      if at_value_end(input) {
        break;
      }
      if input.try_parse(|input| input.expect_delim('/')).is_ok() {
        columns = Some(track_list(input, false)?);
        break;
      }
    }

    Ok(Self {
      rows: Some(rows),
      columns,
      areas: Some(areas.finish()),
    })
  }
}

impl<'i> FromCss<'i> for GridTemplate {
  /// Blink's `ConsumeGridTemplateShorthand`.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if let Ok(template) = input.try_parse(|input| -> ParseResult<'i, Self> {
      let rows = track_listing(input)?;

      input.expect_delim('/')?;
      Ok(Self {
        rows,
        columns: track_listing(input)?,
        areas: None,
      })
    }) {
      return Ok(template);
    }
    if let Ok(template) = input.try_parse(Self::from_areas) {
      return Ok(template);
    }

    input.expect_ident_matching("none")?;
    Ok(Self::default())
  }

  const VALID_TOKENS: &'static [&'static str] = &["none", "<track-list>", "<string>"];
}

/// Blink's `ConsumeGridTemplatesRowsOrColumns`: `none`, as `None`, or a track list.
pub(super) fn track_listing<'i>(
  input: &mut Parser<'i>,
) -> ParseResult<'i, Option<GridTemplateComponents>> {
  if input
    .try_parse(|input| input.expect_ident_matching("none"))
    .is_ok()
  {
    return Ok(None);
  }

  track_list(input, true).map(Some)
}

/// A `grid-template-rows` or `grid-template-columns` track list holding at least one track, and
/// no `repeat()` unless `allows_repeat`.
fn track_list<'i>(
  input: &mut Parser<'i>,
  allows_repeat: bool,
) -> ParseResult<'i, GridTemplateComponents> {
  let components = GridTemplateComponents::from_css(input)?;
  let has_track = components
    .iter()
    .any(|component| !matches!(component, GridTemplateComponent::LineNames(_)));
  let has_repeat = components
    .iter()
    .any(|component| matches!(component, GridTemplateComponent::Repeat(..)));

  if !has_track || (has_repeat && !allows_repeat) {
    return Err(input.new_error_for_next_token());
  }
  Ok(components)
}
