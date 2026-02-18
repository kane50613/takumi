use std::{
  ops::Neg,
  sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
  },
};

use cssparser::{Parser, Token, match_ignore_ascii_case};
use dashmap::DashMap;
use taffy::{CompactLength, Dimension, LengthPercentage, LengthPercentageAuto};

use crate::{
  layout::style::{
    AspectRatio, CssToken, FromCss, MakeComputed, ParseResult,
    tw::{TW_VAR_SPACING, TailwindPropertyParser},
  },
  rendering::Sizing,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Internal handle used by `Length::Calc`.
pub enum CalcHandle {
  /// Internal handle for a parsed calc expression.
  Expr(u64),
  /// Internal handle for a resolved linear calc expression.
  Linear(u64),
}

#[derive(Debug, Clone, Copy)]
struct CalcLinear {
  px: f32,
  percent: f32,
}

impl CalcLinear {
  fn neg(self) -> Self {
    Self {
      px: -self.px,
      percent: -self.percent,
    }
  }

  fn add(self, rhs: Self) -> Self {
    Self {
      px: self.px + rhs.px,
      percent: self.percent + rhs.percent,
    }
  }

  fn sub(self, rhs: Self) -> Self {
    Self {
      px: self.px - rhs.px,
      percent: self.percent - rhs.percent,
    }
  }

  fn scale(self, factor: f32) -> Self {
    Self {
      px: self.px * factor,
      percent: self.percent * factor,
    }
  }

  fn resolve(self, basis: f32) -> f32 {
    self.px + self.percent * basis
  }
}

#[derive(Debug, Clone)]
enum CalcExpr {
  Number(f32),
  Length(CalcLength),
  Add(Box<CalcExpr>, Box<CalcExpr>),
  Sub(Box<CalcExpr>, Box<CalcExpr>),
  Mul(Box<CalcExpr>, Box<CalcExpr>),
  Div(Box<CalcExpr>, Box<CalcExpr>),
}

#[derive(Debug, Clone)]
enum CalcEval {
  Number(f32),
  Linear(CalcLinear),
}

#[derive(Debug, Clone, Copy)]
enum CalcLength {
  Percentage(f32),
  Rem(f32),
  Em(f32),
  Vh(f32),
  Vw(f32),
  Cm(f32),
  Mm(f32),
  In(f32),
  Q(f32),
  Pt(f32),
  Pc(f32),
  Px(f32),
}

static NEXT_EXPR_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_LINEAR_ID: AtomicU64 = AtomicU64::new(1);
static CALC_EXPRS: OnceLock<DashMap<u64, CalcExpr>> = OnceLock::new();
static CALC_LINEAR: OnceLock<DashMap<u64, CalcLinear>> = OnceLock::new();

fn calc_exprs() -> &'static DashMap<u64, CalcExpr> {
  CALC_EXPRS.get_or_init(DashMap::new)
}

fn calc_linear() -> &'static DashMap<u64, CalcLinear> {
  CALC_LINEAR.get_or_init(DashMap::new)
}

fn register_expr(expr: CalcExpr) -> u64 {
  let id = NEXT_EXPR_ID.fetch_add(1, Ordering::Relaxed);
  calc_exprs().insert(id, expr);
  id
}

fn register_linear(linear: CalcLinear) -> u64 {
  let id = NEXT_LINEAR_ID.fetch_add(1, Ordering::Relaxed);
  calc_linear().insert(id, linear);
  id
}

fn linear_ptr(id: u64) -> *const () {
  ((id as usize) << 3) as *const ()
}

fn linear_id_from_ptr(ptr: *const ()) -> Option<u64> {
  let raw = ptr as usize;
  (raw != 0).then_some((raw >> 3) as u64)
}

pub(crate) fn resolve_calc_value(val: *const (), basis: f32) -> f32 {
  let Some(id) = linear_id_from_ptr(val) else {
    return 0.0;
  };

  calc_linear()
    .get(&id)
    .map(|linear| linear.resolve(basis))
    .unwrap_or(0.0)
}

fn parse_calc_sum<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CalcExpr> {
  let mut expr = parse_calc_product(input)?;

  loop {
    if input.try_parse(|parser| parser.expect_delim('+')).is_ok() {
      let rhs = parse_calc_product(input)?;
      expr = CalcExpr::Add(Box::new(expr), Box::new(rhs));
      continue;
    }

    if input.try_parse(|parser| parser.expect_delim('-')).is_ok() {
      let rhs = parse_calc_product(input)?;
      expr = CalcExpr::Sub(Box::new(expr), Box::new(rhs));
      continue;
    }

    break;
  }

  Ok(expr)
}

