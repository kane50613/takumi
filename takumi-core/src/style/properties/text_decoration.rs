use std::{fmt, sync::Arc};

use bitflags::bitflags;
use cssparser::{Parser, Token};
use typed_builder::TypedBuilder;

use crate::style::{
  Animatable, BoxDecorationBreak, Color, ColorInput, ComputedStyle, Float, FromCss, FromCssStr,
  Length, MakeComputed, ParseResult, SizingContext, ToCss, discrete, impl_css_enum,
  tw::TailwindPropertyParser, unexpected_token, write_keywords,
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

impl TextDecorationLines {
  const KEYWORDS: [(Self, &'static str); 3] = [
    (Self::UNDERLINE, "underline"),
    (Self::LINE_THROUGH, "line-through"),
    (Self::OVERLINE, "overline"),
  ];

  fn from_keyword(ident: &str) -> Option<Self> {
    Self::KEYWORDS
      .into_iter()
      .find(|(_, keyword)| ident.eq_ignore_ascii_case(keyword))
      .map(|(line, _)| line)
  }
}

impl<'i> FromCss<'i> for TextDecorationLines {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let first_ident = input.expect_ident()?;

    if first_ident.eq_ignore_ascii_case("none") {
      return Ok(Self::empty());
    }

    let Some(mut lines) = Self::from_keyword(first_ident) else {
      return Err(unexpected_token!(&Token::Ident(first_ident.clone())));
    };

    while let Ok(line) = input.try_parse(|input| {
      let ident = input.expect_ident()?;

      Self::from_keyword(ident).ok_or_else(|| unexpected_token!(&Token::Ident(ident.clone())))
    }) {
      lines |= line;
    }

    Ok(lines)
  }

  const VALID_TOKENS: &'static [&'static str] = &["underline", "line-through", "overline"];
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
        TextDecorationThickness::Length(Length::interpolated(
          &from,
          &to,
          progress,
          sizing,
          current_color,
        ))
      }
      _ => discrete(from, to, progress),
    };
  }
}

/// `text-decoration-inset`: how far each end of a decoration line pulls in from its text.
/// <https://drafts.csswg.org/css-text-decor-4/#text-decoration-inset-property>
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDecorationInset {
  /// A small inset from the line's thickness that keeps adjacent lines apart.
  Auto,
  /// The insets of the start and the end.
  Edges(Length, Length),
}

impl Default for TextDecorationInset {
  fn default() -> Self {
    Self::Edges(Length::zero(), Length::zero())
  }
}

impl MakeComputed for TextDecorationInset {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Edges(start, end) = self {
      start.make_computed(sizing);
      end.make_computed(sizing);
    }
  }
}

impl Animatable for TextDecorationInset {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (*from, *to) {
      (Self::Edges(from_start, from_end), Self::Edges(to_start, to_end)) => Self::Edges(
        Length::interpolated(&from_start, &to_start, progress, sizing, current_color),
        Length::interpolated(&from_end, &to_end, progress, sizing, current_color),
      ),
      _ => discrete(from, to, progress),
    };
  }
}

impl<'i> FromCss<'i> for TextDecorationInset {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Self::Auto);
    }

    let start = Length::from_css(input)?;
    let end = input.try_parse(Length::from_css).unwrap_or(start);

    if matches!(start, Length::Auto) || matches!(end, Length::Auto) {
      return Err(input.new_error_for_next_token());
    }

    Ok(Self::Edges(start, end))
  }

  const VALID_TOKENS: &'static [&'static str] = &["auto", "<length>", "<percentage>"];
}

impl ToCss for TextDecorationInset {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto => dest.write_str("auto"),
      Self::Edges(start, end) => {
        start.to_css(dest)?;

        if end != start {
          dest.write_char(' ')?;
          end.to_css(dest)?;
        }

        Ok(())
      }
    }
  }
}

/// A `text-decoration-inset` resolved for rendering: each edge a fixed length plus a fraction of
/// the length of the decorated run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizedTextDecorationInset {
  /// A small inset from the line's thickness.
  Auto,
  /// The start and end insets.
  Edges(SizedInsetLength, SizedInsetLength),
}

/// A `<length-percentage>` resolved but for its percentage, which a run's length settles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizedInsetLength {
  fixed: f32,
  fraction: f32,
}

impl SizedInsetLength {
  /// `length` resolved against `sizing`, its percentage kept.
  fn new(length: Length, sizing: &SizingContext) -> Self {
    let fixed = length.to_px(sizing, 0.0);

    Self {
      fixed,
      fraction: length.to_px(sizing, 1.0) - fixed,
    }
  }

  /// The length against a run `reference` long.
  pub(crate) fn resolve(self, reference: f32) -> f32 {
    self.fixed + self.fraction * reference
  }

