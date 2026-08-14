use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, BackgroundImage, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss,
  declare_enum_from_css_impl, properties::write_css_string, unexpected_token,
};

/// The counter style a list item's marker is generated from.
///
/// Corresponds to CSS `list-style-type`.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub enum ListStyleType {
  /// No marker is generated.
  None,
  /// A filled circle, `•`.
  #[default]
  Disc,
  /// A hollow circle, `◦`.
  Circle,
  /// A filled square, `■`.
  Square,
  /// Western decimal numbers.
  Decimal,
  /// Decimal numbers padded to two digits.
  DecimalLeadingZero,
  /// Lowercase ASCII letters.
  LowerAlpha,
  /// Uppercase ASCII letters.
  UpperAlpha,
  /// Lowercase roman numerals.
  LowerRoman,
  /// Uppercase roman numerals.
  UpperRoman,
  /// A literal string used for every item.
  String(Arc<str>),
}

/// The image a list item's marker draws instead of its counter style.
///
/// Corresponds to CSS `list-style-image`. The image is shared with every
/// descendant that inherits it, and its lengths resolve once the marker box is
/// built.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListStyleImage(pub Option<Arc<BackgroundImage>>);

impl ListStyleImage {
  pub(crate) fn image(&self) -> Option<&BackgroundImage> {
    self.0.as_deref()
  }
}

impl MakeComputed for ListStyleImage {}

impl Animatable for ListStyleImage {}

impl<'i> FromCss<'i> for ListStyleImage {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let image = BackgroundImage::from_css(input)?;

    Ok(Self(image.paints().then(|| Arc::new(image))))
  }

  const VALID_TOKENS: &'static [CssToken] = BackgroundImage::VALID_TOKENS;
}

impl ToCss for ListStyleImage {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match &self.0 {
      Some(image) => image.to_css(dest),
      None => dest.write_str("none"),
    }
  }
}

/// Where a list item's marker sits relative to the item's content.
///
/// Corresponds to CSS `list-style-position`.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[non_exhaustive]
pub enum ListStylePosition {
  /// The marker hangs outside the item's content box.
  #[default]
  Outside,
  /// The marker is the first inline content of the item.
  Inside,
}

declare_enum_from_css_impl!(
  ListStylePosition,
  "outside" => ListStylePosition::Outside,
  "inside" => ListStylePosition::Inside,
);

impl Animatable for ListStylePosition {}

/// The `list-style` shorthand.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ListStyleShorthand {
  /// The counter style.
  pub style_type: ListStyleType,
  /// The marker position.
  pub position: ListStylePosition,
  /// The marker image, which replaces the counter style when it loads.
  pub image: ListStyleImage,
}

const ROMAN_DIGITS: &[(i32, &str, &str)] = &[
  (1000, "m", "M"),
  (900, "cm", "CM"),
  (500, "d", "D"),
  (400, "cd", "CD"),
  (100, "c", "C"),
  (90, "xc", "XC"),
  (50, "l", "L"),
  (40, "xl", "XL"),
  (10, "x", "X"),
  (9, "ix", "IX"),
  (5, "v", "V"),
  (4, "iv", "IV"),
  (1, "i", "I"),
];

fn alphabetic(ordinal: i32, first: char) -> String {
  let Ok(mut remaining) = u32::try_from(ordinal) else {
    return ordinal.to_string();
  };

  if remaining == 0 {
    return ordinal.to_string();
  }

  let mut letters = Vec::new();
  while remaining > 0 {
    remaining -= 1;
    letters.push(char::from_u32(first as u32 + remaining % 26).unwrap_or(first));
    remaining /= 26;
  }

  letters.iter().rev().collect()
}

fn roman(ordinal: i32, upper: bool) -> String {
  if !(1..=3999).contains(&ordinal) {
    return ordinal.to_string();
  }

  let mut remaining = ordinal;
  let mut text = String::new();
  for &(value, lower_digit, upper_digit) in ROMAN_DIGITS {
    while remaining >= value {
      text.push_str(if upper { upper_digit } else { lower_digit });
      remaining -= value;
    }
  }

  text
}

