use std::fmt;

use cssparser::Parser;

use crate::style::{tw::TailwindPropertyParser, *};

/// Keyword values for the CSS `vertical-align` property.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum VerticalAlignKeyword {
  /// Aligns the baseline of the box with the baseline of the parent box.
  #[default]
  Baseline,
  /// Aligns the top of the box with the top of the line box.
  Top,
  /// Aligns the middle of the box with the baseline of the parent box plus half the x-height of the parent.
  Middle,
  /// Aligns the bottom of the box with the bottom of the line box.
  Bottom,
  /// Aligns the top of the box with the top of the parent's font.
  TextTop,
  /// Aligns the bottom of the box with the bottom of the parent's font.
  TextBottom,
  /// Aligns the baseline of the box with the subscript-baseline of the parent box.
  Sub,
  /// Aligns the baseline of the box with the superscript-baseline of the parent box.
  Super,
}

impl_css_enum!(
  VerticalAlignKeyword,
  "baseline" => VerticalAlignKeyword::Baseline,
  "top" => VerticalAlignKeyword::Top,
  "middle" => VerticalAlignKeyword::Middle,
  "bottom" => VerticalAlignKeyword::Bottom,
  "text-top" => VerticalAlignKeyword::TextTop,
  "text-bottom" => VerticalAlignKeyword::TextBottom,
  "sub" => VerticalAlignKeyword::Sub,
  "super" => VerticalAlignKeyword::Super
);

/// Defines the vertical alignment of an inline-level box.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum VerticalAlign {
  /// A keyword-based alignment mode.
  Keyword(VerticalAlignKeyword),
  /// A baseline shift in `<length-percentage>` form.
  Length(Length),
}

impl ToCss for VerticalAlign {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Keyword(kw) => kw.to_css(dest),
      Self::Length(l) => l.to_css(dest),
    }
  }
}

/// `vertical-align` with its length resolved, as the line box aligns a box by it.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ResolvedVerticalAlign {
  /// A keyword, which aligns against the parent box or the line box.
  Keyword(VerticalAlignKeyword),
  /// How far the baseline rises above the parent's, in pixels.
  Shift(f32),
}

impl Default for VerticalAlign {
  fn default() -> Self {
    Self::Keyword(VerticalAlignKeyword::default())
  }
}

impl Default for ResolvedVerticalAlign {
  fn default() -> Self {
    Self::Keyword(VerticalAlignKeyword::default())
  }
}

impl<'i> FromCss<'i> for VerticalAlign {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(keyword) = input.try_parse(VerticalAlignKeyword::from_css) {
      return Ok(Self::Keyword(keyword));
    }

    Ok(Self::Length(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("baseline"),
    CssToken::Keyword("top"),
    CssToken::Keyword("middle"),
    CssToken::Keyword("bottom"),
    CssToken::Keyword("text-top"),
    CssToken::Keyword("text-bottom"),
    CssToken::Keyword("sub"),
    CssToken::Keyword("super"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl VerticalAlign {
  /// Resolves a length against the box's own `line_height` in pixels, which a percentage refers
  /// to.
  pub fn resolve(self, sizing: &SizingContext, line_height: f32) -> ResolvedVerticalAlign {
    match self {
      Self::Keyword(keyword) => ResolvedVerticalAlign::Keyword(keyword),
      Self::Length(length) => ResolvedVerticalAlign::Shift(length.to_px(sizing, line_height)),
    }
  }
}

impl MakeComputed for VerticalAlign {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl TailwindPropertyParser for VerticalAlign {
  fn parse_tw(token: &str) -> Option<Self> {
    VerticalAlignKeyword::from_css_str(token)
      .ok()
      .map(Self::Keyword)
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{geometry::Size, viewport::Viewport};

  fn sizing() -> SizingContext {
    SizingContext {
      viewport: Viewport {
        size: (200, 100).into(),
        font_size: 16.0,
        device_pixel_ratio: 2.0,
        media_target: Default::default(),
        unit_reference: None,
      },
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: 10.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  #[test]
  fn parse_keywords_and_length_percentage() {
    assert_eq!(
      VerticalAlign::from_css_str("baseline"),
      Ok(VerticalAlign::Keyword(VerticalAlignKeyword::Baseline))
    );
    assert_eq!(
      VerticalAlign::from_css_str("10px"),
      Ok(VerticalAlign::Length(Length::Px(10.0)))
    );
    assert_eq!(
      VerticalAlign::from_css_str("25%"),
      Ok(VerticalAlign::Length(Length::Percentage(25.0)))
    );
    assert_eq!(
      VerticalAlign::from_css_str("-0.5em"),
      Ok(VerticalAlign::Length(Length::Em(-0.5)))
    );
  }

  #[test]
  fn resolve_length_to_shift_px() {
    let resolved = VerticalAlign::Length(Length::Px(8.0)).resolve(&sizing(), 18.0);

    assert_eq!(resolved, ResolvedVerticalAlign::Shift(16.0));
  }

  #[test]
  fn resolve_percentage_against_the_line_height() {
    let resolved = VerticalAlign::Length(Length::Percentage(50.0)).resolve(&sizing(), 18.0);

    assert_eq!(resolved, ResolvedVerticalAlign::Shift(9.0));
  }

  #[test]
  fn resolve_keeps_keywords_for_the_line_box() {
    for keyword in [VerticalAlignKeyword::Sub, VerticalAlignKeyword::Middle] {
      assert_eq!(
        VerticalAlign::Keyword(keyword).resolve(&sizing(), 18.0),
        ResolvedVerticalAlign::Keyword(keyword)
      );
    }
  }
}
