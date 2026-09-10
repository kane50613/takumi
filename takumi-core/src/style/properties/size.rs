use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};
use taffy::{CompactLength, Dimension};

use crate::style::{
  Animatable, Color, CssDescriptorKind, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed,
  ParseResult, SizingContext, ToCss,
  tw::{Namespace, TailwindPropertyParser},
  unexpected_token,
};

/// Represents a `width`/`height` value: a `<length-percentage>`, `auto`, or a
/// CSS Sizing Level 3 keyword.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum SizeValue {
  /// A `<length-percentage>` or `auto`.
  Length(Length),
  /// `min-content`: the largest minimum content contribution.
  MinContent,
  /// `max-content`: the size that takes no soft wrap opportunity.
  MaxContent,
  /// `fit-content`: `max(min-content, min(max-content, stretch))`.
  FitContent,
  /// `fit-content(<length-percentage>)`: `max(min-content, min(max-content, <length-percentage>))`.
  FitContentLimit(Length),
  /// `stretch`: the size the box takes when it fills the available space.
  Stretch,
}

impl Default for SizeValue {
  fn default() -> Self {
    Self::auto()
  }
}

impl SizeValue {
  /// An automatic size.
  pub const fn auto() -> Self {
    Self::Length(Length::Auto)
  }

  /// A zero length.
  pub const fn zero() -> Self {
    Self::Length(Length::zero())
  }

  /// Whether the value leaves the size to the layout algorithm.
  pub fn is_auto(self) -> bool {
    matches!(self, Self::Length(Length::Auto))
  }

  /// The length this value carries, if it is one.
  pub fn as_length(self) -> Option<Length> {
    match self {
      Self::Length(length) => Some(length),
      _ => None,
    }
  }

  /// Resolves to a taffy `Dimension`.
  pub(crate) fn resolve_to_dimension(self, sizing: &SizingContext) -> Dimension {
    match self {
      Self::Length(length) => length.resolve_to_dimension(sizing),
      Self::MinContent => Dimension::min_content(),
      Self::MaxContent => Dimension::max_content(),
      Self::FitContent => Dimension::fit_content(),
      Self::FitContentLimit(limit) => {
        let compact = limit.resolve_to_length_percentage(sizing).into_raw();

        match compact.tag() {
          CompactLength::PERCENT_TAG => Dimension::fit_content_percent(compact.value()),
          CompactLength::LENGTH_TAG => Dimension::fit_content_px(compact.value()),
          // taffy has no `calc()` limit, so a mixed one falls back to the
          // unlimited keyword.
          _ => Dimension::fit_content(),
        }
      }
      Self::Stretch => Dimension::stretch(),
    }
  }
}

impl From<Length> for SizeValue {
  fn from(length: Length) -> Self {
    Self::Length(length)
  }
}

impl MakeComputed for SizeValue {
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      Self::Length(length) | Self::FitContentLimit(length) => length.make_computed(sizing),
      _ => {}
    }
  }
}

impl Animatable for SizeValue {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (from, to) {
      (Self::Length(from), Self::Length(to)) => {
        let mut length = *from;
        length.interpolate(from, to, progress, sizing, current_color);
        Self::Length(length)
      }
      (Self::FitContentLimit(from), Self::FitContentLimit(to)) => {
        let mut length = *from;
        length.interpolate(from, to, progress, sizing, current_color);
        Self::FitContentLimit(length)
      }
      _ => {
        if progress >= 0.5 {
          *to
        } else {
          *from
        }
      }
    };
  }
}

impl<'i> FromCss<'i> for SizeValue {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Keyword("min-content"),
    CssToken::Keyword("max-content"),
    CssToken::Keyword("fit-content"),
    CssToken::Keyword("stretch"),
    CssToken::Descriptor(CssDescriptorKind::FitContentFn),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(keyword) = input.try_parse(parse_sizing_keyword) {
      return Ok(keyword);
    }

    Length::from_css(input).map(Self::Length)
  }
}