fn decimal_leading_zero(ordinal: i32) -> String {
  // css-counter-styles counts the sign against the pad length, which leaves a
  // negative value nothing to pad with.
  if ordinal < 0 {
    return ordinal.to_string();
  }

  format!("{ordinal:02}")
}

impl ListStyleType {
  /// The marker string for an item at `ordinal`, including the counter style's suffix.
  pub(crate) fn marker_text(&self, ordinal: i32) -> Option<String> {
    let (representation, suffix) = match self {
      ListStyleType::None => return None,
      ListStyleType::String(value) => return Some(value.as_ref().to_owned()),
      ListStyleType::Disc => ("\u{2022}".to_owned(), " "),
      ListStyleType::Circle => ("\u{25e6}".to_owned(), " "),
      ListStyleType::Square => ("\u{25a0}".to_owned(), " "),
      ListStyleType::Decimal => (ordinal.to_string(), ". "),
      ListStyleType::DecimalLeadingZero => (decimal_leading_zero(ordinal), ". "),
      ListStyleType::LowerAlpha => (alphabetic(ordinal, 'a'), ". "),
      ListStyleType::UpperAlpha => (alphabetic(ordinal, 'A'), ". "),
      ListStyleType::LowerRoman => (roman(ordinal, false), ". "),
      ListStyleType::UpperRoman => (roman(ordinal, true), ". "),
    };

    Some(representation + suffix)
  }

  fn from_ident(ident: &str) -> Option<Self> {
    match_ignore_ascii_case! {ident,
      "none" => Some(ListStyleType::None),
      "disc" => Some(ListStyleType::Disc),
      "circle" => Some(ListStyleType::Circle),
      "square" => Some(ListStyleType::Square),
      "decimal" => Some(ListStyleType::Decimal),
      "decimal-leading-zero" => Some(ListStyleType::DecimalLeadingZero),
      "lower-alpha" | "lower-latin" => Some(ListStyleType::LowerAlpha),
      "upper-alpha" | "upper-latin" => Some(ListStyleType::UpperAlpha),
      "lower-roman" => Some(ListStyleType::LowerRoman),
      "upper-roman" => Some(ListStyleType::UpperRoman),
      _ => None,
    }
  }
}

impl MakeComputed for ListStyleType {}

impl Animatable for ListStyleType {}

const LIST_STYLE_TYPE_TOKENS: &[CssToken] = &[
  CssToken::Keyword("none"),
  CssToken::Keyword("disc"),
  CssToken::Keyword("circle"),
  CssToken::Keyword("square"),
  CssToken::Keyword("decimal"),
  CssToken::Keyword("decimal-leading-zero"),
  CssToken::Keyword("lower-alpha"),
  CssToken::Keyword("lower-latin"),
  CssToken::Keyword("upper-alpha"),
  CssToken::Keyword("upper-latin"),
  CssToken::Keyword("lower-roman"),
  CssToken::Keyword("upper-roman"),
  CssToken::Syntax(CssSyntaxKind::String),
];

impl<'i> FromCss<'i> for ListStyleType {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?.clone();

    match token {
      Token::QuotedString(ref value) => Ok(ListStyleType::String(value.as_ref().into())),
      Token::Ident(ref ident) => {
        Self::from_ident(ident).ok_or_else(|| unexpected_token!(Self, location, &token))
      }
      other => Err(unexpected_token!(Self, location, &other)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = LIST_STYLE_TYPE_TOKENS;
}

impl ToCss for ListStyleType {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      ListStyleType::None => dest.write_str("none"),
      ListStyleType::Disc => dest.write_str("disc"),
      ListStyleType::Circle => dest.write_str("circle"),
      ListStyleType::Square => dest.write_str("square"),
      ListStyleType::Decimal => dest.write_str("decimal"),
      ListStyleType::DecimalLeadingZero => dest.write_str("decimal-leading-zero"),
      ListStyleType::LowerAlpha => dest.write_str("lower-alpha"),
      ListStyleType::UpperAlpha => dest.write_str("upper-alpha"),
      ListStyleType::LowerRoman => dest.write_str("lower-roman"),
      ListStyleType::UpperRoman => dest.write_str("upper-roman"),
      ListStyleType::String(value) => write_css_string(dest, value),
    }
  }
}

