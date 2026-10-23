use cssparser::{Parser, Token};

use super::{GridTemplate, at_value_end, track_listing};
use crate::style::{FromCss, GridAutoFlow, GridDirection, GridTrackSizes, ParseResult};

/// Represents the `grid` shorthand.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Grid {
  /// The explicit grid, the `grid-template` longhands.
  pub template: GridTemplate,
  /// The `grid-auto-flow` value.
  pub auto_flow: GridAutoFlow,
  /// The `grid-auto-rows` value, `None` for its initial `auto`.
  pub auto_rows: Option<GridTrackSizes>,
  /// The `grid-auto-columns` value, `None` for its initial `auto`.
  pub auto_columns: Option<GridTrackSizes>,
}

impl<'i> FromCss<'i> for Grid {
  /// Blink's `Grid::ParseShorthand`: a `grid-template` value, or one axis's explicit tracks and the
  /// other's implicit ones.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if let Ok(template) = input.try_parse(|input| {
      let template = GridTemplate::from_css(input)?;

      if !at_value_end(input) {
        return Err(input.new_error_for_next_token());
      }
      Ok(template)
    }) {
      return Ok(Self {
        template,
        ..Self::default()
      });
    }

    if starts_auto_flow(input) {
      let dense = implicit_auto_flow(input)?;
      let auto_rows = if input.try_parse(|input| input.expect_delim('/')).is_ok() {
        None
      } else {
        let auto_rows = auto_track_list(input)?;

        input.expect_delim('/')?;
        Some(auto_rows)
      };

      return Ok(Self {
        template: GridTemplate {
          columns: track_listing(input)?,
          ..GridTemplate::default()
        },
        auto_flow: GridAutoFlow {
          direction: GridDirection::Row,
          dense,
        },
        auto_rows,
        auto_columns: None,
      });
    }

    let rows = track_listing(input)?;

    input.expect_delim('/')?;

    let dense = implicit_auto_flow(input)?;

    Ok(Self {
      template: GridTemplate {
        rows,
        ..GridTemplate::default()
      },
      auto_flow: GridAutoFlow {
        direction: GridDirection::Column,
        dense,
      },
      auto_rows: None,
      auto_columns: input.try_parse(auto_track_list).ok(),
    })
  }

  const VALID_TOKENS: &'static [&'static str] =
    &["none", "<track-list>", "<string>", "auto-flow", "dense"];
}

/// Whether `input` starts with `auto-flow` or `dense`, which commits it to the implicit-grid forms.
fn starts_auto_flow(input: &mut Parser<'_>) -> bool {
  let state = input.state();
  let starts = matches!(
    input.next(),
    Ok(Token::Ident(ident))
      if ident.eq_ignore_ascii_case("auto-flow") || ident.eq_ignore_ascii_case("dense")
  );

  input.reset(&state);
  starts
}

/// Blink's `ConsumeImplicitAutoFlow`: `[ auto-flow && dense? ]`, whether it is dense.
fn implicit_auto_flow<'i>(input: &mut Parser<'i>) -> ParseResult<'i, bool> {
  if input
    .try_parse(|input| input.expect_ident_matching("auto-flow"))
    .is_ok()
  {
    return Ok(
      input
        .try_parse(|input| input.expect_ident_matching("dense"))
        .is_ok(),
    );
  }

  input.expect_ident_matching("dense")?;
  input.expect_ident_matching("auto-flow")?;
  Ok(true)
}