fn parse_sizing_keyword<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, SizeValue> {
  let location = input.current_source_location();
  let token = input.next()?.clone();

  match &token {
    Token::Ident(ident) => match_ignore_ascii_case! {ident,
      "min-content" => Ok(SizeValue::MinContent),
      "max-content" => Ok(SizeValue::MaxContent),
      "fit-content" => Ok(SizeValue::FitContent),
      "stretch" => Ok(SizeValue::Stretch),
      _ => Err(unexpected_token!(SizeValue, location, &token)),
    },
    Token::Function(name) if name.eq_ignore_ascii_case("fit-content") => {
      let limit = input.parse_nested_block(Length::from_css)?;

      if limit == Length::Auto {
        return Err(unexpected_token!(SizeValue, location, &token));
      }

      Ok(SizeValue::FitContentLimit(limit))
    }
    _ => Err(unexpected_token!(SizeValue, location, &token)),
  }
}

impl ToCss for SizeValue {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Length(length) => length.to_css(dest),
      Self::MinContent => dest.write_str("min-content"),
      Self::MaxContent => dest.write_str("max-content"),
      Self::FitContent => dest.write_str("fit-content"),
      Self::FitContentLimit(limit) => {
        dest.write_str("fit-content(")?;
        limit.to_css(dest)?;
        dest.write_char(')')
      }
      Self::Stretch => dest.write_str("stretch"),
    }
  }
}

impl TailwindPropertyParser for SizeValue {
  const NAMESPACES: &'static [Namespace] = Length::NAMESPACES;

  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "min" => return Some(Self::MinContent),
      "max" => return Some(Self::MaxContent),
      "fit" => return Some(Self::FitContent),
      _ => {}
    }

    Length::parse_tw(token).map(Self::Length)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn parses_lengths_and_auto() {
    for (css, expected) in [
      ("auto", SizeValue::Length(Length::Auto)),
      ("100px", SizeValue::Length(Length::Px(100.0))),
      ("50%", SizeValue::Length(Length::Percentage(50.0))),
    ] {
      let parsed = SizeValue::from_css_str(css).unwrap();

      assert_eq!(parsed, expected, "{css}");
      assert_eq!(parsed.to_css_string(), css);
    }
  }

  #[test]
  fn parses_the_sizing_keywords() {
    for (css, expected) in [
      ("min-content", SizeValue::MinContent),
      ("max-content", SizeValue::MaxContent),
      ("fit-content", SizeValue::FitContent),
      ("stretch", SizeValue::Stretch),
      (
        "fit-content(20rem)",
        SizeValue::FitContentLimit(Length::Rem(20.0)),
      ),
      (
        "fit-content(50%)",
        SizeValue::FitContentLimit(Length::Percentage(50.0)),
      ),
    ] {
      let parsed = SizeValue::from_css_str(css).unwrap();

      assert_eq!(parsed, expected, "{css}");
      assert_eq!(parsed.to_css_string(), css);
    }

    assert!(SizeValue::from_css_str("fit-content(auto)").is_err());
    assert!(SizeValue::from_css_str("fit-content()").is_err());
    assert!(SizeValue::from_css_str("content").is_err());
  }

  #[test]
  fn resolves_the_sizing_keywords_to_taffy() {
    let sizing = SizingContext::builder()
      .viewport(crate::viewport::Viewport::default())
      .build();

    assert_eq!(
      SizeValue::MinContent.resolve_to_dimension(&sizing),
      Dimension::min_content()
    );
    assert_eq!(
      SizeValue::MaxContent.resolve_to_dimension(&sizing),
      Dimension::max_content()
    );
    assert_eq!(
      SizeValue::Stretch.resolve_to_dimension(&sizing),
      Dimension::stretch()
    );
    assert_eq!(
      SizeValue::FitContentLimit(Length::Px(30.0)).resolve_to_dimension(&sizing),
      Dimension::fit_content_px(30.0)
    );
    assert_eq!(
      SizeValue::FitContentLimit(Length::Percentage(40.0)).resolve_to_dimension(&sizing),
      Dimension::fit_content_percent(0.4)
    );
  }

  #[test]
  fn parses_the_tailwind_keywords() {
    assert_eq!(SizeValue::parse_tw("min"), Some(SizeValue::MinContent));
    assert_eq!(SizeValue::parse_tw("max"), Some(SizeValue::MaxContent));
    assert_eq!(SizeValue::parse_tw("fit"), Some(SizeValue::FitContent));
    assert_eq!(
      SizeValue::parse_tw("full"),
      Some(SizeValue::Length(Length::Percentage(100.0)))
    );
  }
}
