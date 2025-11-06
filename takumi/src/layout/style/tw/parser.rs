use std::ops::Neg;

use cssparser::{Parser, match_ignore_ascii_case};

use crate::layout::style::{
  LengthUnit::{self, *},
  tw::TailwindPropertyParser,
  *,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwFontSize {
  pub(crate) font_size: LengthUnit,
  pub(crate) line_height: Option<LineHeight>,
}

impl<'i> FromCss<'i> for TwFontSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self::new(LengthUnit::from_css(input)?, None))
  }
}

impl TailwindPropertyParser for TwFontSize {
  fn parse_tw(token: &str) -> Option<Self> {
    // text-xs/6 => font-size: 0.75rem, line-height: 1.5em
    if let Some((font_size, line_height)) = token.split_once('/') {
      return Some(Self::new(
        TwFontSize::parse_tw(font_size)?.font_size,
        Some(LineHeight::parse_tw(line_height)?),
      ));
    }

    match_ignore_ascii_case! {token,
      "xs" => Some(
        Self::new(Rem(0.75), Some(LineHeight(Em(1.0 / 0.75)))),
      ),
      "sm" => Some(
        Self::new(Rem(0.875), Some(LineHeight(Em(1.25 / 0.875)))),
      ),
      "base" => Some(
        Self::new(Rem(1.0), Some(LineHeight(Em(1.5 / 1.0)))),
      ),
      "lg" => Some(
        Self::new(Rem(1.125), Some(LineHeight(Em(1.75 / 1.125)))),
      ),
      "xl" => Some(
        Self::new(Rem(1.25), Some(LineHeight(Em(1.75 / 1.25)))),
      ),
      "2xl" => Some(
        Self::new(Rem(1.5), Some(LineHeight(Em(2.0 / 1.5)))),
      ),
      "3xl" => Some(
        Self::new(Rem(1.875), Some(LineHeight(Em(2.25 / 1.875)))),
      ),
      "4xl" => Some(
        Self::new(Rem(2.25), Some(LineHeight(Em(2.5 / 2.25)))),
      ),
      "5xl" => Some(
        Self::new(Rem(3.0), Some(LineHeight(Em(1.0)))),
      ),
      "6xl" => Some(
        Self::new(Rem(3.75), Some(LineHeight(Em(1.0)))),
      ),
      "7xl" => Some(
        Self::new(Rem(4.5), Some(LineHeight(Em(1.0)))),
      ),
      "8xl" => Some(
        Self::new(Rem(6.0), Some(LineHeight(Em(1.0)))),
      ),
      "9xl" => Some(
        Self::new(Rem(8.0), Some(LineHeight(Em(1.0)))),
      ),
      _ => None,
    }
  }
}

impl TwFontSize {
  pub const fn new(font_size: LengthUnit, line_height: Option<LineHeight>) -> Self {
    Self {
      font_size,
      line_height,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwRound {
  pub(crate) border_radius: LengthUnit,
}

impl<'i> FromCss<'i> for TwRound {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self::new(LengthUnit::from_css(input)?))
  }
}

impl TailwindPropertyParser for TwRound {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "none" => Some(Self::new(LengthUnit::Px(0.0))),
      "sm" => Some(Self::new(LengthUnit::Rem(0.125))),
      "md" => Some(Self::new(LengthUnit::Rem(0.375))),
      "lg" => Some(Self::new(LengthUnit::Rem(0.5))),
      "xl" => Some(Self::new(LengthUnit::Rem(0.75))),
      "2xl" => Some(Self::new(LengthUnit::Rem(1.0))),
      "3xl" => Some(Self::new(LengthUnit::Rem(1.5))),
      "full" => Some(Self::new(LengthUnit::Px(9999.0))),
      _ => None,
    }
  }
}

impl TwRound {
  pub const fn new(border_radius: LengthUnit) -> Self {
    Self { border_radius }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwGridTemplate(pub GridTemplateComponents);

impl<'i> FromCss<'i> for TwGridTemplate {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(GridTemplateComponents::from_css(input)?))
  }
}

