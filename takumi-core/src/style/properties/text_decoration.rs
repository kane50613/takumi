use std::fmt;

use bitflags::bitflags;
use cssparser::{Parser, Token, match_ignore_ascii_case};
use typed_builder::TypedBuilder;

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, FromCssStr, Length, MakeComputed,
  ParseResult, SizingContext, ToCss, declare_enum_from_css_impl, properties::ColorInput,
  tw::TailwindPropertyParser, unexpected_token,
};

bitflags! {
  /// Represents a collection of text decoration lines.
  #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
  #[non_exhaustive]
  pub struct TextDecorationLines: u8 {
    /// Underline text decoration.
    const UNDERLINE = 0b001;
    /// Line-through text decoration.
    const LINE_THROUGH = 0b010;
    /// Overline text decoration.
    const OVERLINE = 0b100;
  }
}

impl<'i> FromCss<'i> for TextDecorationLines {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut lines = TextDecorationLines::empty();

    // Parse at least one line decoration
    let first_location = input.current_source_location();
    let first_ident = input.expect_ident()?;
    match_ignore_ascii_case! {first_ident,
      "none" => return Ok(lines),
      "underline" => lines |= TextDecorationLines::UNDERLINE,
      "line-through" => lines |= TextDecorationLines::LINE_THROUGH,
      "overline" => lines |= TextDecorationLines::OVERLINE,
      _ => return Err(unexpected_token!(first_location, &Token::Ident(first_ident.clone()))),
    }

    // Parse additional decorations if present
    while !input.is_exhausted() {
      let state = input.state();
      if let Ok(ident) = input.expect_ident() {
        match_ignore_ascii_case! {ident,
          "underline" => lines |= TextDecorationLines::UNDERLINE,
          "line-through" => lines |= TextDecorationLines::LINE_THROUGH,
          "overline" => lines |= TextDecorationLines::OVERLINE,
          _ => {
            input.reset(&state);
            break;
          }
        }
      } else {
        break;
      }
    }

    Ok(lines)
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("underline"),
    CssToken::Keyword("line-through"),
    CssToken::Keyword("overline"),
  ];
}

impl MakeComputed for TextDecorationLines {}

/// Represents text decoration thickness options.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum TextDecorationThickness {
  /// Use the font's default thickness, fallback to `auto` if not available.
  FromFont,
  /// Use a specific length.
  Length(Length),
}

impl Default for TextDecorationThickness {
  fn default() -> Self {
    Self::Length(Length::Auto)
  }
}

impl MakeComputed for TextDecorationThickness {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for TextDecorationThickness {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (*from, *to) {
      (TextDecorationThickness::Length(from), TextDecorationThickness::Length(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        TextDecorationThickness::Length(value)
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

/// Decoration thickness resolved for rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizedTextDecorationThickness {
  /// Use the font's own thickness.
  FromFont,
  /// A thickness in pixels.
  Value(f32),
}

impl<'i> FromCss<'i> for TextDecorationThickness {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("from-font"))
      .is_ok()
    {
      return Ok(Self::FromFont);
    }

    Ok(Self::Length(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("from-font"),
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Syntax(CssSyntaxKind::Percentage),
  ];
}

impl TailwindPropertyParser for TextDecorationThickness {
  fn parse_tw(token: &str) -> Option<Self> {
    if let Ok(number) = token.parse::<f32>() {
      return Some(Self::Length(Length::Px(number)));
    }

    Self::from_css_str(token).ok()
  }
}

impl ToCss for TextDecorationLines {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.is_empty() {
      return dest.write_str("none");
    }
    let mut first = true;
    if self.contains(TextDecorationLines::UNDERLINE) {
      dest.write_str("underline")?;
      first = false;
    }
    if self.contains(TextDecorationLines::LINE_THROUGH) {
      if !first {
        dest.write_char(' ')?;
      }
      dest.write_str("line-through")?;
      first = false;
    }
    if self.contains(TextDecorationLines::OVERLINE) {
      if !first {
        dest.write_char(' ')?;
      }
      dest.write_str("overline")?;
    }
    Ok(())
  }
}

impl ToCss for TextDecorationThickness {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::FromFont => dest.write_str("from-font"),
      Self::Length(l) => l.to_css(dest),
    }
  }
}

/// Represents the `text-underline-offset` value, shifting the underline away from
/// the text. Positive lengths move it further from the text.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum TextUnderlineOffset {
  /// Use the font's default underline position.
  #[default]
  Auto,
  /// Offset by a specific length; percentages resolve against `1em`.
  Length(Length),
}

