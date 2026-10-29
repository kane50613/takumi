use std::{fmt, ops::Neg};

pub(crate) use crate::units::{
  ONE_CM_IN_PX, ONE_IN_PX, ONE_MM_IN_PX, ONE_PC_IN_PX, ONE_PT_IN_PX, ONE_Q_IN_PX,
};
use crate::{
  layout::layout_style::LayoutLength,
  layout_unit::LayoutUnit,
  style::{
    AspectRatio, FromCss, FromCssStr, MakeComputed, ParseResult, SizingContext, ToCss,
    calc::{CalcTerms, CalcUnit, CalcValue, parse_calc_sum},
    tw::Namespace,
    tw::{TW_VAR_SPACING, TailwindPropertyParser},
    unexpected_token,
  },
};
use cssparser::{Parser, Token, match_ignore_ascii_case};

const CALC_ZERO_EPSILON: f32 = 1e-6;
const SAFE_INT_MIN_PX: f32 = i32::MIN as f32;
const SAFE_INT_MAX_PX: f32 = i32::MAX as f32;

/// Maps a CSS dimension unit (incl. aliases like `dvw`/`cqi`) to its canonical `Length` variant.
pub(crate) fn length_from_dimension_unit(unit: &str, value: f32) -> Option<Length> {
  Some(match_ignore_ascii_case! {unit,
    "px" => Length::Px(value),
    "em" => Length::Em(value),
    "rem" => Length::Rem(value),
    "lh" => Length::Lh(value),
    "rlh" => Length::Rlh(value),
    "ch" => Length::Ch(value),
    "ex" => Length::Ex(value),
    "vw" => Length::Vw(value),
    "dvw" => Length::Vw(value),
    "svw" => Length::Vw(value),
    "lvw" => Length::Vw(value),
    "cqw" => Length::CqW(value),
    "cqi" => Length::CqW(value),
    "vi" => Length::Vw(value),
    "vh" => Length::Vh(value),
    "dvh" => Length::Vh(value),
    "svh" => Length::Vh(value),
    "lvh" => Length::Vh(value),
    "cqh" => Length::CqH(value),
    "cqb" => Length::CqH(value),
    "vb" => Length::Vh(value),
    "vmin" => Length::VMin(value),
    "cqmin" => Length::CqMin(value),
    "vmax" => Length::VMax(value),
    "cqmax" => Length::CqMax(value),
    "cm" => Length::Cm(value),
    "mm" => Length::Mm(value),
    "in" => Length::In(value),
    "q" => Length::Q(value),
    "pt" => Length::Pt(value),
    "pc" => Length::Pc(value),
    _ => return None,
  })
}

fn is_near_zero(value: f32) -> bool {
  value.abs() <= CALC_ZERO_EPSILON
}

/// CSS Values 4 [snap a length as a border width]: a whole number of device
/// pixels stays put, anything thinner rounds away from zero to one, and the
/// rest rounds toward zero.
///
/// [snap a length as a border width]: https://drafts.csswg.org/css-values-4/#snap-a-length-as-a-border-width
pub(crate) fn snap_as_border_width(device_px: f32) -> f32 {
  let truncated = device_px.trunc();

  if truncated == 0.0 && device_px != 0.0 {
    return device_px.signum();
  }

  truncated
}

fn clamp_px_for_integer_cast(value: f32) -> f32 {
  if value.is_nan() {
    return 0.0;
  }

  if value.is_infinite() {
    return if value.is_sign_positive() {
      SAFE_INT_MAX_PX
    } else {
      SAFE_INT_MIN_PX
    };
  }

  value.clamp(SAFE_INT_MIN_PX, SAFE_INT_MAX_PX)
}

/// Represents a value that can be a specific length, percentage, or automatic.
#[derive(Debug, Clone, PartialEq, Copy, Default)]
#[non_exhaustive]
pub enum Length {
  /// Automatic sizing based on content
  #[default]
  Auto,
  /// Percentage value relative to parent container (0-100)
  Percentage(f32),
  /// Rem value relative to the root font size
  Rem(f32),
  /// Em value relative to the font size
  Em(f32),
  /// Lh value relative to the element's computed line-height
  Lh(f32),
  /// Rlh value relative to the root element's computed line-height
  Rlh(f32),
  /// Ch value relative to the advance of the element's `0` glyph
  Ch(f32),
  /// Ex value relative to the x-height of the element's font
  Ex(f32),
  /// Vh value relative to the viewport height (0-100)
  Vh(f32),
  /// Vw value relative to the viewport width (0-100)
  Vw(f32),
  /// Cqh value relative to the query container height (0-100)
  CqH(f32),
  /// Cqw value relative to the query container width (0-100)
  CqW(f32),
  /// Cqmin value relative to the query container smaller dimension (0-100)
  CqMin(f32),
  /// Cqmax value relative to the query container larger dimension (0-100)
  CqMax(f32),
  /// Vmin value relative to the smaller viewport dimension (0-100)
  VMin(f32),
  /// Vmax value relative to the larger viewport dimension (0-100)
  VMax(f32),
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
  /// calc(...) expression
  Calc(CalcTerms),
}