fn parse_calc_product<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CalcExpr> {
  let mut expr = parse_calc_factor(input)?;

  loop {
    if input.try_parse(|parser| parser.expect_delim('*')).is_ok() {
      let rhs = parse_calc_factor(input)?;
      expr = CalcExpr::Mul(Box::new(expr), Box::new(rhs));
      continue;
    }

    if input.try_parse(|parser| parser.expect_delim('/')).is_ok() {
      let rhs = parse_calc_factor(input)?;
      expr = CalcExpr::Div(Box::new(expr), Box::new(rhs));
      continue;
    }

    break;
  }

  Ok(expr)
}

fn parse_calc_factor<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CalcExpr> {
  if input.try_parse(|parser| parser.expect_delim('+')).is_ok() {
    return parse_calc_factor(input);
  }

  if input.try_parse(|parser| parser.expect_delim('-')).is_ok() {
    let inner = parse_calc_factor(input)?;
    return Ok(CalcExpr::Mul(
      Box::new(CalcExpr::Number(-1.0)),
      Box::new(inner),
    ));
  }

  let location = input.current_source_location();
  let token = input.next()?;

  match token {
    Token::Number { value, .. } => Ok(CalcExpr::Number(*value)),
    Token::Percentage { unit_value, .. } => Ok(CalcExpr::Length(CalcLength::Percentage(
      *unit_value * 100.0,
    ))),
    Token::Dimension { value, unit, .. } => {
      let unit = unit.as_ref();
      match_ignore_ascii_case! {unit,
        "px" => Ok(CalcExpr::Length(CalcLength::Px(*value))),
        "em" => Ok(CalcExpr::Length(CalcLength::Em(*value))),
        "rem" => Ok(CalcExpr::Length(CalcLength::Rem(*value))),
        "vw" => Ok(CalcExpr::Length(CalcLength::Vw(*value))),
        "vh" => Ok(CalcExpr::Length(CalcLength::Vh(*value))),
        "cm" => Ok(CalcExpr::Length(CalcLength::Cm(*value))),
        "mm" => Ok(CalcExpr::Length(CalcLength::Mm(*value))),
        "in" => Ok(CalcExpr::Length(CalcLength::In(*value))),
        "q" => Ok(CalcExpr::Length(CalcLength::Q(*value))),
        "pt" => Ok(CalcExpr::Length(CalcLength::Pt(*value))),
        "pc" => Ok(CalcExpr::Length(CalcLength::Pc(*value))),
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

fn calc_length_to_linear(length: CalcLength, sizing: &Sizing) -> CalcLinear {
  const ONE_CM_IN_PX: f32 = 96.0 / 2.54;
  const ONE_MM_IN_PX: f32 = ONE_CM_IN_PX / 10.0;
  const ONE_Q_IN_PX: f32 = ONE_CM_IN_PX / 40.0;
  const ONE_IN_PX: f32 = 2.54 * ONE_CM_IN_PX;
  const ONE_PT_IN_PX: f32 = ONE_IN_PX / 72.0;
  const ONE_PC_IN_PX: f32 = ONE_IN_PX / 6.0;

  match length {
    CalcLength::Percentage(value) => CalcLinear {
      px: 0.0,
      percent: value / 100.0,
    },
    CalcLength::Px(value) => CalcLinear {
      px: value,
      percent: 0.0,
    },
    CalcLength::Rem(value) => CalcLinear {
      px: value * sizing.viewport.font_size * sizing.viewport.device_pixel_ratio,
      percent: 0.0,
    },
    CalcLength::Em(value) => CalcLinear {
      px: value * sizing.font_size,
      percent: 0.0,
    },
    CalcLength::Vh(value) => CalcLinear {
      px: value * sizing.viewport.height.unwrap_or_default() as f32 / 100.0,
      percent: 0.0,
    },
    CalcLength::Vw(value) => CalcLinear {
      px: value * sizing.viewport.width.unwrap_or_default() as f32 / 100.0,
      percent: 0.0,
    },
    CalcLength::Cm(value) => CalcLinear {
      px: value * ONE_CM_IN_PX * sizing.viewport.device_pixel_ratio,
      percent: 0.0,
    },
    CalcLength::Mm(value) => CalcLinear {
      px: value * ONE_MM_IN_PX * sizing.viewport.device_pixel_ratio,
      percent: 0.0,
    },
    CalcLength::In(value) => CalcLinear {
      px: value * ONE_IN_PX * sizing.viewport.device_pixel_ratio,
      percent: 0.0,
    },
    CalcLength::Q(value) => CalcLinear {
      px: value * ONE_Q_IN_PX * sizing.viewport.device_pixel_ratio,
      percent: 0.0,
    },
    CalcLength::Pt(value) => CalcLinear {
      px: value * ONE_PT_IN_PX * sizing.viewport.device_pixel_ratio,
      percent: 0.0,
    },
    CalcLength::Pc(value) => CalcLinear {
      px: value * ONE_PC_IN_PX * sizing.viewport.device_pixel_ratio,
      percent: 0.0,
    },
  }
}

fn eval_calc_expr(expr: &CalcExpr, sizing: &Sizing) -> Option<CalcEval> {
  match expr {
    CalcExpr::Number(value) => Some(CalcEval::Number(*value)),
    CalcExpr::Length(length) => Some(CalcEval::Linear(calc_length_to_linear(*length, sizing))),
    CalcExpr::Add(lhs, rhs) => match (eval_calc_expr(lhs, sizing)?, eval_calc_expr(rhs, sizing)?) {
      (CalcEval::Linear(lhs), CalcEval::Linear(rhs)) => Some(CalcEval::Linear(lhs.add(rhs))),
      (CalcEval::Number(lhs), CalcEval::Number(rhs)) => Some(CalcEval::Number(lhs + rhs)),
      _ => None,
    },
    CalcExpr::Sub(lhs, rhs) => match (eval_calc_expr(lhs, sizing)?, eval_calc_expr(rhs, sizing)?) {
      (CalcEval::Linear(lhs), CalcEval::Linear(rhs)) => Some(CalcEval::Linear(lhs.sub(rhs))),
      (CalcEval::Number(lhs), CalcEval::Number(rhs)) => Some(CalcEval::Number(lhs - rhs)),
      _ => None,
    },
    CalcExpr::Mul(lhs, rhs) => match (eval_calc_expr(lhs, sizing)?, eval_calc_expr(rhs, sizing)?) {
      (CalcEval::Linear(lhs), CalcEval::Number(rhs)) => Some(CalcEval::Linear(lhs.scale(rhs))),
      (CalcEval::Number(lhs), CalcEval::Linear(rhs)) => Some(CalcEval::Linear(rhs.scale(lhs))),
      (CalcEval::Number(lhs), CalcEval::Number(rhs)) => Some(CalcEval::Number(lhs * rhs)),
      _ => None,
    },
    CalcExpr::Div(lhs, rhs) => match (eval_calc_expr(lhs, sizing)?, eval_calc_expr(rhs, sizing)?) {
      (_, CalcEval::Number(0.0)) => None,
      (CalcEval::Linear(lhs), CalcEval::Number(rhs)) => {
        Some(CalcEval::Linear(lhs.scale(1.0 / rhs)))
      }
      (CalcEval::Number(lhs), CalcEval::Number(rhs)) => Some(CalcEval::Number(lhs / rhs)),
      _ => None,
    },
  }
}

fn calc_handle_to_linear(handle: CalcHandle, sizing: &Sizing) -> Option<CalcLinear> {
  match handle {
    CalcHandle::Linear(id) => calc_linear().get(&id).map(|value| *value),
    CalcHandle::Expr(id) => {
      let expr = calc_exprs().get(&id)?.clone();
      match eval_calc_expr(&expr, sizing)? {
        CalcEval::Linear(linear) => Some(linear),
        CalcEval::Number(value) => Some(CalcLinear {
          px: value,
          percent: 0.0,
        }),
      }
    }
  }
}

/// Represents a value that can be a specific length, percentage, or automatic.
#[derive(Debug, Clone, PartialEq, Copy)]
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
  Calc(CalcHandle),
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
      "dvh" => Some(Length::Vh(100.0)),
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
      Length::Cm(v) => Length::Cm(-v),
      Length::Mm(v) => Length::Mm(-v),
      Length::In(v) => Length::In(-v),
      Length::Q(v) => Length::Q(-v),
      Length::Pt(v) => Length::Pt(-v),
      Length::Pc(v) => Length::Pc(-v),
      Length::Px(v) => Length::Px(-v),
      Length::Calc(CalcHandle::Expr(id)) => {
        let Some(expr) = calc_exprs().get(&id).map(|entry| entry.clone()) else {
          return Length::Px(0.0);
        };

        let neg_expr = CalcExpr::Mul(Box::new(CalcExpr::Number(-1.0)), Box::new(expr));
        Length::Calc(CalcHandle::Expr(register_expr(neg_expr)))
      }
      Length::Calc(CalcHandle::Linear(id)) => {
        let Some(linear) = calc_linear().get(&id).map(|entry| *entry) else {
          return Length::Px(0.0);
        };

        Length::Calc(CalcHandle::Linear(register_linear(linear.neg())))
      }
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
        let expr = input.parse_nested_block(parse_calc_sum)?;
        Ok(Self::Calc(CalcHandle::Expr(register_expr(expr))))
      }
      Token::Dimension { value, unit, .. } => {
        match_ignore_ascii_case! {unit.as_ref(),
          "px" => Ok(Self::Px(*value)),
          "em" => Ok(Self::Em(*value)),
          "rem" => Ok(Self::Rem(*value)),
          "vw" => Ok(Self::Vw(*value)),
          "vh" => Ok(Self::Vh(*value)),
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

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("length")]
  }
}

impl<const DEFAULT_AUTO: bool> Length<DEFAULT_AUTO> {
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
      Length::Calc(handle) => {
        let Some(linear) = calc_handle_to_linear(handle, sizing) else {
          return CompactLength::length(0.0);
        };

        if linear.percent == 0.0 {
          return CompactLength::length(linear.px);
        }

        if linear.px == 0.0 {
          return CompactLength::percent(linear.percent);
        }

        CompactLength::calc(linear_ptr(register_linear(linear)))
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
    const ONE_CM_IN_PX: f32 = 96.0 / 2.54;
    const ONE_MM_IN_PX: f32 = ONE_CM_IN_PX / 10.0;
    const ONE_Q_IN_PX: f32 = ONE_CM_IN_PX / 40.0;
    const ONE_IN_PX: f32 = 2.54 * ONE_CM_IN_PX;
    const ONE_PT_IN_PX: f32 = ONE_IN_PX / 72.0;
    const ONE_PC_IN_PX: f32 = ONE_IN_PX / 6.0;

    let value = match self {
      Length::Auto => 0.0,
      Length::Px(value) => value,
      Length::Percentage(value) => (value / 100.0) * percentage_full_px,
      Length::Rem(value) => value * sizing.viewport.font_size,
      Length::Em(value) => value * sizing.font_size,
      Length::Vh(value) => value * sizing.viewport.height.unwrap_or_default() as f32 / 100.0,
      Length::Vw(value) => value * sizing.viewport.width.unwrap_or_default() as f32 / 100.0,
      Length::Cm(value) => value * ONE_CM_IN_PX,
      Length::Mm(value) => value * ONE_MM_IN_PX,
      Length::In(value) => value * ONE_IN_PX,
      Length::Q(value) => value * ONE_Q_IN_PX,
      Length::Pt(value) => value * ONE_PT_IN_PX,
      Length::Pc(value) => value * ONE_PC_IN_PX,
      Length::Calc(handle) => calc_handle_to_linear(handle, sizing)
        .map(|linear| linear.resolve(percentage_full_px))
        .unwrap_or(0.0),
    };

    if matches!(
      self,
      Length::Auto
        | Length::Percentage(_)
        | Length::Vh(_)
        | Length::Vw(_)
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
      *self = Self::Px(em * sizing.font_size);
      return;
    }

    if let Self::Calc(CalcHandle::Expr(expr_id)) = *self {
      let Some(expr) = calc_exprs().get(&expr_id).map(|value| value.clone()) else {
        *self = Self::Px(0.0);
        return;
      };

      let Some(evaluated) = eval_calc_expr(&expr, sizing) else {
        *self = Self::Px(0.0);
        return;
      };

      match evaluated {
        CalcEval::Number(value) => *self = Self::Px(value),
        CalcEval::Linear(CalcLinear { px, percent: 0.0 }) => *self = Self::Px(px),
        CalcEval::Linear(CalcLinear { px: 0.0, percent }) => {
          *self = Self::Percentage(percent * 100.0)
        }
        CalcEval::Linear(linear) => *self = Self::Calc(CalcHandle::Linear(register_linear(linear))),
      }
    }
  }
}
