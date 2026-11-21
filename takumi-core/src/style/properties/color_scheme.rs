use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::{
  style::{Animatable, FromCss, MakeComputed, ParseResult, ToCss, unexpected_token},
  viewport::ColorScheme,
};

/// `color-scheme`: the color schemes an element can be rendered in.
/// <https://drafts.csswg.org/css-color-adjust-1/#color-scheme-prop>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ColorSchemes {
  /// `light` is listed.
  pub light: bool,
  /// `dark` is listed.
  pub dark: bool,
  /// `only` is listed.
  pub only: bool,
}

impl ColorSchemes {
  /// The used color scheme: the preferred one when listed, else the one listed, else light.
  /// <https://drafts.csswg.org/css-color-adjust-1/#determining-the-used-color-scheme>
  pub fn used(self, preferred: ColorScheme) -> ColorScheme {
    let listed = |scheme| match scheme {
      ColorScheme::Light => self.light,
      ColorScheme::Dark => self.dark,
    };

    if listed(preferred) {
      preferred
    } else if self.dark {
      ColorScheme::Dark
    } else {
      ColorScheme::Light
    }
  }
}

impl MakeComputed for ColorSchemes {}

impl Animatable for ColorSchemes {}

impl<'i> FromCss<'i> for ColorSchemes {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Self::default());
    }

    let mut schemes = Self::default();
    let mut keywords = 0;

    while let Ok(token) = input.try_parse(Parser::expect_ident_cloned) {
      match_ignore_ascii_case! { &token,
        "light" => schemes.light = true,
        "dark" => schemes.dark = true,
        "only" if !schemes.only => {
          schemes.only = true;
          continue;
        },
        "only" | "normal" | "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default" => {
          return Err(unexpected_token!(&Token::Ident(token)));
        },
        _ => {},
      }

      keywords += 1;
    }

    if keywords == 0 || !input.is_exhausted() {
      return Err(input.new_error_for_next_token());
    }

    Ok(schemes)
  }

  const VALID_TOKENS: &'static [&'static str] = &["normal", "light", "dark", "only"];
}

impl ToCss for ColorSchemes {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let keywords = [
      (self.light, "light"),
      (self.dark, "dark"),
      (self.only, "only"),
    ]
    .into_iter()
    .filter_map(|(listed, keyword)| listed.then_some(keyword))
    .collect::<Vec<_>>();

    if keywords.is_empty() || keywords == ["only"] {
      return dest.write_str("normal");
    }

    dest.write_str(&keywords.join(" "))
  }
}

#[cfg(test)]
mod tests {
  use super::ColorSchemes;
  use crate::{
    style::{FromCssStr, ToCss},
    viewport::ColorScheme,
  };

  fn schemes(css: &str) -> ColorSchemes {
    ColorSchemes::from_css_str(css).unwrap_or_else(|_| panic!("{css} should parse"))
  }

  #[test]
  fn parses_color_scheme() {
    assert_eq!(schemes("normal"), ColorSchemes::default());
    assert_eq!(
      schemes("light dark"),
      ColorSchemes {
        light: true,
        dark: true,
        only: false
      }
    );
    assert_eq!(
      schemes("only dark"),
      ColorSchemes {
        light: false,
        dark: true,
        only: true
      }
    );
    assert_eq!(
      schemes("dark brand"),
      ColorSchemes {
        light: false,
        dark: true,
        only: false
      }
    );
    assert_eq!(schemes("dark light").to_css_string(), "light dark");

    for invalid in ["", "only", "light normal", "only light only", "light 1px"] {
      assert!(
        ColorSchemes::from_css_str(invalid).is_err(),
        "{invalid} should not parse"
      );
    }
  }

  #[test]
  fn uses_the_preferred_scheme_when_listed() {
    let both = schemes("light dark");

    assert_eq!(both.used(ColorScheme::Light), ColorScheme::Light);
    assert_eq!(both.used(ColorScheme::Dark), ColorScheme::Dark);
    assert_eq!(schemes("dark").used(ColorScheme::Light), ColorScheme::Dark);
    assert_eq!(schemes("light").used(ColorScheme::Dark), ColorScheme::Light);
    assert_eq!(
      ColorSchemes::default().used(ColorScheme::Dark),
      ColorScheme::Light
    );
  }
}
