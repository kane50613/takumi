//! The CSS Containment `contain` property.

use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{Animatable, CssToken, FromCss, MakeComputed, ParseResult, ToCss};

/// A `contain` value: `none | strict | content | [ size || inline-size || layout || style || paint ]`.
///
/// Approximate: only `layout` and `paint` containment reach layout, where they
/// make the box an independent formatting context. `size`, `inline-size` and
/// `style` containment parse and serialize but have no effect, so `strict`
/// behaves like `layout paint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contain(u8);

impl Contain {
  /// No containment.
  pub const NONE: Self = Self(0);
  /// `inline-size`: size containment on the inline axis.
  pub const INLINE_SIZE: Self = Self(1 << 0);
  /// `size`: size containment on both axes.
  pub const SIZE: Self = Self((1 << 0) | (1 << 1));
  /// `layout`: the box is an independent formatting context with no baseline.
  pub const LAYOUT: Self = Self(1 << 2);
  /// `style`: counters and quotes are scoped to the box.
  pub const STYLE: Self = Self(1 << 3);
  /// `paint`: descendants are clipped to the box's padding edge.
  pub const PAINT: Self = Self(1 << 4);
  /// `content`: `layout style paint`.
  pub const CONTENT: Self = Self(Self::LAYOUT.0 | Self::STYLE.0 | Self::PAINT.0);
  /// `strict`: `size layout style paint`.
  pub const STRICT: Self = Self(Self::SIZE.0 | Self::CONTENT.0);

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
    CssToken::Keyword("strict"),
    CssToken::Keyword("content"),
    CssToken::Keyword("size"),
    CssToken::Keyword("inline-size"),
    CssToken::Keyword("layout"),
    CssToken::Keyword("style"),
    CssToken::Keyword("paint"),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut flags = Self::NONE;
    let mut seen_size = false;

    loop {
      let location = input.current_source_location();
      let token = input.next()?.clone();
      let Token::Ident(ident) = &token else {
        return Err(crate::style::unexpected_token!(location, &token));
      };

      let added = match_ignore_ascii_case! {ident,
        // The single-keyword values stand alone.
        "none" | "strict" | "content" => {
          if flags != Self::NONE || !input.is_exhausted() {
            return Err(crate::style::unexpected_token!(location, &token));
          }

          return Ok(match_ignore_ascii_case! {ident,
            "strict" => Self::STRICT,
            "content" => Self::CONTENT,
            _ => Self::NONE,
          });
        },
        "size" => {
          seen_size = true;
          Self::SIZE
        },
        "inline-size" => {
          seen_size = true;
          Self::INLINE_SIZE
        },
        "layout" => Self::LAYOUT,
        "style" => Self::STYLE,
        "paint" => Self::PAINT,
        _ => return Err(crate::style::unexpected_token!(location, &token)),
      };

      // `size` and `inline-size` exclude each other, so both share one slot.
      let is_duplicate = if added.contains(Self::INLINE_SIZE) {
        seen_size && flags.contains(Self::INLINE_SIZE)
      } else {
        flags.contains(added)
      };

      if is_duplicate {
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
      Self::STRICT => return dest.write_str("strict"),
      Self::CONTENT => return dest.write_str("content"),
      _ => {}
    }

    let keywords = [
      (Self::SIZE, "size"),
      (Self::INLINE_SIZE, "inline-size"),
      (Self::LAYOUT, "layout"),
      (Self::STYLE, "style"),
      (Self::PAINT, "paint"),
    ];
    let mut written = false;

    for (flag, keyword) in keywords {
      // `size` covers `inline-size`, so it is written instead of it.
      if !self.contains(flag) || (flag == Self::INLINE_SIZE && self.contains(Self::SIZE)) {
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
    for (css, expected) in [
      ("none", Contain::NONE),
      ("strict", Contain::STRICT),
      ("content", Contain::CONTENT),
    ] {
      let parsed = Contain::from_css_str(css).unwrap();

      assert_eq!(parsed, expected, "{css}");
      assert_eq!(parsed.to_css_string(), css);
    }

    assert!(Contain::from_css_str("strict layout").is_err());
    assert!(Contain::from_css_str("layout none").is_err());
  }

  #[test]
  fn parses_the_containment_type_list() {
    for (css, expected) in [
      ("layout", "layout"),
      ("paint layout", "layout paint"),
      ("inline-size layout", "inline-size layout"),
      ("style size paint", "size style paint"),
      ("SIZE LAYOUT STYLE PAINT", "strict"),
    ] {
      assert_eq!(
        Contain::from_css_str(css).unwrap().to_css_string(),
        expected,
        "{css}"
      );
    }

    assert!(Contain::from_css_str("layout layout").is_err());
    assert!(Contain::from_css_str("size inline-size").is_err());
    assert!(Contain::from_css_str("layout 1px").is_err());
  }

  #[test]
  fn only_layout_and_paint_reach_taffy() {
    assert_eq!(Contain::NONE.into_taffy(), taffy::Contain::NONE);
    assert_eq!(Contain::SIZE.into_taffy(), taffy::Contain::NONE);
    assert_eq!(Contain::STYLE.into_taffy(), taffy::Contain::NONE);
    assert_eq!(Contain::LAYOUT.into_taffy(), taffy::Contain::LAYOUT);
    assert_eq!(Contain::PAINT.into_taffy(), taffy::Contain::PAINT);
    assert_eq!(Contain::CONTENT.into_taffy(), taffy::Contain::CONTENT);
    assert_eq!(Contain::STRICT.into_taffy(), taffy::Contain::CONTENT);
  }
}