impl TextUnderlineOffset {
  /// Resolves the offset to pixels, with `auto` yielding `0`.
  pub(crate) fn resolve_px(&self, sizing: &SizingContext) -> f32 {
    match self {
      Self::Auto => 0.0,
      Self::Length(length) => length.to_px(sizing, sizing.font_size),
    }
  }
}

impl MakeComputed for TextUnderlineOffset {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for TextUnderlineOffset {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (*from, *to) {
      (TextUnderlineOffset::Length(from), TextUnderlineOffset::Length(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        TextUnderlineOffset::Length(value)
      }
      _ if progress >= 0.5 => *to,
      _ => *from,
    };
  }
}

impl<'i> FromCss<'i> for TextUnderlineOffset {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Self::Auto);
    }

    Ok(Self::Length(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Syntax(CssSyntaxKind::Percentage),
  ];
}

impl ToCss for TextUnderlineOffset {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto => dest.write_str("auto"),
      Self::Length(length) => length.to_css(dest),
    }
  }
}

/// Represents the `text-underline-position` value, choosing the baseline the underline
/// is measured from. The `left` and `right` keywords apply to vertical writing modes,
/// which are not supported.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum TextUnderlinePosition {
  /// Use the font's underline metrics.
  #[default]
  Auto,
  /// Use the font's underline metrics, falling back to `auto` when the font has none.
  FromFont,
  /// Place the underline below the text's descenders.
  Under,
}

declare_enum_from_css_impl!(
  TextUnderlinePosition,
  "auto" => TextUnderlinePosition::Auto,
  "from-font" => TextUnderlinePosition::FromFont,
  "under" => TextUnderlinePosition::Under
);

/// Represents text decoration style options (currently only solid is supported).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum TextDecorationStyle {
  /// Solid text decoration style.
  #[default]
  Solid,
}

declare_enum_from_css_impl!(
  TextDecorationStyle,
  "solid" => Self::Solid
);

/// Parsed `text-decoration` value.
#[derive(Debug, Default, Clone, PartialEq, TypedBuilder)]
#[builder(field_defaults(default))]
#[non_exhaustive]
pub struct TextDecoration {
  /// Text decoration line style.
  pub line: TextDecorationLines,
  /// Text decoration style (currently only solid is supported).
  pub style: TextDecorationStyle,
  /// Optional text decoration color.
  pub color: ColorInput,
  /// Optional text decoration thickness.
  pub thickness: TextDecorationThickness,
}

impl MakeComputed for TextDecoration {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.color.make_computed(sizing);
    self.thickness.make_computed(sizing);
  }
}

impl<'i> FromCss<'i> for TextDecoration {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut line = TextDecorationLines::empty();
    let mut style = None;
    let mut color = None;
    let mut thickness = None;