  /// Whether it is zero whatever the run.
  pub(crate) fn is_zero(self) -> bool {
    self.fixed == 0.0 && self.fraction == 0.0
  }
}

impl SizedTextDecorationInset {
  /// Blink's `TextDecorationInset::ResolveAutoInset`: half the thickness, from one to two pixels.
  pub(crate) fn auto_inset(thickness: f32) -> f32 {
    (thickness / 2.0).clamp(1.0, 2.0)
  }

  /// Whether it pulls no end in.
  pub(crate) fn is_zero(self) -> bool {
    matches!(self, Self::Edges(start, end) if start.is_zero() && end.is_zero())
  }
}

impl TextDecorationInset {
  /// The inset resolved against `sizing`.
  pub(crate) fn sized(self, sizing: &SizingContext) -> SizedTextDecorationInset {
    match self {
      Self::Auto => SizedTextDecorationInset::Auto,
      Self::Edges(start, end) => SizedTextDecorationInset::Edges(
        SizedInsetLength::new(start, sizing),
        SizedInsetLength::new(end, sizing),
      ),
    }
  }
}

/// Decoration thickness resolved for rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizedTextDecorationThickness {
  /// A tenth of the font size.
  Auto,
  /// Use the font's own thickness.
  FromFont,
  /// A thickness in pixels.
  Value(f32),
}

/// A decoration a decorating box applies to the text inside it, as Blink's
/// `AppliedTextDecoration`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppliedTextDecoration {
  /// Which lines it draws.
  pub line: TextDecorationLines,
  /// How it draws them.
  pub style: TextDecorationStyle,
  /// Their color.
  pub color: Color,
  /// Their thickness.
  pub thickness: SizedTextDecorationThickness,
  /// Extra offset of the underline away from the text, in pixels, or `None` for `auto`.
  pub underline_offset: Option<f32>,
  /// Which baseline the underline is measured from.
  pub underline_position: TextUnderlinePosition,
  /// How far each end of its lines pulls in.
  pub inset: SizedTextDecorationInset,
  /// Whether its insets apply to each fragment of the decorating box or to the box as a whole.
  pub box_decoration_break: BoxDecorationBreak,
}

/// The decorations a box's text paints, outermost decorating box first, as Blink's
/// `AppliedTextDecorations`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppliedTextDecorations(Option<Arc<[AppliedTextDecoration]>>);

impl AppliedTextDecorations {
  /// These decorations propagated into a child box, as Blink's `StyleAdjuster` does.
  pub(crate) fn for_child(
    &self,
    parent: &ComputedStyle,
    style: &ComputedStyle,
    sizing: &SizingContext,
    current_color: Color,
  ) -> Self {
    let display = if parent.base_data.display.should_blockify_children() {
      style.base_data.display.as_blockified()
    } else {
      style.base_data.display
    };
    let stops = style.base_data.float != Float::None
      || style.base_data.position.is_out_of_flow()
      || (display.is_inline_level() && !display.is_inline());
    let inherited = if stops { Self::default() } else { self.clone() };
    let Some(line) = style
      .visual_data
      .text_decoration_line
      .filter(|line| !line.is_empty())
    else {
      return inherited;
    };
    let own = AppliedTextDecoration {
      line,
      style: style.misc_data.text_decoration_style,
      color: style
        .misc3_data
        .text_decoration_color
        .resolve(current_color),
      thickness: style.resolved_text_decoration_thickness(sizing),
      underline_offset: style
        .misc_inherited1_data
        .text_underline_offset
        .resolve_px(sizing),
      underline_position: style.misc_inherited_data.text_underline_position,
      inset: style.misc3_data.text_decoration_inset.sized(sizing),
      box_decoration_break: style.box_data.box_decoration_break,
    };

    inherited.as_slice().iter().copied().chain([own]).collect()
  }

  /// The decorations, outermost first.
  pub fn as_slice(&self) -> &[AppliedTextDecoration] {
    self.0.as_deref().unwrap_or_default()
  }

  /// Whether no decoration applies.
  pub fn is_empty(&self) -> bool {
    self.0.is_none()
  }
}

impl FromIterator<AppliedTextDecoration> for AppliedTextDecorations {
  fn from_iter<I: IntoIterator<Item = AppliedTextDecoration>>(iter: I) -> Self {
    let decorations: Arc<[AppliedTextDecoration]> = iter.into_iter().collect();

    Self((!decorations.is_empty()).then_some(decorations))
  }
}

