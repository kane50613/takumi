use cssparser::{Parser, Token, match_ignore_ascii_case};
use typed_builder::TypedBuilder;

use crate::style::{
  CssToken, FromCss, MakeComputed, ParseResult, declare_enum_from_css_impl, unexpected_token,
};

/// Controls synthetic font behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Default, TypedBuilder)]
#[builder(field_defaults(default))]
pub struct FontSynthesis {
  /// Controls synthetic bolding when a matching font weight is unavailable.
  pub weight: FontSynthesisMode,
  /// Controls synthetic italics/obliques when a matching style is unavailable.
  pub style: FontSynthesisMode,
}

impl MakeComputed for FontSynthesis {}

impl<'i> FromCss<'i> for FontSynthesis {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut weight = FontSynthesisMode::None;
    let mut style = FontSynthesisMode::None;

    while !input.is_exhausted() {
      let location = input.current_source_location();
      let ident = input.expect_ident()?;

      match_ignore_ascii_case! {ident,
        "none" => {
          weight = FontSynthesisMode::None;
          style = FontSynthesisMode::None;
        },
        "weight" => {
          weight = FontSynthesisMode::Auto;
        },
        "style" => {
          style = FontSynthesisMode::Auto;
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
pub enum FontSynthesisMode {
  /// Synthetic is allowed.
  #[default]
  Auto,
  /// Synthetic is disabled.
  None,
}

impl FontSynthesisMode {
  /// Whether synthesis is permitted.
  pub(crate) fn is_allowed(self) -> bool {
    self == FontSynthesisMode::Auto
  }
}

declare_enum_from_css_impl!(
  FontSynthesisMode,
  "auto" => FontSynthesisMode::Auto,
  "none" => FontSynthesisMode::None,
);