impl<'i> FromCss<'i> for ListStyleShorthand {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut style_type = None;
    let mut position = None;
    let mut image = None;

    while !input.is_exhausted() {
      let location = input.current_source_location();

      if position.is_none()
        && let Ok(parsed) = input.try_parse(ListStylePosition::from_css)
      {
        position = Some(parsed);
        continue;
      }

      if style_type.is_none()
        && let Ok(parsed) = input.try_parse(ListStyleType::from_css)
      {
        style_type = Some(parsed);
        continue;
      }

      if image.is_none()
        && let Ok(parsed) = input.try_parse(ListStyleImage::from_css)
      {
        image = Some(parsed);
        continue;
      }

      let token = input.next()?.clone();
      return Err(unexpected_token!(Self, location, &token));
    }

    Ok(Self {
      style_type: style_type.unwrap_or_default(),
      position: position.unwrap_or_default(),
      image: image.unwrap_or_default(),
    })
  }

  const VALID_TOKENS: &'static [CssToken] = LIST_STYLE_TYPE_TOKENS;
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn parses_keywords_and_strings() {
    assert_eq!(
      ListStyleType::from_css_str("upper-roman"),
      Ok(ListStyleType::UpperRoman)
    );
    assert_eq!(
      ListStyleType::from_css_str("lower-latin"),
      Ok(ListStyleType::LowerAlpha)
    );
    assert_eq!(
      ListStyleType::from_css_str("\"-\""),
      Ok(ListStyleType::String("-".into()))
    );
    assert!(ListStyleType::from_css_str("bogus").is_err());
  }

  #[test]
  fn parses_shorthand_in_any_order() {
    assert_eq!(
      ListStyleShorthand::from_css_str("inside square"),
      Ok(ListStyleShorthand {
        style_type: ListStyleType::Square,
        position: ListStylePosition::Inside,
        image: ListStyleImage::default(),
      })
    );
    assert_eq!(
      ListStyleShorthand::from_css_str("none"),
      Ok(ListStyleShorthand {
        style_type: ListStyleType::None,
        position: ListStylePosition::Outside,
        image: ListStyleImage::default(),
      })
    );
    assert_eq!(
      ListStyleShorthand::from_css_str("url(bullet.png) inside"),
      Ok(ListStyleShorthand {
        style_type: ListStyleType::Disc,
        position: ListStylePosition::Inside,
        image: ListStyleImage(Some(Arc::new(BackgroundImage::Url("bullet.png".into())))),
      })
    );
  }

  #[test]
  fn numbers_the_marker_per_counter_style() {
    assert_eq!(
      ListStyleType::Decimal.marker_text(3).as_deref(),
      Some("3. ")
    );
    assert_eq!(
      ListStyleType::DecimalLeadingZero.marker_text(7).as_deref(),
      Some("07. ")
    );
    assert_eq!(
      ListStyleType::LowerAlpha.marker_text(28).as_deref(),
      Some("ab. ")
    );
    assert_eq!(
      ListStyleType::UpperRoman.marker_text(1994).as_deref(),
      Some("MCMXCIV. ")
    );
    assert_eq!(
      ListStyleType::Disc.marker_text(1).as_deref(),
      Some("\u{2022} ")
    );
    assert_eq!(ListStyleType::None.marker_text(1), None);
  }

  #[test]
  fn a_negative_value_spends_its_padding_on_the_sign() {
    assert_eq!(
      ListStyleType::DecimalLeadingZero.marker_text(-7).as_deref(),
      Some("-7. ")
    );
  }

  #[test]
  fn falls_back_to_decimal_outside_the_counter_range() {
    assert_eq!(
      ListStyleType::LowerAlpha.marker_text(0).as_deref(),
      Some("0. ")
    );
    assert_eq!(
      ListStyleType::LowerRoman.marker_text(-2).as_deref(),
      Some("-2. ")
    );
  }
}