impl Length {
  /// Construct a length from a Tailwind spacing-scale multiplier.
  #[inline]
  pub(crate) fn from_spacing(units: f32) -> Self {
    Length::Rem(units * TW_VAR_SPACING)
  }
}

impl TailwindPropertyParser for Length {
  const NAMESPACES: &'static [Namespace] = &[Namespace::Spacing];

  fn parse_tw(token: &str) -> Option<Self> {
    if let Ok(value) = token.parse::<f32>() {
      return Some(Length::from_spacing(value));
    }

    match AspectRatio::from_css_str(token) {
      Ok(AspectRatio::Ratio(ratio)) => return Some(Length::Percentage(ratio * 100.0)),
      Ok(AspectRatio::Auto) => return Some(Length::Auto),
      _ => {}
    }

    match_ignore_ascii_case! {token,
      "auto" => Some(Length::Auto),
      "dvw" => Some(Length::Vw(100.0)),
      "svw" => Some(Length::Vw(100.0)),
      "lvw" => Some(Length::Vw(100.0)),
      "cqw" => Some(Length::CqW(100.0)),
      "cqi" => Some(Length::CqW(100.0)),
      "vi" => Some(Length::Vw(100.0)),
      "dvh" => Some(Length::Vh(100.0)),
      "svh" => Some(Length::Vh(100.0)),
      "lvh" => Some(Length::Vh(100.0)),
      "cqh" => Some(Length::CqH(100.0)),
      "cqb" => Some(Length::CqH(100.0)),
      "vb" => Some(Length::Vh(100.0)),
      "vmin" => Some(Length::VMin(100.0)),
      "cqmin" => Some(Length::CqMin(100.0)),
      "vmax" => Some(Length::VMax(100.0)),
      "cqmax" => Some(Length::CqMax(100.0)),
      "px" => Some(Length::Px(1.0)),
      "full" => Some(Length::Percentage(100.0)),
      "3xs" => Some(Length::Rem(16.0)),
      "2xs" => Some(Length::Rem(18.0)),
      "xs" => Some(Length::Rem(20.0)),
      "sm" => Some(Length::Rem(24.0)),
      "md" => Some(Length::Rem(28.0)),
      "lg" => Some(Length::Rem(32.0)),
      "xl" => Some(Length::Rem(36.0)),
      "2xl" => Some(Length::Rem(42.0)),
      "3xl" => Some(Length::Rem(48.0)),
      "4xl" => Some(Length::Rem(56.0)),
      "5xl" => Some(Length::Rem(64.0)),
      "6xl" => Some(Length::Rem(72.0)),
      "7xl" => Some(Length::Rem(80.0)),
      _ => None,
    }
  }
}

impl ToCss for Length {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto => dest.write_str("auto"),
      Self::Percentage(v) => write!(dest, "{v}%"),
      Self::Rem(v) => write!(dest, "{v}rem"),
      Self::Em(v) => write!(dest, "{v}em"),
      Self::Lh(v) => write!(dest, "{v}lh"),
      Self::Rlh(v) => write!(dest, "{v}rlh"),
      Self::Ch(v) => write!(dest, "{v}ch"),
      Self::Ex(v) => write!(dest, "{v}ex"),
      Self::Vh(v) => write!(dest, "{v}vh"),
      Self::Vw(v) => write!(dest, "{v}vw"),
      Self::CqH(v) => write!(dest, "{v}cqh"),
      Self::CqW(v) => write!(dest, "{v}cqw"),
      Self::CqMin(v) => write!(dest, "{v}cqmin"),
      Self::CqMax(v) => write!(dest, "{v}cqmax"),
      Self::VMin(v) => write!(dest, "{v}vmin"),
      Self::VMax(v) => write!(dest, "{v}vmax"),
      Self::Cm(v) => write!(dest, "{v}cm"),
      Self::Mm(v) => write!(dest, "{v}mm"),
      Self::In(v) => write!(dest, "{v}in"),
      Self::Q(v) => write!(dest, "{v}q"),
      Self::Pt(v) => write!(dest, "{v}pt"),
      Self::Pc(v) => write!(dest, "{v}pc"),
      Self::Px(v) => write!(dest, "{v}px"),
      Self::Calc(f) => {
        if f.terms().next().is_none() {
          return dest.write_str("0px");
        }
        dest.write_str("calc(")?;
        let mut first = true;
        for (unit, value) in f
          .terms()
          .map(|term| (term.unit.suffix(), term.display_value()))
        {
          if first {
            if value < 0.0 {
              write!(dest, "-{magnitude}{unit}", magnitude = -value)?;
            } else {
              write!(dest, "{value}{unit}")?;
            }
          } else if value < 0.0 {
            write!(dest, " - {magnitude}{unit}", magnitude = -value)?;
          } else {
            write!(dest, " + {value}{unit}")?;
          }
          first = false;
        }
        dest.write_str(")")
      }
    }
  }
}

