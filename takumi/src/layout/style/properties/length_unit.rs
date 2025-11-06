use std::ops::Neg;

use cssparser::{Parser, ParserInput, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use taffy::{CompactLength, Dimension, LengthPercentage, LengthPercentageAuto, Rect};
use ts_rs::TS;

use crate::{
  layout::style::{
    AspectRatio, FromCss, ParseResult,
    tw::{TW_VAR_SPACING, TailwindPropertyParser},
  },
  rendering::RenderContext,
};

/// Represents a value that can be a specific length, percentage, or automatic.
///
/// This corresponds to CSS values that can be specified as pixels, percentages,
/// or the 'auto' keyword for automatic sizing.
#[derive(Default, Debug, Clone, Deserialize, Serialize, PartialEq, Copy, TS)]
#[serde(try_from = "LengthUnitValue", into = "LengthUnitValue")]
#[ts(as = "LengthUnitValue")]
pub enum LengthUnit {
  /// Automatic sizing based on content
  #[default]
  Auto,
  /// Percentage value relative to parent container (0-100)
  Percentage(f32),
  /// Rem value relative to the root font size
  Rem(f32),
  /// Em value relative to the font size
  Em(f32),
  /// Vh value relative to the viewport height (0-100)
  Vh(f32),
  /// Vw value relative to the viewport width (0-100)
  Vw(f32),
  /// Centimeter value
  Cm(f32),
  /// Millimeter value
  Mm(f32),
  /// Inch value
  In(f32),
  /// Quarter value
  Q(f32),
  /// Point value
  Pt(f32),
  /// Picas value
  Pc(f32),
  /// Specific pixel value
  Px(f32),
}

impl TailwindPropertyParser for LengthUnit {
  fn parse_tw(token: &str) -> Option<Self> {
    if let Ok(value) = token.parse::<f32>() {
      return Some(LengthUnit::Rem(value * TW_VAR_SPACING));
    }

    match AspectRatio::from_str(token) {
      Ok(AspectRatio::Ratio(ratio)) => return Some(LengthUnit::Percentage(ratio * 100.0)),
      Ok(AspectRatio::Auto) => return Some(LengthUnit::Auto),
      _ => {}
    }

    match_ignore_ascii_case! {token,
      "auto" => Some(LengthUnit::Auto),
      "dvw" => Some(LengthUnit::Vw(100.0)),
      "dvh" => Some(LengthUnit::Vh(100.0)),
      "px" => Some(LengthUnit::Px(1.0)),
      "full" => Some(LengthUnit::Percentage(100.0)),
      "3xs" => Some(LengthUnit::Rem(16.0)),
      "2xs" => Some(LengthUnit::Rem(18.0)),
      "xs" => Some(LengthUnit::Rem(20.0)),
      "sm" => Some(LengthUnit::Rem(24.0)),
      "md" => Some(LengthUnit::Rem(28.0)),
      "lg" => Some(LengthUnit::Rem(32.0)),
      "xl" => Some(LengthUnit::Rem(36.0)),
      "2xl" => Some(LengthUnit::Rem(42.0)),
      "3xl" => Some(LengthUnit::Rem(48.0)),
      "4xl" => Some(LengthUnit::Rem(56.0)),
      "5xl" => Some(LengthUnit::Rem(64.0)),
      "6xl" => Some(LengthUnit::Rem(72.0)),
      "7xl" => Some(LengthUnit::Rem(80.0)),
      _ => None,
    }
  }
}

/// Proxy type for CSS `LengthUnit` serialization/deserialization.
#[derive(Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LengthUnitValue {
  /// Automatic sizing based on content
  Auto,
  /// Percentage value relative to parent container (0-100)
  Percentage(f32),
  /// Rem value relative to the root font size
  Rem(f32),
  /// Em value relative to the font size
  Em(f32),
  /// Vh value relative to the viewport height (0-100)
  Vh(f32),
  /// Vw value relative to the viewport width (0-100)
  Vw(f32),
  /// Centimeter value
  Cm(f32),
  /// Millimeter value
  Mm(f32),
  /// Inch value
  In(f32),
  /// Quarter value
  #[serde(rename = "Q")]
  Q(f32),
  /// Point value
  #[serde(rename = "Pt")]
  Pt(f32),
  /// Picas value
  #[serde(rename = "Pc")]
  Pc(f32),
  /// Specific pixel value
  #[serde(untagged)]
  Px(f32),
  /// CSS string representation
  #[serde(untagged)]
  Css(String),
}

impl TryFrom<LengthUnitValue> for LengthUnit {
  type Error = String;

