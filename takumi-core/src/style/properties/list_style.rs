use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, match_ignore_ascii_case, serialize_string};

use crate::style::{
  Animatable, BackgroundImage, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss,
  declare_enum_from_css_impl, unexpected_token,
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
  /// A filled square, `▪`.
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
  /// A triangle pointing at the closed disclosure widget's content.
  DisclosureClosed,
  /// A triangle pointing down at the open disclosure widget's content.
  DisclosureOpen,
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
  /// Whether this style draws a bullet symbol rather than alphanumeric text.
  pub(crate) fn is_symbolic(&self) -> bool {
    matches!(
      self,
      ListStyleType::Disc
        | ListStyleType::Circle
        | ListStyleType::Square
        | ListStyleType::DisclosureClosed
        | ListStyleType::DisclosureOpen
    )
  }

  /// The marker string for an item at `ordinal`, including the counter style's
  /// suffix. `is_rtl` only reaches `disclosure-closed`, whose triangle points
  /// the way the text runs.
  pub(crate) fn marker_text(&self, ordinal: i32, is_rtl: bool) -> Option<String> {
    let (representation, suffix) = match self {
      ListStyleType::None => return None,
      ListStyleType::String(value) => return Some(value.as_ref().to_owned()),
      ListStyleType::Disc => ("\u{2022}".to_owned(), " "),
      ListStyleType::Circle => ("\u{25e6}".to_owned(), " "),
      // Blink paints `square` at `(ascent*2/3 + 1)/2` px
      // (`RelativeSymbolMarkerRect`); `▪` tracks that size, `■` does not.
      ListStyleType::Square => ("\u{25aa}".to_owned(), " "),
      // Ref: https://drafts.csswg.org/css-counter-styles-3/#simple-symbolic
      ListStyleType::DisclosureClosed if is_rtl => ("\u{25c2}".to_owned(), " "),
      ListStyleType::DisclosureClosed => ("\u{25b8}".to_owned(), " "),
      ListStyleType::DisclosureOpen => ("\u{25be}".to_owned(), " "),
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
      "disclosure-closed" => Some(ListStyleType::DisclosureClosed),
      "disclosure-open" => Some(ListStyleType::DisclosureOpen),
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

  const VALID_TOKENS: &'static [CssToken] = &LIST_STYLE_TOKENS;
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
      ListStyleType::DisclosureClosed => dest.write_str("disclosure-closed"),
      ListStyleType::DisclosureOpen => dest.write_str("disclosure-open"),
      ListStyleType::String(value) => serialize_string(value, dest),
    }
  }
}

const LIST_STYLE_TOKEN_LISTS: &[&[CssToken]] = &[
  LIST_STYLE_TYPE_TOKENS,
  ListStylePosition::VALID_TOKENS,
  BackgroundImage::VALID_TOKENS,
];

