use std::fmt;

use cssparser::{Parser, Token, serialize_identifier};

use crate::{
  style::{Animatable, FromCss, MakeComputed, ParseResult, ToCss, unexpected_token},
  viewport::ColorScheme,
};

/// `color-scheme`: the color schemes an element can be rendered in, as the ordered list of its keywords.
/// `normal` is the empty list.
/// <https://drafts.csswg.org/css-color-adjust-1/#color-scheme-prop>
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ColorSchemes(Box<[Box<str>]>);

/// The keywords `color-scheme` takes, which a custom identifier cannot be.
const RESERVED: &[&str] = &[
  "normal",
  "initial",
  "inherit",
  "unset",
  "revert",
  "revert-layer",
  "default",
];

impl ColorSchemes {
  fn lists(&self, keyword: &str) -> bool {
    self.0.iter().any(|listed| listed.as_ref() == keyword)
  }

  /// The used color scheme: the preferred one when listed, else the one listed, else light.
  /// <https://drafts.csswg.org/css-color-adjust-1/#determining-the-used-color-scheme>
  pub fn used(&self, preferred: ColorScheme) -> ColorScheme {
    let keyword = match preferred {
      ColorScheme::Light => "light",
      ColorScheme::Dark => "dark",
    };

    if self.lists(keyword) {
      preferred
    } else if self.lists("dark") {
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

    let mut keywords = Vec::new();
    let mut only = false;
    let mut schemes = 0;

    while let Ok(token) = input.try_parse(Parser::expect_ident_cloned) {
      let keyword = token.to_ascii_lowercase();

      if keyword == "only" {
        // `only` comes before or after the whole list, once.
        if only || (schemes > 0 && !input.is_exhausted()) {
          return Err(unexpected_token!(&Token::Ident(token)));
        }

        only = true;
      } else if RESERVED.contains(&keyword.as_str()) {
        return Err(unexpected_token!(&Token::Ident(token)));
      } else {
        schemes += 1;
      }

      keywords.push(match keyword.as_str() {
        "light" | "dark" | "only" => keyword.into(),
        _ => token.as_ref().into(),
      });
    }

    if schemes == 0 || !input.is_exhausted() {
      return Err(input.new_error_for_next_token());
    }

    Ok(Self(keywords.into()))
  }

  const VALID_TOKENS: &'static [&'static str] = &["normal", "light", "dark", "only"];
}

impl ToCss for ColorSchemes {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.0.is_empty() {
      return dest.write_str("normal");
    }

    for (index, keyword) in self.0.iter().enumerate() {
      if index > 0 {
        dest.write_char(' ')?;
      }

      serialize_identifier(keyword, dest)?;
    }

    Ok(())
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
  fn parses_color_scheme_and_keeps_its_keywords_in_order() {
    for (css, serialized) in [
      ("normal", "normal"),
      ("light dark", "light dark"),
      ("dark light", "dark light"),
      ("Dark brand", "dark brand"),
      ("only dark", "only dark"),
      ("light dark only", "light dark only"),
    ] {
      assert_eq!(schemes(css).to_css_string(), serialized, "{css}");
    }

    for invalid in [
      "",
      "only",
      "light normal",
      "only light only",
      "light only dark",
      "light 1px",
    ] {
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