impl Neg for Length {
  type Output = Self;

  fn neg(self) -> Self::Output {
    self.negative()
  }
}

impl Length {
  /// Whether the length refers to a percentage, which Blink's `Length::IsFixed` rules out.
  pub(crate) fn has_percentage(self) -> bool {
    match self {
      Self::Percentage(_) => true,
      Self::Calc(terms) => terms.terms().any(|term| term.unit == CalcUnit::Percent),
      _ => false,
    }
  }

  /// Returns a zero pixel length unit.
  pub const fn zero() -> Self {
    Self::Px(0.0)
  }

  /// Whether the length is authored as a negative value. A `calc()` result is
  /// not known before it resolves, so it counts as non-negative here.
  pub(crate) fn is_negative(self) -> bool {
    match self {
      Self::Percentage(value)
      | Self::Rem(value)
      | Self::Em(value)
      | Self::Lh(value)
      | Self::Rlh(value)
      | Self::Ch(value)
      | Self::Ex(value)
      | Self::Vh(value)
      | Self::Vw(value)
      | Self::CqH(value)
      | Self::CqW(value)
      | Self::CqMin(value)
      | Self::CqMax(value)
      | Self::VMin(value)
      | Self::VMax(value)
      | Self::Cm(value)
      | Self::Mm(value)
      | Self::In(value)
      | Self::Q(value)
      | Self::Pt(value)
      | Self::Pc(value)
      | Self::Px(value) => value < 0.0,
      Self::Auto | Self::Calc(_) => false,
    }
  }

  /// Returns a negative length unit.
  pub fn negative(self) -> Self {
    match self {
      Length::Auto => Length::Auto,
      Length::Percentage(v) => Length::Percentage(-v),
      Length::Rem(v) => Length::Rem(-v),
      Length::Em(v) => Length::Em(-v),
      Length::Lh(v) => Length::Lh(-v),
      Length::Rlh(v) => Length::Rlh(-v),
      Length::Ch(v) => Length::Ch(-v),
      Length::Ex(v) => Length::Ex(-v),
      Length::Vh(v) => Length::Vh(-v),
      Length::Vw(v) => Length::Vw(-v),
      Length::CqH(v) => Length::CqH(-v),
      Length::CqW(v) => Length::CqW(-v),
      Length::CqMin(v) => Length::CqMin(-v),
      Length::CqMax(v) => Length::CqMax(-v),
      Length::VMin(v) => Length::VMin(-v),
      Length::VMax(v) => Length::VMax(-v),
      Length::Cm(v) => Length::Cm(-v),
      Length::Mm(v) => Length::Mm(-v),
      Length::In(v) => Length::In(-v),
      Length::Q(v) => Length::Q(-v),
      Length::Pt(v) => Length::Pt(-v),
      Length::Pc(v) => Length::Pc(-v),
      Length::Px(v) => Length::Px(-v),
      Length::Calc(formula) => Length::Calc(formula.neg()),
    }
  }
}

impl From<f32> for Length {
  fn from(value: f32) -> Self {
    Self::Px(value)
  }
}

impl<'i> FromCss<'i> for Length {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let token = input.next()?;

    match token {
      Token::Ident(unit) => match_ignore_ascii_case! {unit.as_ref(),
        "auto" => Ok(Self::Auto),
        _ => Err(unexpected_token!(token)),
      },
      Token::Function(function) if function.eq_ignore_ascii_case("calc") => {
        let token = token.clone();

        match input.parse_nested_block(parse_calc_sum)? {
          CalcValue::Number(value) => Ok(Self::Px(value)),
          CalcValue::Formula(formula) => formula
            .compress()
            .map(Self::Calc)
            .ok_or_else(|| unexpected_token!(&token)),
        }
      }
      Token::Dimension { value, unit, .. } => {
        length_from_dimension_unit(unit.as_ref(), *value).ok_or_else(|| unexpected_token!(token))
      }
      Token::Percentage { unit_value, .. } => Ok(Self::Percentage(*unit_value * 100.0)),
      Token::Number { value, .. } => Ok(Self::Px(*value)),
      _ => Err(unexpected_token!(token)),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &["<length>"];
}

impl Length {
  fn to_px_pre_dpr(self, sizing: &SizingContext, percentage_full_px: f32) -> f32 {
    match self {
      Length::Auto => 0.0,
      Length::Px(value) => value,
      Length::Percentage(value) => (value / 100.0) * percentage_full_px,
      Length::Rem(value) => value * sizing.rem_basis(),
      Length::Em(value) => value * sizing.font_size,
      Length::Lh(value) => value * sizing.line_height,
      Length::Rlh(value) => value * sizing.root_line_height_basis(),
      Length::Ch(value) => value * sizing.ch,
      Length::Ex(value) => value * sizing.ex,
      Length::Vh(value) => value * sizing.viewport.unit_height() / 100.0,
      Length::Vw(value) => value * sizing.viewport.unit_width() / 100.0,
      Length::CqH(value) => value * sizing.query_container_height() / 100.0,
      Length::CqW(value) => value * sizing.query_container_width() / 100.0,
      Length::CqMin(value) => {
        value
          * sizing
            .query_container_width()
            .min(sizing.query_container_height())
          / 100.0
      }
      Length::CqMax(value) => {
        value
          * sizing
            .query_container_width()
            .max(sizing.query_container_height())
          / 100.0
      }
      Length::VMin(value) => {
        value
          * sizing
            .viewport
            .unit_width()
            .min(sizing.viewport.unit_height())
          / 100.0
      }
      Length::VMax(value) => {
        value
          * sizing
            .viewport
            .unit_width()
            .max(sizing.viewport.unit_height())
          / 100.0
      }
      Length::Cm(value) => value * ONE_CM_IN_PX,
      Length::Mm(value) => value * ONE_MM_IN_PX,
      Length::In(value) => value * ONE_IN_PX,
      Length::Q(value) => value * ONE_Q_IN_PX,
      Length::Pt(value) => value * ONE_PT_IN_PX,
      Length::Pc(value) => value * ONE_PC_IN_PX,
      // Calc linear values are already in device pixels.
      Length::Calc(formula) => formula.resolve(sizing).resolve(percentage_full_px),
    }
  }

