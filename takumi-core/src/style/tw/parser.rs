use std::{ops::Neg, sync::Arc};

use cssparser::{Parser, match_ignore_ascii_case};

use crate::style::{
  CssToken,
  Length::{self, *},
  tw::TailwindPropertyParser,
  *,
};

/// Tailwind `text-*` font size with optional paired line height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TwFontSize {
  /// Font size.
  pub font_size: FontSize,
  /// Paired line height, if any.
  pub line_height: Option<LineHeight>,
}

impl<'i> FromCss<'i> for TwFontSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self::new(FontSize::from_css(input)?, None))
  }

  const VALID_TOKENS: &'static [CssToken] = FontSize::VALID_TOKENS;
}

impl TailwindPropertyParser for TwFontSize {
  const NAMESPACES: &'static [Namespace] = &[Namespace::Text];

  fn parse_tw(token: &str) -> Option<Self> {
    // text-xs/6 => font-size: 0.75rem, line-height: 1.5em
    if let Some((font_size, line_height)) = token.split_once('/') {
      return Some(Self::new(
        TwFontSize::parse_tw(font_size)?.font_size,
        Some(LineHeight::parse_tw_with_arbitrary(line_height)?),
      ));
    }

    match_ignore_ascii_case! {token,
      "xs" => Some(
        Self::new(Rem(0.75).into(), Some(Em(1.0 / 0.75).into())),
      ),
      "sm" => Some(
        Self::new(Rem(0.875).into(), Some(Em(1.25 / 0.875).into())),
      ),
      "base" => Some(
        Self::new(Rem(1.0).into(), Some(Em(1.5 / 1.0).into())),
      ),
      "lg" => Some(
        Self::new(Rem(1.125).into(), Some(Em(1.75 / 1.125).into())),
      ),
      "xl" => Some(
        Self::new(Rem(1.25).into(), Some(Em(1.75 / 1.25).into())),
      ),
      "2xl" => Some(
        Self::new(Rem(1.5).into(), Some(Em(2.0 / 1.5).into())),
      ),
      "3xl" => Some(
        Self::new(Rem(1.875).into(), Some(Em(2.25 / 1.875).into())),
      ),
      "4xl" => Some(
        Self::new(Rem(2.25).into(), Some(Em(2.5 / 2.25).into())),
      ),
      "5xl" => Some(
        Self::new(Rem(3.0).into(), Some(LineHeight::Unitless(1.0))),
      ),
      "6xl" => Some(
        Self::new(Rem(3.75).into(), Some(LineHeight::Unitless(1.0))),
      ),
      "7xl" => Some(
        Self::new(Rem(4.5).into(), Some(LineHeight::Unitless(1.0))),
      ),
      "8xl" => Some(
        Self::new(Rem(6.0).into(), Some(LineHeight::Unitless(1.0))),
      ),
      "9xl" => Some(
        Self::new(Rem(8.0).into(), Some(LineHeight::Unitless(1.0))),
      ),
      _ => None,
    }
  }
}

impl TwFontSize {
  /// Builds a font size with an optional line height.
  pub const fn new(font_size: FontSize, line_height: Option<LineHeight>) -> Self {
    Self {
      font_size,
      line_height,
    }
  }
}

/// Tailwind `grid-cols-*` / `grid-rows-*` track template.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TwGridTemplate(
  /// Resolved track components.
  pub GridTemplateComponents,
);

impl<'i> FromCss<'i> for TwGridTemplate {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(GridTemplateComponents::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = GridTemplateComponents::VALID_TOKENS;
}

impl TailwindPropertyParser for TwGridTemplate {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("none") {
      return Some(TwGridTemplate(GridTemplateComponents::default()));
    }

    let count = token.parse::<u32>().ok()?;
    if count == 0 {
      return None;
    }

    // Create repeat(count, minmax(0, 1fr))
    let track_sizes = vec![
      GridTrackSize::MinMax(GridMinMaxSize {
        min: GridLength::Unit(Length::Px(0.0)),
        max: GridLength::Fr(1.0),
      });
      count as usize
    ];

    let template_components = track_sizes
      .into_iter()
      .map(GridTemplateComponent::Single)
      .collect();

    Some(TwGridTemplate(template_components))
  }
}

/// Tailwind `tracking-*` letter spacing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TwLetterSpacing(
  /// Letter spacing length.
  pub Length,
);

impl<'i> FromCss<'i> for TwLetterSpacing {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = Length::VALID_TOKENS;
}

impl Neg for TwLetterSpacing {
  type Output = Self;

  fn neg(self) -> Self::Output {
    TwLetterSpacing(-self.0)
  }
}

