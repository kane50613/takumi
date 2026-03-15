use std::{cell::RefCell, ops::Neg};

use cssparser::{Parser, Token, match_ignore_ascii_case};
use taffy::{CompactLength, Dimension, LengthPercentage, LengthPercentageAuto};

use crate::{
  layout::style::{
    AspectRatio, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult,
    tw::{TW_VAR_SPACING, TailwindPropertyParser},
  },
  rendering::Sizing,
};

const ONE_CM_IN_PX: f32 = 96.0 / 2.54;
const ONE_MM_IN_PX: f32 = ONE_CM_IN_PX / 10.0;
const ONE_Q_IN_PX: f32 = ONE_CM_IN_PX / 40.0;
const ONE_IN_PX: f32 = 2.54 * ONE_CM_IN_PX;
const ONE_PT_IN_PX: f32 = ONE_IN_PX / 72.0;
const ONE_PC_IN_PX: f32 = ONE_IN_PX / 6.0;
const CALC_ZERO_EPSILON: f32 = 1e-6;

#[derive(Default)]
pub(crate) struct CalcArena {
  linear_values: RefCell<Vec<CalcLinear>>,
}

impl CalcArena {
  fn register_linear(&self, linear: CalcLinear) -> *const () {
    let mut linear_values = self.linear_values.borrow_mut();

    linear_values.push(linear);
    encode_linear_id(linear_values.len())
  }

  pub(crate) fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
    let Some(id) = decode_linear_id(val) else {
      return 0.0;
    };

    let linear_values = self.linear_values.borrow();
    linear_values
      .get(id - 1)
      .map(|linear| linear.resolve(basis))
      .unwrap_or(0.0)
  }
}

fn encode_linear_id(id: usize) -> *const () {
  // The low 3 bits are reserved because aligned pointers keep them as zero.
  ((id << 3) as *const ()).cast()
}

