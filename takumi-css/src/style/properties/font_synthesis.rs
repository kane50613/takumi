use crate::style::unexpected_token;
use cssparser::{Parser, Token, match_ignore_ascii_case};
use typed_builder::TypedBuilder;

use crate::style::{CssToken, FromCss, MakeComputed, ParseResult, declare_enum_from_css_impl};

/// Controls synthetic font behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Default, TypedBuilder)]
#[builder(field_defaults(default))]
pub struct FontSynthesis {
  /// Controls synthetic bolding when a matching font weight is unavailable.
  pub weight: FontSynthesisValue,
  /// Controls synthetic italics/obliques when a matching style is unavailable.
  pub style: FontSynthesisValue,
}

impl MakeComputed for FontSynthesis {}

impl<'i> FromCss<'i> for FontSynthesis {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut weight = FontSynthesisValue::None;
    let mut style = FontSynthesisValue::None;

    while !input.is_exhausted() {
      let location = input.current_source_location();
      let ident = input.expect_ident()?;

      match_ignore_ascii_case! {ident,
        "none" => {
          weight = FontSynthesisValue::None;
          style = FontSynthesisValue::None;
        },
        "weight" => {
          weight = FontSynthesisValue::Auto;
        },
        "style" => {
          style = FontSynthesisValue::Auto;
        },
        _ => return Err(unexpected_token!(location, &Token::Ident(ident.to_owned()))),
      };
    }

    if !input.is_exhausted() {
      return Err(input.new_error_for_next_token());
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
pub enum FontSynthesisValue {
  /// Synthetic is allowed.
  #[default]
  Auto,
  /// Synthetic is disabled.
  None,
}

impl FontSynthesisValue {
  pub fn is_allowed(self) -> bool {
    self == FontSynthesisValue::Auto
  }
}

declare_enum_from_css_impl!(
  FontSynthesisValue,
  "auto" => FontSynthesisValue::Auto,
  "none" => FontSynthesisValue::None,
);