impl TailwindPropertyParser for TwGridTemplate {
  fn parse_tw(token: &str) -> Option<Self> {
    let count = token.parse::<u32>().ok()?;

    // Create repeat(count, minmax(0, 1fr))
    let track_sizes = vec![
      GridTrackSize::MinMax(GridMinMaxSize {
        min: GridLengthUnit::Unit(LengthUnit::Px(0.0)),
        max: GridLengthUnit::Fr(1.0),
      });
      count as usize
    ];

    let template_components = track_sizes
      .into_iter()
      .map(GridTemplateComponent::Single)
      .collect();

    Some(TwGridTemplate(GridTemplateComponents(template_components)))
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwGridPlacement(pub GridLine);

impl<'i> FromCss<'i> for TwGridPlacement {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(GridLine::from_css(input)?))
  }
}

impl TailwindPropertyParser for TwGridPlacement {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("auto") {
      return Some(TwGridPlacement(GridLine {
        start: None,
        end: None,
      }));
    }

    let value = token.parse::<i16>().ok()?;

    Some(TwGridPlacement(GridLine {
      start: Some(GridPlacement::Line(value)),
      end: None,
    }))
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwGridSpan(pub GridLine);

impl<'i> FromCss<'i> for TwGridSpan {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(GridLine::from_css(input)?))
  }
}

impl TailwindPropertyParser for TwGridSpan {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("full") {
      return Some(TwGridSpan(GridLine {
        start: Some(GridPlacement::Span(GridPlacementSpan::Span(u16::MAX))),
        end: None,
      }));
    }

    let span = token.parse::<u32>().ok()?;

    Some(TwGridSpan(GridLine {
      start: Some(GridPlacement::Span(GridPlacementSpan::Span(span as u16))),
      end: None,
    }))
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwGridAutoSize(pub GridTrackSizes);

impl<'i> FromCss<'i> for TwGridAutoSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(GridTrackSizes::from_css(input)?))
  }
}

impl TailwindPropertyParser for TwGridAutoSize {
  fn parse_tw(token: &str) -> Option<Self> {
    let track_size = match token {
      "auto" => GridTrackSize::Fixed(GridLengthUnit::Unit(LengthUnit::Auto)),
      "min" => GridTrackSize::Fixed(GridLengthUnit::Unit(LengthUnit::Px(0.0))),
      "max" => GridTrackSize::Fixed(GridLengthUnit::Fr(1.0)),
      "fr" => GridTrackSize::Fixed(GridLengthUnit::Fr(1.0)),
      _ => return None,
    };
    Some(TwGridAutoSize(GridTrackSizes(vec![track_size])))
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwLetterSpacing(pub LengthUnit);

impl<'i> FromCss<'i> for TwLetterSpacing {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(LengthUnit::from_css(input)?))
  }
}

impl Neg for TwLetterSpacing {
  type Output = Self;

  fn neg(self) -> Self::Output {
    TwLetterSpacing(-self.0)
  }
}

impl TailwindPropertyParser for TwLetterSpacing {
  fn parse_tw(token: &str) -> Option<Self> {
    match token {
      "tighter" => Some(TwLetterSpacing(LengthUnit::Em(-0.05))),
      "tight" => Some(TwLetterSpacing(LengthUnit::Em(-0.025))),
      "normal" => Some(TwLetterSpacing(LengthUnit::Em(0.0))),
      "wide" => Some(TwLetterSpacing(LengthUnit::Em(0.025))),
      "wider" => Some(TwLetterSpacing(LengthUnit::Em(0.05))),
      "widest" => Some(TwLetterSpacing(LengthUnit::Em(0.1))),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwBorderWidth(pub LengthUnit);

impl<'i> FromCss<'i> for TwBorderWidth {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(LengthUnit::from_css(input)?))
  }
}

impl TailwindPropertyParser for TwBorderWidth {
  fn parse_tw(token: &str) -> Option<Self> {
    if let Ok(value) = token.parse::<f32>() {
      Some(TwBorderWidth(LengthUnit::Px(value)))
    } else {
      None
    }
  }
}