  fn try_from(value: LengthUnitValue) -> Result<Self, Self::Error> {
    match value {
      LengthUnitValue::Auto => Ok(Self::Auto),
      LengthUnitValue::Percentage(v) => Ok(Self::Percentage(v)),
      LengthUnitValue::Rem(v) => Ok(Self::Rem(v)),
      LengthUnitValue::Em(v) => Ok(Self::Em(v)),
      LengthUnitValue::Vh(v) => Ok(Self::Vh(v)),
      LengthUnitValue::Vw(v) => Ok(Self::Vw(v)),
      LengthUnitValue::Cm(v) => Ok(Self::Cm(v)),
      LengthUnitValue::Mm(v) => Ok(Self::Mm(v)),
      LengthUnitValue::In(v) => Ok(Self::In(v)),
      LengthUnitValue::Q(v) => Ok(Self::Q(v)),
      LengthUnitValue::Pt(v) => Ok(Self::Pt(v)),
      LengthUnitValue::Pc(v) => Ok(Self::Pc(v)),
      LengthUnitValue::Px(v) => Ok(Self::Px(v)),
      LengthUnitValue::Css(s) => {
        let mut input = ParserInput::new(&s);
        let mut parser = Parser::new(&mut input);

        let unit = LengthUnit::from_css(&mut parser).map_err(|e| e.to_string())?;

        Ok(unit)
      }
    }
  }
}

impl From<LengthUnit> for LengthUnitValue {
  fn from(value: LengthUnit) -> Self {
    match value {
      LengthUnit::Auto => LengthUnitValue::Auto,
      LengthUnit::Percentage(v) => LengthUnitValue::Percentage(v),
      LengthUnit::Rem(v) => LengthUnitValue::Rem(v),
      LengthUnit::Em(v) => LengthUnitValue::Em(v),
      LengthUnit::Vh(v) => LengthUnitValue::Vh(v),
      LengthUnit::Vw(v) => LengthUnitValue::Vw(v),
      LengthUnit::Cm(v) => LengthUnitValue::Cm(v),
      LengthUnit::Mm(v) => LengthUnitValue::Mm(v),
      LengthUnit::In(v) => LengthUnitValue::In(v),
      LengthUnit::Q(v) => LengthUnitValue::Q(v),
      LengthUnit::Pt(v) => LengthUnitValue::Pt(v),
      LengthUnit::Pc(v) => LengthUnitValue::Pc(v),
      LengthUnit::Px(v) => LengthUnitValue::Px(v),
    }
  }
}

impl Neg for LengthUnit {
  type Output = Self;

  fn neg(self) -> Self::Output {
    self.negative()
  }
}

impl LengthUnit {
  /// Returns a zero pixel length unit.
  pub const fn zero() -> Self {
    Self::Px(0.0)
  }

  /// Returns a negative length unit.
  pub fn negative(self) -> Self {
    match self {
      LengthUnit::Auto => LengthUnit::Auto,
      LengthUnit::Percentage(v) => LengthUnit::Percentage(-v),
      LengthUnit::Rem(v) => LengthUnit::Rem(-v),
      LengthUnit::Em(v) => LengthUnit::Em(-v),
      LengthUnit::Vh(v) => LengthUnit::Vh(-v),
      LengthUnit::Vw(v) => LengthUnit::Vw(-v),
      LengthUnit::Cm(v) => LengthUnit::Cm(-v),
      LengthUnit::Mm(v) => LengthUnit::Mm(-v),
      LengthUnit::In(v) => LengthUnit::In(-v),
      LengthUnit::Q(v) => LengthUnit::Q(-v),
      LengthUnit::Pt(v) => LengthUnit::Pt(-v),
      LengthUnit::Pc(v) => LengthUnit::Pc(-v),
      LengthUnit::Px(v) => LengthUnit::Px(-v),
    }
  }
}

impl From<f32> for LengthUnit {
  fn from(value: f32) -> Self {
    Self::Px(value)
  }
}

impl<'i> FromCss<'i> for LengthUnit {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match *token {
      Token::Ident(ref unit) => match_ignore_ascii_case! {&unit,
        "auto" => Ok(Self::Auto),
        _ => Err(location.new_basic_unexpected_token_error(token.clone()).into()),
      },
      Token::Dimension {
        value, ref unit, ..
      } => {
        match_ignore_ascii_case! {&unit,
          "px" => Ok(Self::Px(value)),
          "em" => Ok(Self::Em(value)),
          "rem" => Ok(Self::Rem(value)),
          "vw" => Ok(Self::Vw(value)),
          "vh" => Ok(Self::Vh(value)),
          "cm" => Ok(Self::Cm(value)),
          "mm" => Ok(Self::Mm(value)),
          "in" => Ok(Self::In(value)),
          "q" => Ok(Self::Q(value)),
          "pt" => Ok(Self::Pt(value)),
          "pc" => Ok(Self::Pc(value)),
          _ => Err(location.new_basic_unexpected_token_error(token.clone()).into()),
        }
      }
      Token::Percentage { unit_value, .. } => Ok(Self::Percentage(unit_value * 100.0)),
      Token::Number { value, .. } => Ok(Self::Px(value)),
      _ => Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      ),
    }
  }
}