const LIST_STYLE_TOKENS: [CssToken; CssToken::merged_len(LIST_STYLE_TOKEN_LISTS)] =
  CssToken::merge_lists(LIST_STYLE_TOKEN_LISTS);

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

  /// Every character `marker_text` can generate across the predefined counter
  /// styles. Mirrored by `LIST_MARKER_CHARACTERS` in
  /// `takumi-helpers/src/fonts.ts`, which font subsetting feeds to callers.
  const MARKER_CHARACTERS: &str = "\u{2022}\u{25e6}\u{25a0}\u{25aa}\u{25b8}\u{25c2}\u{25be} 0123456789.-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

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
    assert_eq!(
      ListStyleType::from_css_str("disclosure-closed"),
      Ok(ListStyleType::DisclosureClosed)
    );
    assert_eq!(
      ListStyleType::from_css_str("disclosure-open"),
      Ok(ListStyleType::DisclosureOpen)
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
      ListStyleType::Decimal.marker_text(3, false).as_deref(),
      Some("3. ")
    );
    assert_eq!(
      ListStyleType::DecimalLeadingZero
        .marker_text(7, false)
        .as_deref(),
      Some("07. ")
    );
    assert_eq!(
      ListStyleType::LowerAlpha.marker_text(28, false).as_deref(),
      Some("ab. ")
    );
    assert_eq!(
      ListStyleType::UpperRoman
        .marker_text(1994, false)
        .as_deref(),
      Some("MCMXCIV. ")
    );
    assert_eq!(
      ListStyleType::DisclosureClosed
        .marker_text(1, false)
        .as_deref(),
      Some("\u{25b8} ")
    );
    assert_eq!(
      ListStyleType::DisclosureClosed
        .marker_text(1, true)
        .as_deref(),
      Some("\u{25c2} ")
    );
    assert_eq!(
      ListStyleType::DisclosureOpen
        .marker_text(1, false)
        .as_deref(),
      Some("\u{25be} ")
    );
    assert_eq!(
      ListStyleType::DisclosureOpen
        .marker_text(9, true)
        .as_deref(),
      Some("\u{25be} ")
    );
    assert_eq!(
      ListStyleType::Disc.marker_text(1, false).as_deref(),
      Some("\u{2022} ")
    );
    assert_eq!(ListStyleType::None.marker_text(1, false), None);
  }

  #[test]
  fn a_negative_value_spends_its_padding_on_the_sign() {
    assert_eq!(
      ListStyleType::DecimalLeadingZero
        .marker_text(-7, false)
        .as_deref(),
      Some("-7. ")
    );
  }

  /// Font subsetting loads faces by the characters `MARKER_CHARACTERS`
  /// promises, so every counter style's output has to stay inside it. The
  /// match is exhaustive on purpose: a new style fails to compile here until
  /// it is accounted for.
  #[test]
  fn the_character_set_covers_every_counter_style() {
    let styles = [
      ListStyleType::None,
      ListStyleType::Disc,
      ListStyleType::Circle,
      ListStyleType::Square,
      ListStyleType::Decimal,
      ListStyleType::DecimalLeadingZero,
      ListStyleType::LowerAlpha,
      ListStyleType::UpperAlpha,
      ListStyleType::LowerRoman,
      ListStyleType::UpperRoman,
      ListStyleType::DisclosureClosed,
      ListStyleType::DisclosureOpen,
      ListStyleType::String("marker".into()),
    ];

    for style in styles {
      // A `String` marker carries its own text; every other style draws from
      // the shared character set.
      let covered: &[i32] = match style {
        ListStyleType::None | ListStyleType::String(_) => &[],
        ListStyleType::Disc
        | ListStyleType::Circle
        | ListStyleType::Square
        | ListStyleType::DisclosureClosed
        | ListStyleType::DisclosureOpen => &[1, 100],
        ListStyleType::Decimal
        | ListStyleType::DecimalLeadingZero
        | ListStyleType::LowerAlpha
        | ListStyleType::UpperAlpha
        | ListStyleType::LowerRoman
        | ListStyleType::UpperRoman => &[i32::MIN, -7, 0, 1, 9, 26, 27, 3999, 4000, i32::MAX],
      };

      for ordinal in covered {
        for is_rtl in [false, true] {
          let marker = style.marker_text(*ordinal, is_rtl).expect("marker text");

          for character in marker.chars() {
            assert!(
              MARKER_CHARACTERS.contains(character),
              "{style:?} at {ordinal} generates {character:?} outside MARKER_CHARACTERS"
            );
          }
        }
      }
    }
  }

  #[test]
  fn falls_back_to_decimal_outside_the_counter_range() {
    assert_eq!(
      ListStyleType::LowerAlpha.marker_text(0, false).as_deref(),
      Some("0. ")
    );
    assert_eq!(
      ListStyleType::LowerRoman.marker_text(-2, false).as_deref(),
      Some("-2. ")
    );
  }
}
