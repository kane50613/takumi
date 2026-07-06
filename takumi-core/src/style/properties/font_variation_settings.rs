use std::fmt;

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
pub(crate) type FontVariationSettings = Box<[FontVariation]>;

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

      Ok(FontVariation { tag, value })
    })?;

    Ok(list.into_boxed_slice())
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