impl TailwindPropertyParser for TwLetterSpacing {
  const NAMESPACES: &'static [Namespace] = &[Namespace::Tracking];

  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "tighter" => Some(TwLetterSpacing(Length::Em(-0.05))),
      "tight" => Some(TwLetterSpacing(Length::Em(-0.025))),
      "normal" => Some(TwLetterSpacing(Length::Em(0.0))),
      "wide" => Some(TwLetterSpacing(Length::Em(0.025))),
      "wider" => Some(TwLetterSpacing(Length::Em(0.05))),
      "widest" => Some(TwLetterSpacing(Length::Em(0.1))),
      _ => None,
    }
  }
}

/// Tailwind `rounded-*` corner radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TwRounded(
  /// Corner radius.
  pub Length,
);

impl<'i> FromCss<'i> for TwRounded {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(TwRounded(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = Length::VALID_TOKENS;
}

impl TailwindPropertyParser for TwRounded {
  const NAMESPACES: &'static [Namespace] = &[Namespace::Radius];

  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "full" => Some(TwRounded(Length::Px(9999.0))),
      "none" => Some(TwRounded(Length::Px(0.0))),
      "xs" => Some(TwRounded(Length::Rem(0.125))),
      "sm" => Some(TwRounded(Length::Rem(0.25))),
      "md" => Some(TwRounded(Length::Rem(0.375))),
      "lg" => Some(TwRounded(Length::Rem(0.5))),
      "xl" => Some(TwRounded(Length::Rem(0.75))),
      "2xl" => Some(TwRounded(Length::Rem(1.0))),
      "3xl" => Some(TwRounded(Length::Rem(1.5))),
      "4xl" => Some(TwRounded(Length::Rem(2.0))),
      _ => None,
    }
  }
}

/// `<length-percentage>` for `from-N%` / `via-N%` / `to-N%` stop positions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TwGradientPosition(
  /// Stop position length-percentage.
  pub Length,
);

impl<'i> FromCss<'i> for TwGradientPosition {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let length = Length::from_css(input)?;
    if matches!(length, Length::Auto) {
      return Err(
        location
          .new_basic_unexpected_token_error(cssparser::Token::Ident("auto".into()))
          .into(),
      );
    }
    Ok(TwGradientPosition(length))
  }

  const VALID_TOKENS: &'static [CssToken] = Length::VALID_TOKENS;
}

impl TailwindPropertyParser for TwGradientPosition {
  fn parse_tw(token: &str) -> Option<Self> {
    let stripped = token.strip_suffix('%')?;
    let value = stripped.parse::<f32>().ok()?;
    Some(TwGradientPosition(Length::Percentage(value)))
  }
}

/// Tailwind `blur-*` radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TwBlur(
  /// Blur radius.
  pub Length,
);

impl<'i> FromCss<'i> for TwBlur {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(TwBlur(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = Length::VALID_TOKENS;
}

impl TailwindPropertyParser for TwBlur {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "none" => Some(TwBlur(Length::Px(0.0))),
      "xs" => Some(TwBlur(Length::Px(4.0))),
      "sm" => Some(TwBlur(Length::Px(8.0))),
      "md" => Some(TwBlur(Length::Px(12.0))),
      "lg" => Some(TwBlur(Length::Px(16.0))),
      "xl" => Some(TwBlur(Length::Px(24.0))),
      "2xl" => Some(TwBlur(Length::Px(40.0))),
      "3xl" => Some(TwBlur(Length::Px(64.0))),
      _ => None,
    }
  }
}

/// A variable-backed colour expression for gradient stops and shadow colours:
/// `var(--color-*)`, with the built-in colour as the fallback when the token
/// names one. `current` and `transparent` stay plain keywords.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TwVarColor(pub Arc<str>);

impl<'i> FromCss<'i> for TwVarColor {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let color = ColorInput::from_css(input)?;
    let mut css = String::new();

    let _ = color.to_css(&mut css);
    Ok(Self(css.into()))
  }

  const VALID_TOKENS: &'static [CssToken] = ColorInput::VALID_TOKENS;
}

impl TailwindPropertyParser for TwVarColor {
  fn parse_tw(token: &str) -> Option<Self> {
    let (base, modifier) = match token.split_once('/') {
      Some((base, modifier)) => (base, Some(modifier)),
      None => (token, None),
    };

    let percentage = match modifier {
      Some(modifier) => {
        let percentage = modifier.parse::<f32>().ok()?;

        if !(0.0..=100.0).contains(&percentage) {
          return None;
        }

        Some(percentage)
      }
      None => None,
    };

    let expression = if base.eq_ignore_ascii_case("current") {
      "currentcolor".to_owned()
    } else if base.eq_ignore_ascii_case("transparent") {
      "transparent".to_owned()
    } else {
      if base.is_empty() || !base.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
      }

      match Color::parse_tw(base) {
        Some(color) => {
          let mut css = String::new();

          let _ = color.to_css(&mut css);
          format!("var(--color-{base}, {css})")
        }
        None => format!("var(--color-{base})"),
      }
    };

    Some(Self(match percentage {
      Some(percentage) => {
        format!("color-mix(in oklab, {expression} {percentage}%, transparent)").into()
      }
      None => expression.into(),
    }))
  }
}
