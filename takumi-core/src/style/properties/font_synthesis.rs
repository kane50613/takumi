use crate::style::unexpected_token;
use cssparser::{Parser, Token, match_ignore_ascii_case};
use typed_builder::TypedBuilder;

use crate::style::{CssToken, FromCss, MakeComputed, ParseResult, declare_enum_from_css_impl};

/// Controls synthetic font behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Default, TypedBuilder)]
#[builder(field_defaults(default))]
pub struct FontSynthesis {
  /// Controls synthetic bolding when a matching font weight is unavailable.
  pub weight: FontSynthesic,
  /// Controls synthetic italics/obliques when a matching style is unavailable.
  pub style: FontSynthesic,
}

impl MakeComputed for FontSynthesis {}

impl<'i> FromCss<'i> for FontSynthesis {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut weight = FontSynthesic::None;
    let mut style = FontSynthesic::None;

    while !input.is_exhausted() {
      let location = input.current_source_location();
      let ident = input.expect_ident()?;

      match_ignore_ascii_case! {ident,
        "none" => {
          weight = FontSynthesic::None;
          style = FontSynthesic::None;
        },
        "weight" => {
          weight = FontSynthesic::Auto;
        },
        "style" => {
          style = FontSynthesic::Auto;
        },
        _ => return Err(unexpected_token!(location, &Token::Ident(ident.to_owned()))),
      };
    }

    Ok(Self { weight, style })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("weight"),
    CssToken::Keyword("style"),
  ];
}

/// Control mode for synthetic.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum FontSynthesic {
  /// Synthetic is allowed.
  #[default]
  Auto,
  /// Synthetic is disabled.
  None,
}

impl FontSynthesic {
  /// Whether synthesis is permitted.
  pub fn is_allowed(self) -> bool {
    self == FontSynthesic::Auto
  }
}

declare_enum_from_css_impl!(
  FontSynthesic,
  "auto" => FontSynthesic::Auto,
  "none" => FontSynthesic::None,
);