/// A `grid-auto-rows` or `grid-auto-columns` value: one or more track sizes.
fn auto_track_list<'i>(input: &mut Parser<'i>) -> ParseResult<'i, GridTrackSizes> {
  let sizes = GridTrackSizes::from_css(input)?;

  if sizes.is_empty() {
    return Err(input.new_error_for_next_token());
  }
  Ok(sizes)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{GridLength, GridTemplateComponent, GridTrackSize, Length};

  /// From WPT's `css/css-grid/parsing/grid-template-shorthand-valid.html`.
  const TEMPLATE_VALID: &[&str] = &[
    "none",
    "none / none",
    "auto / auto",
    "none / [a] 0px",
    "[a] 10px / auto",
    "[] 10px [] / [] auto []",
    "[a] \"a\" 10px []",
    "\"a\"",
    "\"a a a\"",
    "\"a\" / 20%",
    "\"a\" [] [] \"b\"",
    "\"a\" [a] [b] \"b\"",
    "\"a\" calc(100% - 10px) / calc(10px)",
    "\"a\" 10px [a] \"b\" [a]",
    "\"a\" \"a\" [a] [a] \"b\" / auto",
    "\"a\" auto [a] \"b\" auto [b] / 10px",
  ];

  /// From WPT's `css/css-grid/parsing/grid-template-shorthand-invalid.html`.
  const TEMPLATE_INVALID: &[&str] = &[
    "auto",
    "none none",
    "none []",
    "10px",
    "[a]",
    "[a] 10px []",
    "[a] repeat(2, 10px)",
    "10px \"a\"",
    "[] [] \"a\"",
    "\"a\" none",
    "\"a\" 10px 10px",
    "\"a\" [a] 10px",
    "\"a\" [a] [a]",
    "\"a\" \"a\" [a] [a] / none",
    "none / \"a\"",
    "\"a\" / none",
    "\"a\" / repeat(2, 10px)",
    "none / \"a\" [] 10px",
  ];

  /// From WPT's `css/css-grid/parsing/grid-shorthand-valid.html`.
  const GRID_VALID: &[&str] = &[
    "100px / auto-flow dense 100px",
    "auto-flow dense 1fr / 100px",
    "100px / dense auto-flow 100px",
    "dense auto-flow 1fr / 100px",
    "100px / auto-flow 100px",
    "none / auto-flow 100px",
    "auto-flow 1fr / none",
    "auto / auto-flow 100px",
    "1fr / 1fr",
  ];

  /// From WPT's `css/css-grid/parsing/grid-shorthand-invalid.html`.
  const GRID_INVALID: &[&str] = &[
    "auto-flow 100px",
    "auto-flow / auto-flow",
    "auto-flow 1fr / auto-flow 1fr",
    "dense auto-flow / dense auto-flow",
    "auto / auto-flow foo()",
  ];

  /// Parses all of `css` as a declaration's value does.
  fn parse<T: for<'i> FromCss<'i>>(css: &str) -> Option<T> {
    Parser::new(css).parse_entirely(T::from_css).ok()
  }

  fn px(value: f32) -> GridTemplateComponent {
    GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Length::Px(value))))
  }

  #[test]
  fn accepts_what_blink_accepts() {
    for css in TEMPLATE_VALID {
      assert!(parse::<GridTemplate>(css).is_some(), "grid-template: {css}");
      assert!(parse::<Grid>(css).is_some(), "grid: {css}");
    }
    for css in GRID_VALID {
      assert!(parse::<Grid>(css).is_some(), "grid: {css}");
    }
  }

  #[test]
  fn rejects_what_blink_rejects() {
    for css in TEMPLATE_INVALID {
      assert!(parse::<GridTemplate>(css).is_none(), "grid-template: {css}");
      assert!(parse::<Grid>(css).is_none(), "grid: {css}");
    }
    for css in GRID_INVALID {
      assert!(parse::<Grid>(css).is_none(), "grid: {css}");
    }
  }

  #[test]
  fn areas_merge_the_line_names_between_rows() {
    let template = parse::<GridTemplate>("[a] \"x y\" 10px [b] [c] \"z z\" / 20px").unwrap();

    assert_eq!(
      template.rows,
      Some(vec![
        GridTemplateComponent::LineNames(vec!["a".to_owned()]),
        px(10.0),
        GridTemplateComponent::LineNames(vec!["b".to_owned(), "c".to_owned()]),
        GridTemplateComponent::Single(GridTrackSize::Fixed(GridLength::Unit(Length::Auto))),
      ])
    );
    assert_eq!(template.columns, Some(vec![px(20.0)]));
    assert_eq!(
      template.areas.map(|areas| areas.0),
      Some(vec![
        vec!["x".to_owned(), "y".to_owned()],
        vec!["z".to_owned(), "z".to_owned()],
      ])
    );
  }

  #[test]
  fn auto_flow_takes_the_axis_after_the_slash_or_before_it() {
    let rows = parse::<Grid>("auto-flow dense 40px / 20px").unwrap();

    assert_eq!(rows.auto_flow, GridAutoFlow::row().dense());
    assert_eq!(
      rows.auto_rows.as_deref(),
      Some(&[GridTrackSize::Fixed(GridLength::Unit(Length::Px(40.0)))][..])
    );
    assert_eq!(rows.template.rows, None);
    assert_eq!(rows.template.columns, Some(vec![px(20.0)]));

    let columns = parse::<Grid>("none / auto-flow 100px").unwrap();

    assert_eq!(columns.auto_flow, GridAutoFlow::column());
    assert_eq!(
      columns.auto_columns.as_deref(),
      Some(&[GridTrackSize::Fixed(GridLength::Unit(Length::Px(100.0)))][..])
    );
    assert_eq!(columns.template, GridTemplate::default());
  }
}