impl LengthUnit {
  /// Converts the length unit to a compact length representation.
  ///
  /// This method converts the length unit (either a percentage, pixel, rem, em, vh, vw, or auto)
  /// into a compact length format that can be used by the layout engine.
  pub(crate) fn to_compact_length(self, context: &RenderContext) -> CompactLength {
    match self {
      LengthUnit::Auto => CompactLength::auto(),
      LengthUnit::Percentage(value) => CompactLength::percent(value / 100.0),
      LengthUnit::Rem(value) => CompactLength::length(value * context.viewport.font_size),
      LengthUnit::Em(value) => CompactLength::length(value * context.font_size),
      LengthUnit::Vh(value) => {
        CompactLength::length(context.viewport.height as f32 * value / 100.0)
      }
      LengthUnit::Vw(value) => CompactLength::length(context.viewport.width as f32 * value / 100.0),
      _ => CompactLength::length(self.resolve_to_px(context, context.viewport.width as f32)),
    }
  }

  /// Resolves the length unit to a `LengthPercentage`.
  pub(crate) fn resolve_to_length_percentage(self, context: &RenderContext) -> LengthPercentage {
    let compact_length = self.to_compact_length(context);

    if compact_length.is_auto() {
      return LengthPercentage::length(0.0);
    }

    // SAFETY: only length/percentage are allowed
    unsafe { LengthPercentage::from_raw(compact_length) }
  }

  /// Resolves the length unit to a pixel value.
  pub(crate) fn resolve_to_px(self, context: &RenderContext, percentage_full_px: f32) -> f32 {
    const ONE_CM_IN_PX: f32 = 96.0 / 2.54;
    const ONE_MM_IN_PX: f32 = ONE_CM_IN_PX / 10.0;
    const ONE_Q_IN_PX: f32 = ONE_CM_IN_PX / 40.0;
    const ONE_IN_PX: f32 = 2.54 * ONE_CM_IN_PX;
    const ONE_PT_IN_PX: f32 = ONE_IN_PX / 72.0;
    const ONE_PC_IN_PX: f32 = ONE_IN_PX / 6.0;

    match self {
      LengthUnit::Auto => 0.0,
      LengthUnit::Px(value) => value,
      LengthUnit::Percentage(value) => (value / 100.0) * percentage_full_px,
      LengthUnit::Rem(value) => value * context.viewport.font_size,
      LengthUnit::Em(value) => value * context.font_size,
      LengthUnit::Vh(value) => value * context.viewport.height as f32 / 100.0,
      LengthUnit::Vw(value) => value * context.viewport.width as f32 / 100.0,
      LengthUnit::Cm(value) => value * ONE_CM_IN_PX,
      LengthUnit::Mm(value) => value * ONE_MM_IN_PX,
      LengthUnit::In(value) => value * ONE_IN_PX,
      LengthUnit::Q(value) => value * ONE_Q_IN_PX,
      LengthUnit::Pt(value) => value * ONE_PT_IN_PX,
      LengthUnit::Pc(value) => value * ONE_PC_IN_PX,
    }
  }

  /// Resolves the length unit to a `LengthPercentageAuto`.
  pub(crate) fn resolve_to_length_percentage_auto(
    self,
    context: &RenderContext,
  ) -> LengthPercentageAuto {
    // SAFETY: only length/percentage/auto are allowed
    unsafe { LengthPercentageAuto::from_raw(self.to_compact_length(context)) }
  }

  /// Resolves the length unit to a `Dimension`.
  pub(crate) fn resolve_to_dimension(self, context: &RenderContext) -> Dimension {
    self.resolve_to_length_percentage_auto(context).into()
  }
}
/// Utility function to resolve a rect of length units to length percentages.
#[inline]
pub(crate) fn resolve_length_unit_rect_to_length_percentage(
  context: &RenderContext,
  value: Rect<LengthUnit>,
) -> Rect<LengthPercentage> {
  value.map(|unit| unit.resolve_to_length_percentage(context))
}

/// Utility function to resolve a rect of length units to length percentage auto.
#[inline]
pub(crate) fn resolve_length_unit_rect_to_length_percentage_auto(
  context: &RenderContext,
  value: Rect<LengthUnit>,
) -> Rect<LengthPercentageAuto> {
  value.map(|unit| unit.resolve_to_length_percentage_auto(context))
}
