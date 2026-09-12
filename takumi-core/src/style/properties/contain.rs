//! The CSS Containment `contain` property.

use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{Animatable, CssToken, FromCss, MakeComputed, ParseResult, ToCss};

/// A `contain` value: `none | content | [ layout || style || paint ]`.
///
/// `size`, `inline-size` and `strict` are rejected, per the partial
/// implementation rule in <https://www.w3.org/TR/css-contain-2/#conform-partial>.
///
/// Approximate: `style` containment scopes nothing, because takumi has no
/// author-facing counters. List-item ordinals cross the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contain(u8);

impl Contain {
  /// No containment.
  pub const NONE: Self = Self(0);
  /// `layout`: the box is an independent formatting context with no baseline.
  pub const LAYOUT: Self = Self(1 << 0);
  /// `style`: counters and quotes are scoped to the box.
  pub const STYLE: Self = Self(1 << 1);
  /// `paint`: descendants are clipped to the box's padding edge.
  pub const PAINT: Self = Self(1 << 2);
  /// `content`: `layout style paint`.
  pub const CONTENT: Self = Self(Self::LAYOUT.0 | Self::STYLE.0 | Self::PAINT.0);

  /// Whether every containment type in `other` is present.
  pub const fn contains(self, other: Self) -> bool {
    self.0 & other.0 == other.0
  }

  pub(crate) fn into_taffy(self) -> taffy::Contain {
    let mut contain = taffy::Contain::NONE;

    if self.contains(Self::LAYOUT) {
      contain |= taffy::Contain::LAYOUT;
    }
    if self.contains(Self::PAINT) {
      contain |= taffy::Contain::PAINT;
    }

    contain
  }
}

impl MakeComputed for Contain {}

impl Animatable for Contain {}

impl<'i> FromCss<'i> for Contain {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("content"),
    CssToken::Keyword("layout"),
    CssToken::Keyword("style"),
    CssToken::Keyword("paint"),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut flags = Self::NONE;

    loop {
      let location = input.current_source_location();
      let token = input.next()?.clone();
      let Token::Ident(ident) = &token else {
        return Err(crate::style::unexpected_token!(location, &token));
      };

      let added = match_ignore_ascii_case! {ident,
        // The single-keyword values stand alone.
        "none" | "content" => {
          if flags != Self::NONE || !input.is_exhausted() {
            return Err(crate::style::unexpected_token!(location, &token));
          }

          return Ok(match_ignore_ascii_case! {ident,
            "content" => Self::CONTENT,
            _ => Self::NONE,
          });
        },
        "layout" => Self::LAYOUT,
        "style" => Self::STYLE,
        "paint" => Self::PAINT,
        _ => return Err(crate::style::unexpected_token!(location, &token)),
      };

      if flags.contains(added) {
        return Err(crate::style::unexpected_token!(location, &token));
      }

      flags = Self(flags.0 | added.0);

      if input.is_exhausted() {
        return Ok(flags);
      }
    }
  }
}

impl ToCss for Contain {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match *self {
      Self::NONE => return dest.write_str("none"),
      Self::CONTENT => return dest.write_str("content"),
      _ => {}
    }

    let mut written = false;

    for (flag, keyword) in [
      (Self::LAYOUT, "layout"),
      (Self::STYLE, "style"),
      (Self::PAINT, "paint"),
    ] {
      if !self.contains(flag) {
        continue;
      }

      if written {
        dest.write_char(' ')?;
      }

      dest.write_str(keyword)?;
      written = true;
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn parses_the_single_keyword_values() {
    for (css, expected) in [("none", Contain::NONE), ("content", Contain::CONTENT)] {
      let parsed = Contain::from_css_str(css).unwrap();

      assert_eq!(parsed, expected, "{css}");
      assert_eq!(parsed.to_css_string(), css);
    }

    assert!(Contain::from_css_str("content layout").is_err());
    assert!(Contain::from_css_str("layout none").is_err());
  }

  #[test]
  fn parses_the_containment_type_list() {
    for (css, expected) in [
      ("layout", "layout"),
      ("paint layout", "layout paint"),
      ("style paint", "style paint"),
      ("LAYOUT STYLE PAINT", "content"),
    ] {
      assert_eq!(
        Contain::from_css_str(css).unwrap().to_css_string(),
        expected,
        "{css}"
      );
    }

    assert!(Contain::from_css_str("layout layout").is_err());
    assert!(Contain::from_css_str("layout 1px").is_err());
  }

  #[test]
  fn rejects_the_unsupported_size_containment() {
    for css in [
      "size",
      "inline-size",
      "strict",
      "size layout",
      "layout size",
    ] {
      assert!(Contain::from_css_str(css).is_err(), "{css}");
    }
  }

  #[test]
  fn only_layout_and_paint_reach_taffy() {
    assert_eq!(Contain::NONE.into_taffy(), taffy::Contain::NONE);
    assert_eq!(Contain::STYLE.into_taffy(), taffy::Contain::NONE);
    assert_eq!(Contain::LAYOUT.into_taffy(), taffy::Contain::LAYOUT);
    assert_eq!(Contain::PAINT.into_taffy(), taffy::Contain::PAINT);
    assert_eq!(Contain::CONTENT.into_taffy(), taffy::Contain::CONTENT);
  }
}