fn decode_linear_id(ptr: *const ()) -> Option<usize> {
  let raw = ptr as usize;
  // `raw != 0` filters out the null pointer case.
  (raw != 0).then_some(raw >> 3)
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Internal linear form of a `calc(...)` expression: `px + percent * basis`.
#[non_exhaustive]
pub struct CalcLinear {
  px: f32,
  percent: f32,
}

impl CalcLinear {
  fn resolve(self, basis: f32) -> f32 {
    self.px + self.percent * basis
  }

  pub(crate) fn components(self) -> (f32, f32) {
    (self.px, self.percent)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
/// Internal symbolic form of a `calc(...)` expression before sizing is known.
#[non_exhaustive]
pub struct CalcFormula {
  px: f32,
  percent: f32,
  rem: f32,
  em: f32,
  vh: f32,
  vw: f32,
  cqh: f32,
  cqw: f32,
  cqmin: f32,
  cqmax: f32,
  vmin: f32,
  vmax: f32,
  cm: f32,
  mm: f32,
  inch: f32,
  q: f32,
  pt: f32,
  pc: f32,
}

impl CalcFormula {
  fn px(value: f32) -> Self {
    Self {
      px: value,
      ..Default::default()
    }
  }

  fn percentage(value: f32) -> Self {
    Self {
      percent: value,
      ..Default::default()
    }
  }

  fn rem(value: f32) -> Self {
    Self {
      rem: value,
      ..Default::default()
    }
  }

  fn em(value: f32) -> Self {
    Self {
      em: value,
      ..Default::default()
    }
  }

  fn vh(value: f32) -> Self {
    Self {
      vh: value,
      ..Default::default()
    }
  }

  fn vw(value: f32) -> Self {
    Self {
      vw: value,
      ..Default::default()
    }
  }

  fn vmin(value: f32) -> Self {
    Self {
      vmin: value,
      ..Default::default()
    }
  }

  fn vmax(value: f32) -> Self {
    Self {
      vmax: value,
      ..Default::default()
    }
  }

  fn cqh(value: f32) -> Self {
    Self {
      cqh: value,
      ..Default::default()
    }
  }

  fn cqw(value: f32) -> Self {
    Self {
      cqw: value,
      ..Default::default()
    }
  }

  fn cqmin(value: f32) -> Self {
    Self {
      cqmin: value,
      ..Default::default()
    }
  }

  fn cqmax(value: f32) -> Self {
    Self {
      cqmax: value,
      ..Default::default()
    }
  }

  fn cm(value: f32) -> Self {
    Self {
      cm: value,
      ..Default::default()
    }
  }

  fn mm(value: f32) -> Self {
    Self {
      mm: value,
      ..Default::default()
    }
  }

  fn inch(value: f32) -> Self {
    Self {
      inch: value,
      ..Default::default()
    }
  }

  fn q(value: f32) -> Self {
    Self {
      q: value,
      ..Default::default()
    }
  }

  fn pt(value: f32) -> Self {
    Self {
      pt: value,
      ..Default::default()
    }
  }

  fn pc(value: f32) -> Self {
    Self {
      pc: value,
      ..Default::default()
    }
  }

  fn neg(self) -> Self {
    Self {
      px: -self.px,
      percent: -self.percent,
      rem: -self.rem,
      em: -self.em,
      vh: -self.vh,
      vw: -self.vw,
      cqh: -self.cqh,
      cqw: -self.cqw,
      cqmin: -self.cqmin,
      cqmax: -self.cqmax,
      vmin: -self.vmin,
      vmax: -self.vmax,
      cm: -self.cm,
      mm: -self.mm,
      inch: -self.inch,
      q: -self.q,
      pt: -self.pt,
      pc: -self.pc,
    }
  }

  fn add(self, rhs: Self) -> Self {
    Self {
      px: self.px + rhs.px,
      percent: self.percent + rhs.percent,
      rem: self.rem + rhs.rem,
      em: self.em + rhs.em,
      vh: self.vh + rhs.vh,
      vw: self.vw + rhs.vw,
      cqh: self.cqh + rhs.cqh,
      cqw: self.cqw + rhs.cqw,
      cqmin: self.cqmin + rhs.cqmin,
      cqmax: self.cqmax + rhs.cqmax,
      vmin: self.vmin + rhs.vmin,
      vmax: self.vmax + rhs.vmax,
      cm: self.cm + rhs.cm,
      mm: self.mm + rhs.mm,
      inch: self.inch + rhs.inch,
      q: self.q + rhs.q,
      pt: self.pt + rhs.pt,
      pc: self.pc + rhs.pc,
    }
  }

  fn sub(self, rhs: Self) -> Self {
    Self {
      px: self.px - rhs.px,
      percent: self.percent - rhs.percent,
      rem: self.rem - rhs.rem,
      em: self.em - rhs.em,
      vh: self.vh - rhs.vh,
      vw: self.vw - rhs.vw,
      cqh: self.cqh - rhs.cqh,
      cqw: self.cqw - rhs.cqw,
      cqmin: self.cqmin - rhs.cqmin,
      cqmax: self.cqmax - rhs.cqmax,
      vmin: self.vmin - rhs.vmin,
      vmax: self.vmax - rhs.vmax,
      cm: self.cm - rhs.cm,
      mm: self.mm - rhs.mm,
      inch: self.inch - rhs.inch,
      q: self.q - rhs.q,
      pt: self.pt - rhs.pt,
      pc: self.pc - rhs.pc,
    }
  }

  fn scale(self, factor: f32) -> Self {
    Self {
      px: self.px * factor,
      percent: self.percent * factor,
      rem: self.rem * factor,
      em: self.em * factor,
      vh: self.vh * factor,
      vw: self.vw * factor,
      cqh: self.cqh * factor,
      cqw: self.cqw * factor,
      cqmin: self.cqmin * factor,
      cqmax: self.cqmax * factor,
      vmin: self.vmin * factor,
      vmax: self.vmax * factor,
      cm: self.cm * factor,
      mm: self.mm * factor,
      inch: self.inch * factor,
      q: self.q * factor,
      pt: self.pt * factor,
      pc: self.pc * factor,
    }
  }

  pub(crate) fn resolve(self, sizing: &Sizing) -> CalcLinear {
    let viewport_width = sizing.viewport.width.unwrap_or_default() as f32;
    let viewport_height = sizing.viewport.height.unwrap_or_default() as f32;
    let viewport_min = viewport_width.min(viewport_height);
    let viewport_max = viewport_width.max(viewport_height);
    let container_width = sizing.query_container_width();
    let container_height = sizing.query_container_height();
    let container_min = container_width.min(container_height);
    let container_max = container_width.max(container_height);

    CalcLinear {
      px: self.px * sizing.viewport.device_pixel_ratio
        + self.rem * sizing.viewport.font_size * sizing.viewport.device_pixel_ratio
        + self.em * sizing.font_size
        + self.vh * viewport_height / 100.0
        + self.vw * viewport_width / 100.0
        + self.cqh * container_height / 100.0
        + self.cqw * container_width / 100.0
        + self.cqmin * container_min / 100.0
        + self.cqmax * container_max / 100.0
        + self.vmin * viewport_min / 100.0
        + self.vmax * viewport_max / 100.0
        + self.cm * ONE_CM_IN_PX * sizing.viewport.device_pixel_ratio
        + self.mm * ONE_MM_IN_PX * sizing.viewport.device_pixel_ratio
        + self.inch * ONE_IN_PX * sizing.viewport.device_pixel_ratio
        + self.q * ONE_Q_IN_PX * sizing.viewport.device_pixel_ratio
        + self.pt * ONE_PT_IN_PX * sizing.viewport.device_pixel_ratio
        + self.pc * ONE_PC_IN_PX * sizing.viewport.device_pixel_ratio,
      percent: self.percent,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CalcValue {
  Number(f32),
  Formula(CalcFormula),
}

fn parse_calc_sum<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CalcValue> {
  let mut value = parse_calc_product(input)?;

  loop {
    if input.try_parse(|parser| parser.expect_delim('+')).is_ok() {
      let rhs = parse_calc_product(input)?;
      value = match (value, rhs) {
        (CalcValue::Number(lhs), CalcValue::Number(rhs)) => CalcValue::Number(lhs + rhs),
        (CalcValue::Formula(lhs), CalcValue::Formula(rhs)) => CalcValue::Formula(lhs.add(rhs)),
        _ => {
          return Err(<Length as FromCss<'i>>::unexpected_token_error(
            input.current_source_location(),
            &Token::Delim('+'),
          ));
        }
      };
      continue;
    }

    if input.try_parse(|parser| parser.expect_delim('-')).is_ok() {
      let rhs = parse_calc_product(input)?;
      value = match (value, rhs) {
        (CalcValue::Number(lhs), CalcValue::Number(rhs)) => CalcValue::Number(lhs - rhs),
        (CalcValue::Formula(lhs), CalcValue::Formula(rhs)) => CalcValue::Formula(lhs.sub(rhs)),
        _ => {
          return Err(<Length as FromCss<'i>>::unexpected_token_error(
            input.current_source_location(),
            &Token::Delim('-'),
          ));
        }
      };
      continue;
    }

    break;
  }

  Ok(value)
}

pub(crate) fn parse_calc_number_expression<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, f32> {
  let location = input.current_source_location();
  let token = input.next()?.clone();

  match &token {
    Token::Function(function) if function.eq_ignore_ascii_case("calc") => {
      match input.parse_nested_block(parse_calc_sum)? {
        CalcValue::Number(value) => Ok(value),
        _ => Err(location.new_unexpected_token_error(token.clone())),
      }
    }
    _ => Err(location.new_unexpected_token_error(token.clone())),
  }
}

fn parse_calc_product<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CalcValue> {
  let mut value = parse_calc_factor(input)?;

  loop {
    if input.try_parse(|parser| parser.expect_delim('*')).is_ok() {
      let rhs = parse_calc_factor(input)?;
      value = match (value, rhs) {
        (CalcValue::Formula(lhs), CalcValue::Number(rhs)) => CalcValue::Formula(lhs.scale(rhs)),
        (CalcValue::Number(lhs), CalcValue::Formula(rhs)) => CalcValue::Formula(rhs.scale(lhs)),
        (CalcValue::Number(lhs), CalcValue::Number(rhs)) => CalcValue::Number(lhs * rhs),
        _ => {
          return Err(<Length as FromCss<'i>>::unexpected_token_error(
            input.current_source_location(),
            &Token::Delim('*'),
          ));
        }
      };
      continue;
    }

    if input.try_parse(|parser| parser.expect_delim('/')).is_ok() {
      let rhs = parse_calc_factor(input)?;
      value = match (value, rhs) {
        (_, CalcValue::Number(0.0)) => {
          return Err(<Length as FromCss<'i>>::unexpected_token_error(
            input.current_source_location(),
            &Token::Delim('/'),
          ));
        }
        (CalcValue::Formula(lhs), CalcValue::Number(rhs)) => {
          CalcValue::Formula(lhs.scale(1.0 / rhs))
        }
        (CalcValue::Number(lhs), CalcValue::Number(rhs)) => CalcValue::Number(lhs / rhs),
        _ => {
          return Err(<Length as FromCss<'i>>::unexpected_token_error(
            input.current_source_location(),
            &Token::Delim('/'),
          ));
        }
      };
      continue;
    }

    break;
  }

  Ok(value)
}

fn parse_calc_factor<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CalcValue> {
  if input.try_parse(|parser| parser.expect_delim('+')).is_ok() {
    return parse_calc_factor(input);
  }

  if input.try_parse(|parser| parser.expect_delim('-')).is_ok() {
    return Ok(match parse_calc_factor(input)? {
      CalcValue::Number(value) => CalcValue::Number(-value),
      CalcValue::Formula(formula) => CalcValue::Formula(formula.neg()),
    });
  }

  let location = input.current_source_location();
  let token = input.next()?;

  match token {
    Token::Number { value, .. } => Ok(CalcValue::Number(*value)),
    Token::Percentage { unit_value, .. } => {
      Ok(CalcValue::Formula(CalcFormula::percentage(*unit_value)))
    }
    Token::Dimension { value, unit, .. } => {
      let unit = unit.as_ref();
      match_ignore_ascii_case! {unit,
        "px" => Ok(CalcValue::Formula(CalcFormula::px(*value))),
        "em" => Ok(CalcValue::Formula(CalcFormula::em(*value))),
        "rem" => Ok(CalcValue::Formula(CalcFormula::rem(*value))),
        "vw" => Ok(CalcValue::Formula(CalcFormula::vw(*value))),
        "dvw" => Ok(CalcValue::Formula(CalcFormula::vw(*value))),
        "svw" => Ok(CalcValue::Formula(CalcFormula::vw(*value))),
        "lvw" => Ok(CalcValue::Formula(CalcFormula::vw(*value))),
        "cqw" => Ok(CalcValue::Formula(CalcFormula::cqw(*value))),
        "cqi" => Ok(CalcValue::Formula(CalcFormula::cqw(*value))),
        "vi" => Ok(CalcValue::Formula(CalcFormula::vw(*value))),
        "vh" => Ok(CalcValue::Formula(CalcFormula::vh(*value))),
        "dvh" => Ok(CalcValue::Formula(CalcFormula::vh(*value))),
        "svh" => Ok(CalcValue::Formula(CalcFormula::vh(*value))),
        "lvh" => Ok(CalcValue::Formula(CalcFormula::vh(*value))),
        "cqh" => Ok(CalcValue::Formula(CalcFormula::cqh(*value))),
        "cqb" => Ok(CalcValue::Formula(CalcFormula::cqh(*value))),
        "vb" => Ok(CalcValue::Formula(CalcFormula::vh(*value))),
        "vmin" => Ok(CalcValue::Formula(CalcFormula::vmin(*value))),
        "cqmin" => Ok(CalcValue::Formula(CalcFormula::cqmin(*value))),
        "vmax" => Ok(CalcValue::Formula(CalcFormula::vmax(*value))),
        "cqmax" => Ok(CalcValue::Formula(CalcFormula::cqmax(*value))),
        "cm" => Ok(CalcValue::Formula(CalcFormula::cm(*value))),
        "mm" => Ok(CalcValue::Formula(CalcFormula::mm(*value))),
        "in" => Ok(CalcValue::Formula(CalcFormula::inch(*value))),
        "q" => Ok(CalcValue::Formula(CalcFormula::q(*value))),
        "pt" => Ok(CalcValue::Formula(CalcFormula::pt(*value))),
        "pc" => Ok(CalcValue::Formula(CalcFormula::pc(*value))),
        _ => Err(<Length as FromCss<'i>>::unexpected_token_error(location, token)),
      }
    }
    Token::Function(name) if name.eq_ignore_ascii_case("calc") => {
      input.parse_nested_block(parse_calc_sum)
    }
    _ => Err(<Length as FromCss<'i>>::unexpected_token_error(
      location, token,
    )),
  }
}

fn is_near_zero(value: f32) -> bool {
  value.abs() <= CALC_ZERO_EPSILON
}

/// A length value that defaults to zero instead of auto.
pub type LengthDefaultsToZero = Length<false>;

/// Represents a value that can be a specific length, percentage, or automatic.
#[derive(Debug, Clone, PartialEq, Copy)]
#[non_exhaustive]
pub enum Length<const DEFAULT_AUTO: bool = true> {
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
  Calc(CalcFormula),
}

impl<const DEFAULT_AUTO: bool> Default for Length<DEFAULT_AUTO> {
  fn default() -> Self {
    if DEFAULT_AUTO {
      Self::Auto
    } else {
      Self::Px(0.0)
    }
  }
}

impl<const DEFAULT_AUTO: bool> TailwindPropertyParser for Length<DEFAULT_AUTO> {
  fn parse_tw(token: &str) -> Option<Self> {
    if let Ok(value) = token.parse::<f32>() {
      return Some(Length::Rem(value * TW_VAR_SPACING));
    }

    match AspectRatio::from_str(token) {
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

impl<const DEFAULT_AUTO: bool> Neg for Length<DEFAULT_AUTO> {
  type Output = Self;

  fn neg(self) -> Self::Output {
    self.negative()
  }
}

impl<const DEFAULT_AUTO: bool> Length<DEFAULT_AUTO> {
  /// Returns a zero pixel length unit.
  pub const fn zero() -> Self {
    Self::Px(0.0)
  }

  /// Returns a negative length unit.
  pub fn negative(self) -> Self {
    match self {
      Length::Auto => Length::Auto,
      Length::Percentage(v) => Length::Percentage(-v),
      Length::Rem(v) => Length::Rem(-v),
      Length::Em(v) => Length::Em(-v),
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

impl<const DEFAULT_AUTO: bool> From<f32> for Length<DEFAULT_AUTO> {
  fn from(value: f32) -> Self {
    Self::Px(value)
  }
}

impl<'i, const DEFAULT_AUTO: bool> FromCss<'i> for Length<DEFAULT_AUTO> {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Ident(unit) => match_ignore_ascii_case! {unit.as_ref(),
        "auto" => Ok(Self::Auto),
        _ => Err(Self::unexpected_token_error(location, token)),
      },
      Token::Function(function) if function.eq_ignore_ascii_case("calc") => {
        match input.parse_nested_block(parse_calc_sum)? {
          CalcValue::Number(value) => Ok(Self::Px(value)),
          CalcValue::Formula(formula) => Ok(Self::Calc(formula)),
        }
      }
      Token::Dimension { value, unit, .. } => {
        match_ignore_ascii_case! {unit.as_ref(),
          "px" => Ok(Self::Px(*value)),
          "em" => Ok(Self::Em(*value)),
          "rem" => Ok(Self::Rem(*value)),
          "vw" => Ok(Self::Vw(*value)),
          "dvw" => Ok(Self::Vw(*value)),
          "svw" => Ok(Self::Vw(*value)),
          "lvw" => Ok(Self::Vw(*value)),
          "cqw" => Ok(Self::CqW(*value)),
          "cqi" => Ok(Self::CqW(*value)),
          "vi" => Ok(Self::Vw(*value)),
          "vh" => Ok(Self::Vh(*value)),
          "dvh" => Ok(Self::Vh(*value)),
          "svh" => Ok(Self::Vh(*value)),
          "lvh" => Ok(Self::Vh(*value)),
          "cqh" => Ok(Self::CqH(*value)),
          "cqb" => Ok(Self::CqH(*value)),
          "vb" => Ok(Self::Vh(*value)),
          "vmin" => Ok(Self::VMin(*value)),
          "cqmin" => Ok(Self::CqMin(*value)),
          "vmax" => Ok(Self::VMax(*value)),
          "cqmax" => Ok(Self::CqMax(*value)),
          "cm" => Ok(Self::Cm(*value)),
          "mm" => Ok(Self::Mm(*value)),
          "in" => Ok(Self::In(*value)),
          "q" => Ok(Self::Q(*value)),
          "pt" => Ok(Self::Pt(*value)),
          "pc" => Ok(Self::Pc(*value)),
          _ => Err(Self::unexpected_token_error(location, token)),
        }
      }
      Token::Percentage { unit_value, .. } => Ok(Self::Percentage(*unit_value * 100.0)),
      Token::Number { value, .. } => Ok(Self::Px(*value)),
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Length)];
}

impl<const DEFAULT_AUTO: bool> Length<DEFAULT_AUTO> {
  fn to_px_pre_dpr(self, sizing: &Sizing, percentage_full_px: f32) -> f32 {
    match self {
      Length::Auto => 0.0,
      Length::Px(value) => value,
      Length::Percentage(value) => (value / 100.0) * percentage_full_px,
      Length::Rem(value) => value * sizing.viewport.font_size,
      Length::Em(value) => value * sizing.font_size,
      Length::Vh(value) => value * sizing.viewport.height.unwrap_or_default() as f32 / 100.0,
      Length::Vw(value) => value * sizing.viewport.width.unwrap_or_default() as f32 / 100.0,
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
        let viewport_width = sizing.viewport.width.unwrap_or_default() as f32;
        let viewport_height = sizing.viewport.height.unwrap_or_default() as f32;
        value * viewport_width.min(viewport_height) / 100.0
      }
      Length::VMax(value) => {
        let viewport_width = sizing.viewport.width.unwrap_or_default() as f32;
        let viewport_height = sizing.viewport.height.unwrap_or_default() as f32;
        value * viewport_width.max(viewport_height) / 100.0
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

  pub(crate) fn to_compact_length(self, sizing: &Sizing) -> CompactLength {
    match self {
      Length::Auto => CompactLength::auto(),
      Length::Percentage(value) => CompactLength::percent(value / 100.0),
      Length::Rem(value) => CompactLength::length(
        value * sizing.viewport.font_size * sizing.viewport.device_pixel_ratio,
      ),
      Length::Em(value) => CompactLength::length(value * sizing.font_size),
      Length::Vh(value) => {
        CompactLength::length(sizing.viewport.height.unwrap_or_default() as f32 * value / 100.0)
      }
      Length::Vw(value) => {
        CompactLength::length(sizing.viewport.width.unwrap_or_default() as f32 * value / 100.0)
      }
      Length::CqH(value) => CompactLength::length(sizing.query_container_height() * value / 100.0),
      Length::CqW(value) => CompactLength::length(sizing.query_container_width() * value / 100.0),
      Length::CqMin(value) => CompactLength::length(
        sizing
          .query_container_width()
          .min(sizing.query_container_height())
          * value
          / 100.0,
      ),
      Length::CqMax(value) => CompactLength::length(
        sizing
          .query_container_width()
          .max(sizing.query_container_height())
          * value
          / 100.0,
      ),
      Length::VMin(value) => {
        let viewport_width = sizing.viewport.width.unwrap_or_default() as f32;
        let viewport_height = sizing.viewport.height.unwrap_or_default() as f32;
        CompactLength::length(viewport_width.min(viewport_height) * value / 100.0)
      }
      Length::VMax(value) => {
        let viewport_width = sizing.viewport.width.unwrap_or_default() as f32;
        let viewport_height = sizing.viewport.height.unwrap_or_default() as f32;
        CompactLength::length(viewport_width.max(viewport_height) * value / 100.0)
      }
      Length::Calc(formula) => {
        let linear = formula.resolve(sizing);

        if is_near_zero(linear.percent) {
          return CompactLength::length(linear.px);
        }

        if is_near_zero(linear.px) {
          return CompactLength::percent(linear.percent);
        }

        CompactLength::calc(sizing.calc_arena.register_linear(linear))
      }
      _ => {
        CompactLength::length(self.to_px(sizing, sizing.viewport.width.unwrap_or_default() as f32))
      }
    }
  }

  pub(crate) fn resolve_to_length_percentage(self, sizing: &Sizing) -> LengthPercentage {
    let compact_length = self.to_compact_length(sizing);

    if compact_length.is_auto() {
      return LengthPercentage::length(0.0);
    }

    unsafe { LengthPercentage::from_raw(compact_length) }
  }

  pub(crate) fn to_px(self, sizing: &Sizing, percentage_full_px: f32) -> f32 {
    let value = self.to_px_pre_dpr(sizing, percentage_full_px);

    if matches!(
      self,
      Length::Auto
        | Length::Percentage(_)
        | Length::Vh(_)
        | Length::Vw(_)
        | Length::CqH(_)
        | Length::CqW(_)
        | Length::CqMin(_)
        | Length::CqMax(_)
        | Length::VMin(_)
        | Length::VMax(_)
        | Length::Em(_)
        | Length::Calc(_)
    ) {
      return value;
    }

    value * sizing.viewport.device_pixel_ratio
  }

  pub(crate) fn resolve_to_length_percentage_auto(self, sizing: &Sizing) -> LengthPercentageAuto {
    unsafe { LengthPercentageAuto::from_raw(self.to_compact_length(sizing)) }
  }

  pub(crate) fn resolve_to_dimension(self, sizing: &Sizing) -> Dimension {
    self.resolve_to_length_percentage_auto(sizing).into()
  }
}

impl<const DEFAULT_AUTO: bool> MakeComputed for Length<DEFAULT_AUTO> {
  fn make_computed(&mut self, sizing: &Sizing) {
    if let Self::Em(em) = *self {
      let dpr = sizing.viewport.device_pixel_ratio;
      let font_size = if dpr > 0.0 {
        sizing.font_size / dpr
      } else {
        sizing.font_size
      };

      *self = Self::Px(em * font_size);
      return;
    }

    if let Self::Calc(formula) = *self {
      let linear = formula.resolve(sizing);

      if is_near_zero(linear.percent) {
        *self = Self::Px(linear.px / sizing.viewport.device_pixel_ratio);
        return;
      }

      if is_near_zero(linear.px) {
        *self = Self::Percentage(linear.percent * 100.0);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use taffy::Size;

  use super::*;
  use crate::layout::Viewport;

  fn sizing() -> Sizing {
    Sizing {
      viewport: Viewport {
        width: Some(200),
        height: Some(100),
        font_size: 16.0,
        device_pixel_ratio: 2.0,
      },
      container_size: Size::NONE,
      font_size: 10.0,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  fn assert_near(lhs: f32, rhs: f32) {
    let diff = (lhs - rhs).abs();
    assert!(diff < 0.0001, "lhs={lhs}, rhs={rhs}, diff={diff}");
  }

  #[test]
  fn parse_calc_mixed_returns_formula() {
    assert_eq!(
      Length::<true>::from_str("calc(100% - 12px)"),
      Ok(Length::Calc(CalcFormula {
        percent: 1.0,
        px: -12.0,
        ..Default::default()
      }))
    );
  }

  #[test]
  fn parse_calc_number_expression_becomes_px() {
    let parsed = Length::<true>::from_str("calc(1 + 2)");
    assert_eq!(parsed, Ok(Length::Px(3.0)));
  }

  #[test]
  fn parse_calc_rejects_number_plus_length() {
    let parsed = Length::<true>::from_str("calc(1 + 2px)");
    assert!(parsed.is_err());
  }

  #[test]
  fn parse_calc_rejects_division_by_zero() {
    let parsed = Length::<true>::from_str("calc(10px / 0)");
    assert!(parsed.is_err());
  }

  #[test]
  fn negative_calc_keeps_value_sign_consistent() {
    let value: Length<true> = Length::Calc(CalcFormula {
      percent: 0.5,
      px: 10.0,
      ..Default::default()
    });
    let negated = -value;
    let sizing = sizing();
    assert_near(value.to_px(&sizing, 200.0), 120.0);
    assert_near(negated.to_px(&sizing, 200.0), -120.0);
  }

  #[test]
  fn make_computed_collapses_formula_without_percent_to_px() {
    let mut value: Length<true> = Length::Calc(CalcFormula {
      rem: 1.0,
      px: 5.0,
      ..Default::default()
    });
    value.make_computed(&sizing());
    assert_eq!(value, Length::Px(21.0));
  }

  #[test]
  fn make_computed_collapsed_px_applies_dpr_only_once_in_to_px() {
    let mut value: Length<true> = Length::Calc(CalcFormula {
      rem: 1.0,
      px: 5.0,
      ..Default::default()
    });
    let sizing = sizing();
    value.make_computed(&sizing);

    assert_eq!(value, Length::Px(21.0));
    assert_eq!(value.to_px(&sizing, 0.0), 42.0);
  }

  #[test]
  fn make_computed_collapses_formula_with_only_percent_to_percentage() {
    let mut value: Length<true> = Length::Calc(CalcFormula {
      percent: 0.5,
      ..Default::default()
    });
    value.make_computed(&sizing());
    assert_eq!(value, Length::Percentage(50.0));
  }

  #[test]
  fn make_computed_keeps_mixed_formula_as_calc() {
    let mut value: Length<true> = Length::Calc(CalcFormula {
      percent: 0.5,
      px: 10.0,
      ..Default::default()
    });
    value.make_computed(&sizing());
    assert_eq!(
      value,
      Length::Calc(CalcFormula {
        percent: 0.5,
        px: 10.0,
        ..Default::default()
      })
    );
  }

  #[test]
  fn compact_length_calc_pointer_resolves_through_callback() {
    let value: Length<true> = Length::Calc(CalcFormula {
      percent: 0.5,
      px: 10.0,
      ..Default::default()
    });
    let sizing = sizing();
    let compact = value.to_compact_length(&sizing);
    assert!(compact.is_calc());
    let resolved = sizing
      .calc_arena
      .resolve_calc_value(compact.calc_value(), 200.0);
    assert_near(resolved, 120.0);
  }

  #[test]
  fn compact_length_percent_does_not_use_calc_pointer() {
    let sizing = sizing();
    let compact = Length::<true>::Percentage(50.0).to_compact_length(&sizing);
    assert!(!compact.is_calc());
    assert_eq!(compact.tag(), CompactLength::PERCENT_TAG);
    assert_near(compact.value(), 0.5);
  }

  #[test]
  fn to_px_applies_device_pixel_ratio_for_absolute_units() {
    let px = Length::<true>::Rem(2.0).to_px(&sizing(), 100.0);
    assert_near(px, 64.0);
  }

  #[test]
  fn make_computed_em_applies_dpr_only_once_in_to_px() {
    let mut value: Length<true> = Length::Em(1.5);
    let sizing = sizing();
    value.make_computed(&sizing);
    assert_eq!(value, Length::Px(7.5));
    assert_eq!(value.to_px(&sizing, 0.0), 15.0);
  }

  #[test]
  fn parse_supports_modern_viewport_and_container_units() {
    assert_eq!(Length::<true>::from_str("12dvw"), Ok(Length::Vw(12.0)));
    assert_eq!(Length::<true>::from_str("12svw"), Ok(Length::Vw(12.0)));
    assert_eq!(Length::<true>::from_str("12lvw"), Ok(Length::Vw(12.0)));
    assert_eq!(Length::<true>::from_str("12cqw"), Ok(Length::CqW(12.0)));
    assert_eq!(Length::<true>::from_str("12cqi"), Ok(Length::CqW(12.0)));
    assert_eq!(Length::<true>::from_str("12vi"), Ok(Length::Vw(12.0)));
    assert_eq!(Length::<true>::from_str("12dvh"), Ok(Length::Vh(12.0)));
    assert_eq!(Length::<true>::from_str("12svh"), Ok(Length::Vh(12.0)));
    assert_eq!(Length::<true>::from_str("12lvh"), Ok(Length::Vh(12.0)));
    assert_eq!(Length::<true>::from_str("12cqh"), Ok(Length::CqH(12.0)));
    assert_eq!(Length::<true>::from_str("12cqb"), Ok(Length::CqH(12.0)));
    assert_eq!(Length::<true>::from_str("12vb"), Ok(Length::Vh(12.0)));
    assert_eq!(Length::<true>::from_str("12vmin"), Ok(Length::VMin(12.0)));
    assert_eq!(Length::<true>::from_str("12cqmin"), Ok(Length::CqMin(12.0)));
    assert_eq!(Length::<true>::from_str("12vmax"), Ok(Length::VMax(12.0)));
    assert_eq!(Length::<true>::from_str("12cqmax"), Ok(Length::CqMax(12.0)));
  }

  #[test]
  fn parse_calc_supports_modern_viewport_and_container_units() {
    let parsed = Length::<true>::from_str("calc(20cqmax + 5px - 2cqb)");
    assert_eq!(
      parsed,
      Ok(Length::Calc(CalcFormula {
        cqmax: 20.0,
        cqh: -2.0,
        px: 5.0,
        ..Default::default()
      }))
    );
  }

  #[test]
  fn cq_lengths_use_container_size() {
    let mut sizing = sizing();
    sizing.container_size = Size {
      width: Some(80.0),
      height: Some(40.0),
    };
    assert_near(Length::<true>::CqW(50.0).to_px(&sizing, 0.0), 40.0);
    assert_near(Length::<true>::CqH(50.0).to_px(&sizing, 0.0), 20.0);
    assert_near(Length::<true>::CqMin(50.0).to_px(&sizing, 0.0), 20.0);
    assert_near(Length::<true>::CqMax(50.0).to_px(&sizing, 0.0), 40.0);
  }

  #[test]
  fn vmin_and_vmax_resolve_to_expected_pixels() {
    let sizing = sizing();
    assert_near(Length::<true>::VMin(50.0).to_px(&sizing, 0.0), 50.0);
    assert_near(Length::<true>::VMax(50.0).to_px(&sizing, 0.0), 100.0);
  }
}