  /// Resolves to a layout length, keeping percent and calc unresolved.
  pub(crate) fn to_layout_length(self, sizing: &SizingContext) -> LayoutLength {
    match self {
      Length::Auto => LayoutLength::Auto,
      Length::Percentage(value) => LayoutLength::Percent(clamp_px_for_integer_cast(value / 100.0)),
      Length::Rem(_)
      | Length::Em(_)
      | Length::Lh(_)
      | Length::Rlh(_)
      | Length::Ch(_)
      | Length::Ex(_)
      | Length::Vh(_)
      | Length::Vw(_)
      | Length::CqH(_)
      | Length::CqW(_)
      | Length::CqMin(_)
      | Length::CqMax(_)
      | Length::VMin(_)
      | Length::VMax(_) => LayoutLength::Fixed(self.to_px_pre_dpr(sizing, 0.0)),
      Length::Calc(formula) => {
        let linear = formula.resolve(sizing);
        let px = clamp_px_for_integer_cast(linear.px);
        let percent = clamp_px_for_integer_cast(linear.percent);

        if is_near_zero(percent) {
          return LayoutLength::Fixed(px);
        }

        if is_near_zero(px) {
          return LayoutLength::Percent(percent);
        }

        LayoutLength::Calc { px, percent }
      }
      _ => LayoutLength::Fixed(self.to_px(
        sizing,
        sizing.viewport.size.width.unwrap_or_default() as f32,
      )),
    }
  }

  /// Resolves to device pixels, applying the device-pixel ratio to absolute units.
  pub fn to_px(self, sizing: &SizingContext, percentage_full_px: f32) -> f32 {
    let value = self.to_px_pre_dpr(sizing, percentage_full_px);

    // Only absolute units carry a device-pixel-ratio factor; relative units
    // already resolve against device-pixel bases.
    let value = match self {
      Length::Px(_)
      | Length::Cm(_)
      | Length::Mm(_)
      | Length::In(_)
      | Length::Q(_)
      | Length::Pt(_)
      | Length::Pc(_) => sizing.to_device(value),
      _ => value,
    };

    clamp_px_for_integer_cast(value)
  }

  /// Blink's `MinimumValueForLength`: this length in layout units against `maximum`, `auto` as
  /// zero.
  pub(crate) fn minimum_value_for(self, sizing: &SizingContext, maximum: LayoutUnit) -> LayoutUnit {
    match self {
      Length::Auto => LayoutUnit::ZERO,
      Length::Percentage(percent) => LayoutUnit::from_float(maximum.to_float() * percent / 100.0),
      length => LayoutUnit::from_float(length.to_px(sizing, maximum.to_float())),
    }
  }

  /// Blink's `ValueForLength`: this length in layout units against `maximum`, `auto` as all of it.
  pub(crate) fn value_for(self, sizing: &SizingContext, maximum: LayoutUnit) -> LayoutUnit {
    match self {
      Length::Auto => maximum,
      length => length.minimum_value_for(sizing, maximum),
    }
  }

  /// Resolves to device pixels, then snaps the result as a border width.
  pub(crate) fn to_border_px(self, sizing: &SizingContext, percentage_full_px: f32) -> f32 {
    snap_as_border_width(self.to_px(sizing, percentage_full_px))
  }
}

impl MakeComputed for Length {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Em(em) = *self {
      *self = Self::Px(em * sizing.to_css(sizing.font_size));
      return;
    }

    if let Self::Lh(lh) = *self {
      *self = Self::Px(lh * sizing.to_css(sizing.line_height));
      return;
    }

