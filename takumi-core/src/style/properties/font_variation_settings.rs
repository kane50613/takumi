use std::{fmt, sync::Arc};

use cssparser::Parser;

use super::font_feature_settings::{Tag, parse_opentype_tag};
use crate::style::{CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss};

/// An OpenType font variation setting (tag + value) from CSS `font-variation-settings`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FontVariation {
  /// The OpenType tag for this setting.
  pub tag: Tag,
  /// The variation value.
  pub value: f32,
}

impl FontVariation {
  /// Creates a new variation setting.
  pub const fn new(tag: Tag, value: f32) -> Self {
    Self { tag, value }
  }

  pub(crate) fn into_parlance(self) -> parley::FontVariation {
    parley::FontVariation {
      tag: self.tag.into_parlance(),
      value: self.value,
    }
  }
}

/// Controls variable font axis values via CSS font-variation-settings property.
///
/// This allows fine-grained control over variable font characteristics like weight,
/// width, slant, and other custom axes defined in the font.
pub(crate) type FontVariationSettings = Arc<[FontVariation]>;

impl MakeComputed for FontVariationSettings {}

impl<'i> FromCss<'i> for FontVariationSettings {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Arc::new([]));
    }

    let list = input.parse_comma_separated(|input| {
      let tag = parse_opentype_tag::<FontVariationSettings>(input)?;
      let value = input.expect_number()?;

      Ok(FontVariation { tag, value })
    })?;

    Ok(list.into())
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl ToCss for FontVariation {
  // An empty `font-variation-settings` list is the keyword `normal`.
  const EMPTY_LIST_KEYWORD: Option<&'static str> = Some("normal");

  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "\"{}\" {}", self.tag, self.value)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_font_variation_settings() {
    for (css, expected) in [
      ("normal", Arc::new([]) as FontVariationSettings),
      (
        "\"wght\" 400",
        Arc::new([FontVariation::new(Tag::new(b"wght"), 400.0)]),
      ),
      (
        "\"wght\" 400, \"slnt\" -10",
        Arc::new([
          FontVariation::new(Tag::new(b"wght"), 400.0),
          FontVariation::new(Tag::new(b"slnt"), -10.0),
        ]),
      ),
    ] {
      assert_eq!(
        FontVariationSettings::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_font_variation_settings_round_trip() {
    for css in ["normal", "\"wght\" 400", "\"wght\" 400, \"slnt\" -10"] {
      let parsed = FontVariationSettings::from_css_str(css).unwrap();
      let reparsed = FontVariationSettings::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_font_variation_settings_invalid() {
    assert!(FontVariationSettings::from_css_str("123").is_err());
    assert!(FontVariationSettings::from_css_str("wght 400").is_err());
  }
}
