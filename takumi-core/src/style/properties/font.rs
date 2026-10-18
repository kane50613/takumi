//! The `font` shorthand. <https://drafts.csswg.org/css-fonts-4/#font-prop>

use cssparser::{Parser, Token};

use crate::style::{
  FontFamily, FontSize, FontStretch, FontStyle, FontVariantCaps, FontWeight, FromCss, LineHeight,
  ParseResult, unexpected_token,
};

/// The `font` shorthand: the values it sets, the other font longhands taking their initial values.
/// System font keywords are not supported.
#[derive(Debug, Clone, PartialEq)]
pub struct FontShorthand {
  /// `font-style`.
  pub style: FontStyle,
  /// `font-variant-caps`, `normal` or `small-caps` only.
  pub caps: FontVariantCaps,
  /// `font-weight`.
  pub weight: FontWeight,
  /// `font-stretch`, its keywords only.
  pub stretch: FontStretch,
  /// `font-size`.
  pub size: FontSize,
  /// `line-height`.
  pub line_height: LineHeight,
  /// `font-family`.
  pub family: FontFamily,
}

impl<'i> FromCss<'i> for FontShorthand {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut style = None;
    let mut caps = None;
    let mut weight = None;
    let mut stretch = None;

    // Each of the four may come once in any order, `normal` standing for any of them.
    for _ in 0..4 {
      if input
        .try_parse(|input| input.expect_ident_matching("normal"))
        .is_ok()
      {
        continue;
      }

      if style.is_none()
        && let Ok(value) = input.try_parse(FontStyle::from_css)
      {
        style = Some(value);

        continue;
      }

      if caps.is_none()
        && input
          .try_parse(|input| input.expect_ident_matching("small-caps"))
          .is_ok()
      {
        caps = Some(FontVariantCaps::SmallCaps);

        continue;
      }

      if weight.is_none()
        && let Ok(value) = input.try_parse(FontWeight::from_css)
      {
        weight = Some(value);

        continue;
      }

      // `<font-width-css3>` takes keywords only, so a percentage stays the font size.
      if stretch.is_none()
        && let Ok(value) = input.try_parse(|input| -> ParseResult<'i, FontStretch> {
          let start = input.state();
          let location = input.current_source_location();
          let token = input.next()?.clone();

          if !matches!(token, Token::Ident(_)) {
            return Err(unexpected_token!(location, &token));
          }

          input.reset(&start);
          FontStretch::from_css(input)
        })
      {
        stretch = Some(value);

        continue;
      }

      break;
    }

    let size = FontSize::from_css(input)?;
    let line_height = if input.try_parse(|input| input.expect_delim('/')).is_ok() {
      LineHeight::from_css(input)?
    } else {
      LineHeight::default()
    };
    let family = FontFamily::from_css(input)?;

    Ok(Self {
      style: style.unwrap_or_default(),
      caps: caps.unwrap_or_default(),
      weight: weight.unwrap_or_default(),
      stretch: stretch.unwrap_or_default(),
      size,
      line_height,
      family,
    })
  }

  const VALID_TOKENS: &'static [&'static str] = &["<font-size> <font-family>"];
}

#[cfg(test)]
mod tests {
  use super::FontShorthand;
  use crate::style::{
    FontFamily, FontSize, FontStretch, FontStyle, FontVariantCaps, FontWeight, FromCssStr,
    LineHeight,
  };

  #[test]
  fn size_line_height_and_family() {
    let font = FontShorthand::from_css_str("25px/1 Ahem").expect("parses");

    assert_eq!(font.size, FontSize::from_css_str("25px").expect("size"));
    assert_eq!(
      font.line_height,
      LineHeight::from_css_str("1").expect("line height")
    );
    assert_eq!(
      font.family,
      FontFamily::from_css_str("Ahem").expect("family")
    );
    assert_eq!(font.style, FontStyle::default());
  }

  #[test]
  fn leading_values_in_any_order() {
    let font =
      FontShorthand::from_css_str("bold italic small-caps condensed 12px serif").expect("parses");

    assert_eq!(
      font.weight,
      FontWeight::from_css_str("bold").expect("weight")
    );
    assert_eq!(
      font.style,
      FontStyle::from_css_str("italic").expect("style")
    );
    assert_eq!(font.caps, FontVariantCaps::SmallCaps);
    assert_eq!(
      font.stretch,
      FontStretch::from_css_str("condensed").expect("stretch")
    );
  }

  #[test]
  fn a_percentage_is_the_font_size() {
    let font = FontShorthand::from_css_str("50% serif").expect("parses");

    assert_eq!(font.size, FontSize::from_css_str("50%").expect("size"));
    assert_eq!(font.stretch, FontStretch::default());
  }

  #[test]
  fn needs_a_size_and_a_family() {
    assert!(FontShorthand::from_css_str("bold serif").is_err());
    assert!(FontShorthand::from_css_str("12px").is_err());
  }
}