    loop {
      if let Ok(value) = input.try_parse(TextDecorationLines::from_css) {
        line |= value;
        continue;
      }

      if let Ok(value) = input.try_parse(TextDecorationStyle::from_css) {
        style = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(ColorInput::from_css) {
        color = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(TextDecorationThickness::from_css) {
        thickness = Some(value);
        continue;
      }

      if input.is_exhausted() {
        break;
      }

      return Err(unexpected_token!(
        input.current_source_location(),
        input.next()?,
      ));
    }

    Ok(TextDecoration {
      line,
      style: style.unwrap_or_default(),
      color: color.unwrap_or_default(),
      thickness: thickness.unwrap_or_default(),
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("underline"),
    CssToken::Keyword("line-through"),
    CssToken::Keyword("overline"),
    CssToken::Keyword("solid"),
    CssToken::Keyword("from-font"),
    CssToken::Syntax(CssSyntaxKind::Color),
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Syntax(CssSyntaxKind::Percentage),
  ];
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::properties::Color;

  #[test]
  fn test_parse_text_decoration_none() {
    assert_eq!(
      TextDecoration::from_css_str("none"),
      Ok(TextDecoration::builder().build())
    );
    assert_eq!(
      TextDecorationLines::from_css_str("none"),
      Ok(TextDecorationLines::empty())
    );
  }

  #[test]
  fn test_parse_text_decoration_underline() {
    assert_eq!(
      TextDecoration::from_css_str("underline"),
      Ok(
        TextDecoration::builder()
          .line(TextDecorationLines::UNDERLINE)
          .build()
      )
    );
  }

  #[test]
  fn test_parse_text_decoration_line_through() {
    assert_eq!(
      TextDecoration::from_css_str("line-through"),
      Ok(
        TextDecoration::builder()
          .line(TextDecorationLines::LINE_THROUGH)
          .build()
      )
    );
  }

  #[test]
  fn test_parse_text_decoration_underline_solid() {
    assert_eq!(
      TextDecoration::from_css_str("underline solid"),
      Ok(
        TextDecoration::builder()
          .line(TextDecorationLines::UNDERLINE)
          .style(TextDecorationStyle::Solid)
          .build()
      )
    );
  }

  #[test]
  fn test_parse_text_decoration_line_through_solid_red() {
    assert_eq!(
      TextDecoration::from_css_str("line-through solid red"),
      Ok(
        TextDecoration::builder()
          .line(TextDecorationLines::LINE_THROUGH)
          .style(TextDecorationStyle::Solid)
          .color(ColorInput::Value(Color([255, 0, 0, 255])))
          .build()
      )
    );
  }

  #[test]
  fn test_parse_text_decoration_multiple_lines() {
    assert_eq!(
      TextDecoration::from_css_str("underline line-through solid red"),
      Ok(
        TextDecoration::builder()
          .line(TextDecorationLines::UNDERLINE | TextDecorationLines::LINE_THROUGH)
          .style(TextDecorationStyle::Solid)
          .color(ColorInput::Value(Color([255, 0, 0, 255])))
          .build()
      )
    );
  }

  #[test]
  fn test_parse_text_decoration_invalid() {
    let result = TextDecoration::from_css_str("invalid");
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_text_underline_offset_auto() {
    assert_eq!(
      TextUnderlineOffset::from_css_str("auto"),
      Ok(TextUnderlineOffset::Auto)
    );
  }

  #[test]
  fn test_parse_text_underline_offset_length() {
    assert_eq!(
      TextUnderlineOffset::from_css_str("3px"),
      Ok(TextUnderlineOffset::Length(Length::Px(3.0)))
    );
  }

  #[test]
  fn test_parse_text_underline_offset_invalid() {
    assert!(TextUnderlineOffset::from_css_str("solid").is_err());
  }

  #[test]
  fn test_parse_text_underline_position() {
    assert_eq!(
      TextUnderlinePosition::from_css_str("auto"),
      Ok(TextUnderlinePosition::Auto)
    );
    assert_eq!(
      TextUnderlinePosition::from_css_str("from-font"),
      Ok(TextUnderlinePosition::FromFont)
    );
    assert_eq!(
      TextUnderlinePosition::from_css_str("under"),
      Ok(TextUnderlinePosition::Under)
    );
  }

  #[test]
  fn test_parse_text_underline_position_invalid() {
    assert!(TextUnderlinePosition::from_css_str("left").is_err());
  }
}
