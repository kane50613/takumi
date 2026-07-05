use std::fmt;

use cssparser::Parser;
use parley::{FontVariation, setting::Tag};

use super::font_feature_settings::parse_opentype_tag;
use crate::style::{
  Animatable, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss,
};

/// A single `font-variation-settings` entry: a variation axis tag and its value.
/// Wraps `parley::FontVariation` so callers need not depend on `parley` (and so the
/// engine-only type doesn't appear directly in `ComputedStyle`'s field).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontVariationSetting(pub(crate) FontVariation);

impl FontVariationSetting {
  /// Creates a variation setting from its 4-byte axis tag (e.g. `*b"wght"`) and value.
  pub fn new(tag: [u8; 4], value: f32) -> Self {
    Self(FontVariation {
      tag: Tag::from_bytes(tag),
      value,
    })
  }
}

/// Controls variable font axis values via CSS font-variation-settings property.
///
/// This allows fine-grained control over variable font characteristics like weight,
/// width, slant, and other custom axes defined in the font.
pub(crate) type FontVariationSettings = Box<[FontVariationSetting]>;

impl MakeComputed for FontVariationSettings {}

impl<'i> FromCss<'i> for FontVariationSettings {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Box::new([]));
    }

    let list = input.parse_comma_separated(|input| {
      let tag = parse_opentype_tag::<FontVariationSettings>(input)?;
      let value = input.expect_number()?;

      Ok(FontVariationSetting(FontVariation { tag, value }))
    })?;

    Ok(list.into_boxed_slice())
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl Animatable for FontVariationSetting {}

impl ToCss for FontVariationSetting {
  // An empty `font-variation-settings` list is the keyword `normal`.
  const EMPTY_LIST_KEYWORD: Option<&'static str> = Some("normal");

  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "\"{}\" {}", self.0.tag, self.0.value)
  }
}