    if let Self::Rlh(rlh) = *self {
      *self = Self::Px(rlh * sizing.to_css(sizing.root_line_height_basis()));
      return;
    }

    if let Self::Ch(ch) = *self {
      *self = Self::Px(ch * sizing.to_css(sizing.ch));
      return;
    }

    if let Self::Ex(ex) = *self {
      *self = Self::Px(ex * sizing.to_css(sizing.ex));
      return;
    }

    if let Self::Calc(formula) = *self {
      let linear = formula.resolve(sizing);

      if is_near_zero(linear.percent) {
        *self = Self::Px(sizing.to_css(linear.px));
        return;
      }

      if is_near_zero(linear.px) {
        *self = Self::Percentage(linear.percent * 100.0);
      }
    }
  }
}

/// Builds a [`Length`] from a number, as in `32.0.px()`.
pub trait LengthUnit {
  /// Returns a `Px` length.
  fn px(self) -> Length;

  /// Returns a `Percentage` length.
  fn percent(self) -> Length;

  /// Returns an `Em` length.
  fn em(self) -> Length;

  /// Returns a `Rem` length.
  fn rem(self) -> Length;

  /// Returns a `Vw` length.
  fn vw(self) -> Length;

  /// Returns a `Vh` length.
  fn vh(self) -> Length;
}

impl LengthUnit for f32 {
  fn px(self) -> Length {
    Length::Px(self)
  }

  fn percent(self) -> Length {
    Length::Percentage(self)
  }

  fn em(self) -> Length {
    Length::Em(self)
  }

  fn rem(self) -> Length {
    Length::Rem(self)
  }

  fn vw(self) -> Length {
    Length::Vw(self)
  }