impl<'i> FromCss<'i> for TextDecorationThickness {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("from-font"))
      .is_ok()
    {
      return Ok(Self::FromFont);
    }

    Ok(Self::Length(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [&'static str] = &["from-font", "<length>", "<percentage>"];
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
    write_keywords(
      dest,
      Self::KEYWORDS
        .into_iter()
        .filter(|(line, _)| self.contains(*line))
        .map(|(_, keyword)| keyword),
      "none",
    )
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
  /// Resolves the offset to pixels, or `None` for `auto`.
  pub(crate) fn resolve_px(&self, sizing: &SizingContext) -> Option<f32> {
    match self {
      Self::Auto => None,
      Self::Length(length) => Some(length.to_px(sizing, sizing.font_size)),
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
        TextUnderlineOffset::Length(Length::interpolated(
          &from,
          &to,
          progress,
          sizing,
          current_color,
        ))
      }
      _ => discrete(from, to, progress),
    };
  }
}

impl<'i> FromCss<'i> for TextUnderlineOffset {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Self::Auto);
    }

    Ok(Self::Length(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [&'static str] = &["auto", "<length>", "<percentage>"];
}

impl ToCss for TextUnderlineOffset {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto => dest.write_str("auto"),
      Self::Length(length) => length.to_css(dest),
    }
  }
}

/// The `text-underline-position` property, without the vertical `left` and `right`.
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

impl_css_enum!(
  TextUnderlinePosition,
  "auto" => TextUnderlinePosition::Auto,
  "from-font" => TextUnderlinePosition::FromFont,
  "under" => TextUnderlinePosition::Under
);

/// How a text decoration line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum TextDecorationStyle {
  /// One solid line.
  #[default]
  Solid,
  /// Two solid lines.
  Double,
  /// A line of dots.
  Dotted,
  /// A line of dashes.
  Dashed,
  /// A wavy line.
  Wavy,
}

impl_css_enum!(
  TextDecorationStyle,
  "solid" => TextDecorationStyle::Solid,
  "double" => TextDecorationStyle::Double,
  "dotted" => TextDecorationStyle::Dotted,
  "dashed" => TextDecorationStyle::Dashed,
  "wavy" => TextDecorationStyle::Wavy
);

/// Parsed `text-decoration` value.
#[derive(Debug, Default, Clone, PartialEq, TypedBuilder)]
#[builder(field_defaults(default))]
#[non_exhaustive]
pub struct TextDecoration {
  /// Text decoration line style.
  pub line: TextDecorationLines,
  /// Text decoration style.
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
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
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

      return Err(unexpected_token!(input.next()?,));
    }

    Ok(TextDecoration {
      line,
      style: style.unwrap_or_default(),
      color: color.unwrap_or_default(),
      thickness: thickness.unwrap_or_default(),
    })
  }

  const VALID_TOKENS: &'static [&'static str] = &[
    "none",
    "underline",
    "line-through",
    "overline",
    "solid",
    "double",
    "dotted",
    "dashed",
    "wavy",
    "from-font",
    "<color>",
    "<length>",
    "<percentage>",
  ];
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::properties::Color;

  /// `text-decoration-inset` takes `auto` or one or two lengths, a lone length for both ends.
  #[test]
  fn text_decoration_inset_parses_one_or_two_lengths() {
    assert_eq!(
      TextDecorationInset::from_css_str("auto"),
      Ok(TextDecorationInset::Auto)
    );
    assert_eq!(
      TextDecorationInset::from_css_str("10px"),
      Ok(TextDecorationInset::Edges(
        Length::Px(10.0),
        Length::Px(10.0)
      ))
    );
    assert_eq!(
      TextDecorationInset::from_css_str("10% -2px"),
      Ok(TextDecorationInset::Edges(
        Length::Percentage(10.0),
        Length::Px(-2.0)
      ))
    );
    assert!(TextDecorationInset::from_css_str("10px auto").is_err());
  }

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
  fn a_thickness_right_after_the_line_keeps_its_value() {
    assert_eq!(
      TextDecoration::from_css_str("overline 12px red"),
      Ok(
        TextDecoration::builder()
          .line(TextDecorationLines::OVERLINE)
          .color(ColorInput::Value(Color([255, 0, 0, 255])))
          .thickness(TextDecorationThickness::Length(Length::Px(12.0)))
          .build()
      )
    );
  }

  #[test]
  fn test_parse_text_decoration_styles() {
    for (css, style) in [
      ("underline double", TextDecorationStyle::Double),
      ("underline dotted", TextDecorationStyle::Dotted),
      ("underline dashed", TextDecorationStyle::Dashed),
      ("underline wavy", TextDecorationStyle::Wavy),
    ] {
      assert_eq!(
        TextDecoration::from_css_str(css),
        Ok(
          TextDecoration::builder()
            .line(TextDecorationLines::UNDERLINE)
            .style(style)
            .build()
        )
      );
    }
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