  fn vh(self) -> Length {
    Length::Vh(self)
  }
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use super::*;
  use crate::{geometry::Size, style::calc::CalcFormula, viewport::Viewport};

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
      line_height: 30.0,
      root_line_height: Some(40.0),
      ch: 7.0,
      ex: 7.0,
    }
  }

  fn assert_near(lhs: f32, rhs: f32) {
    let diff = (lhs - rhs).abs();
    assert!(diff < 0.0001, "lhs={lhs}, rhs={rhs}, diff={diff}");
  }

  #[test]
  fn ex_is_the_x_height_of_the_font() {
    assert_eq!(Length::from_css_str("4ex"), Ok(Length::Ex(4.0)));

    let mut sizing = sizing();

    sizing.ex = 6.0;

    assert_near(Length::Ex(2.0).to_px(&sizing, 0.0), 12.0);
  }

  #[test]
  fn parse_calc_more_than_four_units_fails() {
    assert!(Length::from_css_str("calc(1px + 1em + 1rem + 1vh + 1vw)").is_err());
  }

  #[test]
  fn parse_calc_sums_absolute_lengths_into_px() {
    let parsed = Length::from_css_str("calc(1in + 1cm + 1mm + 1pt + 1pc + 1q + 1px + 1em)");

    assert_eq!(
      parsed.map(|length| length.to_css_string()),
      Ok("calc(156.85301px + 1em)".to_string())
    );
    assert_eq!(
      Length::from_css_str("calc(1in + 1px)").map(|length| length.to_css_string()),
      Ok("calc(97px)".to_string())
    );
  }

  #[test]
  fn parse_calc_mixed_returns_formula() {
    assert_eq!(
      Length::from_css_str("calc(100% - 12px)"),
      Ok(Length::Calc(
        CalcFormula {
          percent: 1.0,
          px: -12.0,
          ..Default::default()
        }
        .compress()
        .unwrap()
      ))
    );
  }

  #[test]
  fn parse_calc_number_expression_becomes_px() {
    let parsed = Length::from_css_str("calc(1 + 2)");
    assert_eq!(parsed, Ok(Length::Px(3.0)));
  }

  #[test]
  fn parse_calc_rejects_number_plus_length() {
    let parsed = Length::from_css_str("calc(1 + 2px)");
    assert!(parsed.is_err());
  }

  #[test]
  fn parse_calc_rejects_division_by_zero() {
    let parsed = Length::from_css_str("calc(10px / 0)");
    assert!(parsed.is_err());
  }

  #[test]
  fn negative_calc_keeps_value_sign_consistent() {
    let value: Length = Length::Calc(
      CalcFormula {
        percent: 0.5,
        px: 10.0,
        ..Default::default()
      }
      .compress()
      .unwrap(),
    );
    let negated = -value;
    let sizing = sizing();
    assert_near(value.to_px(&sizing, 200.0), 120.0);
    assert_near(negated.to_px(&sizing, 200.0), -120.0);
  }

  #[test]
  fn make_computed_collapsed_px_applies_dpr_only_once_in_to_px() {
    let mut value: Length = Length::Calc(
      CalcFormula {
        rem: 1.0,
        px: 5.0,
        ..Default::default()
      }
      .compress()
      .unwrap(),
    );
    let sizing = sizing();
    value.make_computed(&sizing);

    assert_eq!(value, Length::Px(21.0));
    assert_eq!(value.to_px(&sizing, 0.0), 42.0);
  }

  #[test]
  fn make_computed_collapses_formula_with_only_percent_to_percentage() {
    let mut value: Length = Length::Calc(
      CalcFormula {
        percent: 0.5,
        ..Default::default()
      }
      .compress()
      .unwrap(),
    );
    value.make_computed(&sizing());
    assert_eq!(value, Length::Percentage(50.0));
  }

  #[test]
  fn make_computed_keeps_mixed_formula_as_calc() {
    let mut value: Length = Length::Calc(
      CalcFormula {
        percent: 0.5,
        px: 10.0,
        ..Default::default()
      }
      .compress()
      .unwrap(),
    );
    value.make_computed(&sizing());
    assert_eq!(
      value,
      Length::Calc(
        CalcFormula {
          percent: 0.5,
          px: 10.0,
          ..Default::default()
        }
        .compress()
        .unwrap()
      )
    );
  }

  #[test]
  fn percentage_lays_out_as_a_fraction() {
    assert_eq!(
      Length::Percentage(50.0).to_layout_length(&sizing()),
      LayoutLength::Percent(0.5)
    );
  }

  #[test]
  fn to_px_applies_device_pixel_ratio_for_absolute_units() {
    let px = Length::Rem(2.0).to_px(&sizing(), 100.0);
    assert_near(px, 64.0);
  }

  fn descendant_sizing() -> SizingContext {
    let mut sizing = sizing();
    sizing.root_font_size = Some(32.0);
    sizing
  }

  #[test]
  fn rem_to_px_does_not_double_apply_dpr_when_root_font_size_set() {
    let sizing = descendant_sizing();
    assert_near(Length::Rem(1.0).to_px(&sizing, 0.0), 32.0);
    assert_near(Length::Rem(2.0).to_px(&sizing, 0.0), 64.0);
    assert_near(Length::Rem(0.5).to_px(&sizing, 0.0), 16.0);
  }

  #[test]
  fn rem_to_layout_length_does_not_double_apply_dpr_when_root_font_size_set() {
    let sizing = descendant_sizing();

    assert_eq!(
      Length::Rem(1.0).to_layout_length(&sizing),
      LayoutLength::Fixed(32.0)
    );
  }

  #[test]
  fn calc_with_rem_does_not_double_apply_dpr_when_root_font_size_set() {
    let sizing = descendant_sizing();
    let value: Length = Length::Calc(
      CalcFormula {
        rem: 1.0,
        ..Default::default()
      }
      .compress()
      .unwrap(),
    );
    assert_near(value.to_px(&sizing, 0.0), 32.0);
  }

  #[test]
  fn units_agree_in_device_space_at_dpr_2() {
    // sizing(): dpr = 2, viewport.size = (200, 100) device px (= 100x50 css px).
    // Absolute units cross the dpr boundary; viewport/percentage units already
    // resolve in device space, so all land in the same 200px-wide frame.
    let sizing = sizing();

    assert_near(Length::Px(100.0).to_px(&sizing, 0.0), 200.0);
    assert_near(Length::Vw(100.0).to_px(&sizing, 0.0), 200.0);
    assert_near(Length::Percentage(50.0).to_px(&sizing, 200.0), 100.0);
    // 1in = 96 css px -> 192 device px.
    assert_near(Length::In(1.0).to_px(&sizing, 0.0), 192.0);
  }

  #[test]
  fn calc_with_rem_and_px_does_not_double_apply_dpr_when_root_font_size_set() {
    let sizing = descendant_sizing();
    let value: Length = Length::Calc(
      CalcFormula {
        rem: 1.0,
        px: 5.0,
        ..Default::default()
      }
      .compress()
      .unwrap(),
    );
    assert_near(value.to_px(&sizing, 0.0), 42.0);
  }

  #[test]
  fn make_computed_calc_with_rem_collapses_correctly_when_root_font_size_set() {
    let mut value: Length = Length::Calc(
      CalcFormula {
        rem: 1.0,
        px: 5.0,
        ..Default::default()
      }
      .compress()
      .unwrap(),
    );
    let sizing = descendant_sizing();
    value.make_computed(&sizing);
    assert_eq!(value, Length::Px(21.0));
    assert_near(value.to_px(&sizing, 0.0), 42.0);
  }

  #[test]
  fn make_computed_em_applies_dpr_only_once_in_to_px() {
    let mut value: Length = Length::Em(1.5);
    let sizing = sizing();
    value.make_computed(&sizing);
    assert_eq!(value, Length::Px(7.5));
    assert_eq!(value.to_px(&sizing, 0.0), 15.0);
  }

  #[test]
  fn parse_supports_modern_viewport_and_container_units() {
    assert_eq!(Length::from_css_str("12dvw"), Ok(Length::Vw(12.0)));
    assert_eq!(Length::from_css_str("12svw"), Ok(Length::Vw(12.0)));
    assert_eq!(Length::from_css_str("12lvw"), Ok(Length::Vw(12.0)));
    assert_eq!(Length::from_css_str("12cqw"), Ok(Length::CqW(12.0)));
    assert_eq!(Length::from_css_str("12cqi"), Ok(Length::CqW(12.0)));
    assert_eq!(Length::from_css_str("12vi"), Ok(Length::Vw(12.0)));
    assert_eq!(Length::from_css_str("12dvh"), Ok(Length::Vh(12.0)));
    assert_eq!(Length::from_css_str("12svh"), Ok(Length::Vh(12.0)));
    assert_eq!(Length::from_css_str("12lvh"), Ok(Length::Vh(12.0)));
    assert_eq!(Length::from_css_str("12cqh"), Ok(Length::CqH(12.0)));
    assert_eq!(Length::from_css_str("12cqb"), Ok(Length::CqH(12.0)));
    assert_eq!(Length::from_css_str("12vb"), Ok(Length::Vh(12.0)));
    assert_eq!(Length::from_css_str("12vmin"), Ok(Length::VMin(12.0)));
    assert_eq!(Length::from_css_str("12cqmin"), Ok(Length::CqMin(12.0)));
    assert_eq!(Length::from_css_str("12vmax"), Ok(Length::VMax(12.0)));
    assert_eq!(Length::from_css_str("12cqmax"), Ok(Length::CqMax(12.0)));
  }

  #[test]
  fn parse_supports_lh_and_rlh_units() {
    assert_eq!(Length::from_css_str("1.5lh"), Ok(Length::Lh(1.5)));
    assert_eq!(Length::from_css_str("2rlh"), Ok(Length::Rlh(2.0)));
    assert_eq!(Length::from_css_str("4ch"), Ok(Length::Ch(4.0)));
  }

  #[test]
  fn ch_resolves_to_the_zero_advance() {
    let sizing = sizing();

    assert_near(Length::Ch(2.0).to_px(&sizing, 0.0), 14.0);
    assert_near(
      Length::from_css_str("calc(2ch + 1px)")
        .unwrap()
        .to_px(&sizing, 0.0),
      16.0,
    );
  }

  #[test]
  fn lh_and_rlh_resolve_to_line_height_basis() {
    let sizing = sizing();
    assert_near(Length::Lh(1.0).to_px(&sizing, 0.0), 30.0);
    assert_near(Length::Lh(2.0).to_px(&sizing, 0.0), 60.0);
    assert_near(Length::Rlh(1.0).to_px(&sizing, 0.0), 40.0);
    assert_near(Length::Rlh(0.5).to_px(&sizing, 0.0), 20.0);
  }

  #[test]
  fn rlh_falls_back_to_viewport_font_size_without_document_root() {
    let mut sizing = sizing();
    sizing.root_line_height = None;
    assert_near(Length::Rlh(1.0).to_px(&sizing, 0.0), 32.0);
  }

  #[test]
  fn parse_calc_supports_lh_and_rlh() {
    let parsed = Length::from_css_str("calc(1lh + 2rlh - 3px)");
    assert_eq!(
      parsed,
      Ok(Length::Calc(
        CalcFormula {
          lh: 1.0,
          rlh: 2.0,
          px: -3.0,
          ..Default::default()
        }
        .compress()
        .unwrap()
      ))
    );
  }

  #[test]
  fn calc_lh_resolves_through_line_height_basis() {
    let sizing = sizing();
    let parsed = Length::from_css_str("calc(1lh + 2px)");
    assert_eq!(
      parsed,
      Ok(Length::Calc(
        CalcFormula {
          lh: 1.0,
          px: 2.0,
          ..Default::default()
        }
        .compress()
        .unwrap()
      ))
    );
    if let Ok(value) = parsed {
      assert_near(value.to_px(&sizing, 0.0), 34.0);
    }
  }

  #[test]
  fn make_computed_lh_collapses_to_px_in_pre_dpr_space() {
    let mut value: Length = Length::Lh(1.5);
    let sizing = sizing();
    value.make_computed(&sizing);
    assert_eq!(value, Length::Px(22.5));
    assert_eq!(value.to_px(&sizing, 0.0), 45.0);
  }

  #[test]
  fn parse_calc_supports_modern_viewport_and_container_units() {
    let parsed = Length::from_css_str("calc(20cqmax + 5px - 2cqb)");
    assert_eq!(
      parsed,
      Ok(Length::Calc(
        CalcFormula {
          cqmax: 20.0,
          cqh: -2.0,
          px: 5.0,
          ..Default::default()
        }
        .compress()
        .unwrap()
      ))
    );
  }

  #[test]
  fn cq_lengths_use_container_size() {
    let mut sizing = sizing();
    sizing.container_size = Size {
      width: Some(80.0),
      height: Some(40.0),
    };
    assert_near(Length::CqW(50.0).to_px(&sizing, 0.0), 40.0);
    assert_near(Length::CqH(50.0).to_px(&sizing, 0.0), 20.0);
    assert_near(Length::CqMin(50.0).to_px(&sizing, 0.0), 20.0);
    assert_near(Length::CqMax(50.0).to_px(&sizing, 0.0), 40.0);
  }

  /// Whether resolution read the query container.
  #[test]
  fn resolving_a_container_length_reports_the_read() {
    let mut sizing = sizing();
    sizing.container_size = Size {
      width: Some(80.0),
      height: Some(40.0),
    };

    sizing.container_read.set(false);
    Length::Px(10.0).to_px(&sizing, 0.0);
    assert!(!sizing.container_read.get(), "a px length reads nothing");

    assert_near(Length::CqW(50.0).to_px(&sizing, 0.0), 40.0);
    assert!(
      sizing.container_read.get(),
      "a cqw length reads the container"
    );

    // Zero resolves the same against every container, yet still reports the
    // read: the flag tracks what was consulted, not whether it mattered.
    sizing.container_read.set(false);
    assert_near(Length::CqW(0.0).to_px(&sizing, 0.0), 0.0);
    assert!(sizing.container_read.get());
  }

  #[test]
  fn vmin_and_vmax_resolve_to_expected_pixels() {
    let sizing = sizing();
    assert_near(Length::VMin(50.0).to_px(&sizing, 0.0), 50.0);
    assert_near(Length::VMax(50.0).to_px(&sizing, 0.0), 100.0);
  }

  #[test]
  fn parse_calc_supports_constants() {
    assert_eq!(
      Length::from_css_str("calc(pi)").as_ref(),
      Ok(&Length::Px(std::f32::consts::PI))
    );
    assert_eq!(
      Length::from_css_str("calc(e)").as_ref(),
      Ok(&Length::Px(std::f32::consts::E))
    );

    let inf = Length::from_css_str("calc(infinity)");
    assert_matches!(inf, Ok(Length::Px(v)) if v.is_infinite() && v.is_sign_positive());

    let neg_inf = Length::from_css_str("calc(-infinity)");
    assert_matches!(neg_inf, Ok(Length::Px(v)) if v.is_infinite() && v.is_sign_negative());

    let nan = Length::from_css_str("calc(nan)");
    assert_matches!(nan, Ok(Length::Px(v)) if v.is_nan());
  }

  #[test]
  fn parse_calc_infinity_times_length_clamps_in_to_px() {
    let parsed = Length::from_css_str("calc(infinity * 1px)");
    let sizing = sizing();
    assert!(parsed.is_ok(), "expected successful parse, got {parsed:?}");
    let Ok(length) = parsed else {
      return;
    };
    let resolved = length.to_px(&sizing, 200.0);

    assert_eq!(resolved, SAFE_INT_MAX_PX);
    assert!(resolved.is_finite());
  }

  #[test]
  fn calc_non_finite_never_reaches_layout_length() {
    let sizing = sizing();

    for css in [
      "calc(1px * nan)",
      "calc(1px * infinity)",
      "calc(1px * -infinity)",
      "calc(100% * nan)",
      "calc(100% * nan + 1px)",
    ] {
      let parsed = Length::from_css_str(css);
      assert!(parsed.is_ok(), "expected {css} to parse, got {parsed:?}");
      let Ok(length) = parsed else {
        continue;
      };
      let finite = match length.to_layout_length(&sizing) {
        LayoutLength::Fixed(px) => px.is_finite(),
        LayoutLength::Percent(percent) => percent.is_finite(),
        LayoutLength::Calc { px, percent } => px.is_finite() && percent.is_finite(),
        _ => true,
      };

      assert!(finite, "{css} produced a non-finite LayoutLength");
    }
  }

  #[test]
  fn infinite_percentage_never_reaches_layout_length() {
    for (value, expected) in [
      (f32::INFINITY, SAFE_INT_MAX_PX),
      (f32::NEG_INFINITY, SAFE_INT_MIN_PX),
      (f32::NAN, 0.0),
    ] {
      assert_eq!(
        Length::Percentage(value).to_layout_length(&sizing()),
        LayoutLength::Percent(expected),
        "for {value}"
      );
    }
  }

  #[test]
  fn calc_rejects_deeply_nested_unary_signs() {
    let css = format!("calc({signs}1px)", signs = "- ".repeat(200));

    assert!(Length::from_css_str(&css).is_err());
  }

  #[test]
  fn calc_rejects_deeply_nested_calc_functions() {
    let css = format!(
      "{open}1px{close}",
      open = "calc(".repeat(200),
      close = ")".repeat(200)
    );

    assert!(Length::from_css_str(&css).is_err());
  }

  #[test]
  fn calc_still_accepts_shallow_nesting() {
    let sizing = sizing();
    let parsed = Length::from_css_str("calc(calc(calc(1px + 2px)))");
    assert!(parsed.is_ok(), "expected successful parse, got {parsed:?}");
    let Ok(length) = parsed else {
      return;
    };

    assert_near(length.to_px(&sizing, 200.0), sizing.to_device(3.0));
  }
}
