use std::cell::RefCell;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Length, ONE_CM_IN_PX, ONE_IN_PX, ONE_MM_IN_PX, ONE_PC_IN_PX, ONE_PT_IN_PX, ONE_Q_IN_PX,
  ParseResult, SizingContext, length_from_dimension_unit, unexpected_token,
};

/// Interns `calc(...)` linear values so `CompactLength` can reference them by id.
#[derive(Default)]
pub(crate) struct CalcArena {
  linear_values: RefCell<Vec<CalcLinear>>,
}

impl CalcArena {
  pub(crate) fn register_linear(&self, linear: CalcLinear) -> *const () {
    let mut linear_values = self.linear_values.borrow_mut();

    linear_values.push(linear);
    encode_linear_id(linear_values.len())
  }

  /// Resolves an interned `calc(...)` value against a percentage basis.
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
  (id << 3) as *const ()
}

fn decode_linear_id(ptr: *const ()) -> Option<usize> {
  let raw = ptr as usize;
  // Reject pointers that `encode_linear_id` could not have produced: the low 3
  // bits must be clear and the 1-based id must be non-zero.
  if raw & 0b111 != 0 {
    return None;
  }
  let id = raw >> 3;
  (id != 0).then_some(id)
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Internal linear form of a `calc(...)` expression: `px + percent * basis`.
pub struct CalcLinear {
  pub(crate) px: f32,
  pub(crate) percent: f32,
}

impl CalcLinear {
  pub(crate) fn resolve(self, basis: f32) -> f32 {
    self.px + self.percent * basis
  }

  /// The `(px, percent)` coefficients.
  pub fn components(self) -> (f32, f32) {
    (self.px, self.percent)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
/// Internal symbolic form of a `calc(...)` expression before sizing is known.
pub struct CalcFormula {
  pub(crate) px: f32,
  pub(crate) percent: f32,
  pub(crate) rem: f32,
  pub(crate) em: f32,
  pub(crate) lh: f32,
  pub(crate) rlh: f32,
  pub(crate) vh: f32,
  pub(crate) vw: f32,
  pub(crate) cqh: f32,
  pub(crate) cqw: f32,
  pub(crate) cqmin: f32,
  pub(crate) cqmax: f32,
  pub(crate) vmin: f32,
  pub(crate) vmax: f32,
  pub(crate) cm: f32,
  pub(crate) mm: f32,
  pub(crate) inch: f32,
  pub(crate) q: f32,
  pub(crate) pt: f32,
  pub(crate) pc: f32,
}

impl CalcFormula {
  /// Hashes every coefficient by bit pattern. Keep in sync with the fields
  /// above.
  pub(crate) fn hash_bits(&self, hasher: &mut impl core::hash::Hasher) {
    use core::hash::Hash;

    for value in [
      self.px,
      self.percent,
      self.rem,
      self.em,
      self.lh,
      self.rlh,
      self.vh,
      self.vw,
      self.cqh,
      self.cqw,
      self.cqmin,
      self.cqmax,
      self.vmin,
      self.vmax,
      self.cm,
      self.mm,
      self.inch,
      self.q,
      self.pt,
      self.pc,
    ] {
      value.to_bits().hash(hasher);
    }
  }
}

/// Invokes `$callback!` with every `CalcFormula` field, keeping constructors and ops in sync.
macro_rules! for_each_unit {
  ($callback:ident) => {
    $callback!(
      px, percent, rem, em, lh, rlh, vh, vw, cqh, cqw, cqmin, cqmax, vmin, vmax, cm, mm, inch, q,
      pt, pc
    );
  };
}

macro_rules! calc_constructors {
  ($($field:ident),+) => {
    $(
      fn $field(value: f32) -> Self {
        Self { $field: value, ..Default::default() }
      }
    )+
  };
}

macro_rules! calc_neg {
  ($($field:ident),+) => {
    pub(crate) fn neg(self) -> Self {
      Self { $($field: -self.$field),+ }
    }
  };
}

macro_rules! calc_add {
  ($($field:ident),+) => {
    fn add(self, rhs: Self) -> Self {
      Self { $($field: self.$field + rhs.$field),+ }
    }
  };
}

macro_rules! calc_sub {
  ($($field:ident),+) => {
    fn sub(self, rhs: Self) -> Self {
      Self { $($field: self.$field - rhs.$field),+ }
    }
  };
}

macro_rules! calc_scale {
  ($($field:ident),+) => {
    fn scale(self, factor: f32) -> Self {
      Self { $($field: Self::scale_component(self.$field, factor)),+ }
    }
  };
}

impl CalcFormula {
  fn scale_component(value: f32, factor: f32) -> f32 {
    if value == 0.0 { 0.0 } else { value * factor }
  }

  for_each_unit!(calc_constructors);
  for_each_unit!(calc_neg);
  for_each_unit!(calc_add);
  for_each_unit!(calc_sub);
  for_each_unit!(calc_scale);
}

impl CalcFormula {
  /// Collapses the symbolic formula into a `px + percent * basis` linear form.
  pub fn resolve(self, sizing: &SizingContext) -> CalcLinear {
    let viewport_width = sizing.viewport.size.width.unwrap_or_default() as f32;
    let viewport_height = sizing.viewport.size.height.unwrap_or_default() as f32;
    let viewport_min = viewport_width.min(viewport_height);
    let viewport_max = viewport_width.max(viewport_height);
    let container_width = sizing.query_container_width();
    let container_height = sizing.query_container_height();
    let container_min = container_width.min(container_height);
    let container_max = container_width.max(container_height);

    // Absolute units are authored in CSS px and cross the dpr boundary once;
    // every relative unit already resolves against a device-pixel basis.
    let absolute_css = self.px
      + self.cm * ONE_CM_IN_PX
      + self.mm * ONE_MM_IN_PX
      + self.inch * ONE_IN_PX
      + self.q * ONE_Q_IN_PX
      + self.pt * ONE_PT_IN_PX
      + self.pc * ONE_PC_IN_PX;

    CalcLinear {
      px: sizing.to_device(absolute_css)
        + self.rem * sizing.rem_basis()
        + self.em * sizing.font_size
        + self.lh * sizing.line_height
        + self.rlh * sizing.root_line_height_basis()
        + self.vh * viewport_height / 100.0
        + self.vw * viewport_width / 100.0
        + self.cqh * container_height / 100.0
        + self.cqw * container_width / 100.0
        + self.cqmin * container_min / 100.0
        + self.cqmax * container_max / 100.0
        + self.vmin * viewport_min / 100.0
        + self.vmax * viewport_max / 100.0,
      percent: self.percent,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CalcValue {
  Number(f32),
  Formula(CalcFormula),
}

// Matches Blink's `kMaxExpressionDepth` (css_math_expression_node.h).
const MAX_CALC_DEPTH: u32 = 100;

pub(crate) fn parse_calc_sum<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CalcValue> {
  parse_calc_sum_at(input, 0)
}

fn parse_calc_sum_at<'i>(input: &mut Parser<'i, '_>, depth: u32) -> ParseResult<'i, CalcValue> {
  let mut value = parse_calc_product_at(input, depth)?;

  loop {
    if input.try_parse(|parser| parser.expect_delim('+')).is_ok() {
      let rhs = parse_calc_product_at(input, depth)?;
      value = match (value, rhs) {
        (CalcValue::Number(lhs), CalcValue::Number(rhs)) => CalcValue::Number(lhs + rhs),
        (CalcValue::Formula(lhs), CalcValue::Formula(rhs)) => CalcValue::Formula(lhs.add(rhs)),
        _ => {
          return Err(unexpected_token!(
            Length,
            input.current_source_location(),
            &Token::Delim('+'),
          ));
        }
      };
      continue;
    }

    if input.try_parse(|parser| parser.expect_delim('-')).is_ok() {
      let rhs = parse_calc_product_at(input, depth)?;
      value = match (value, rhs) {
        (CalcValue::Number(lhs), CalcValue::Number(rhs)) => CalcValue::Number(lhs - rhs),
        (CalcValue::Formula(lhs), CalcValue::Formula(rhs)) => CalcValue::Formula(lhs.sub(rhs)),
        _ => {
          return Err(unexpected_token!(
            Length,
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

/// Parses a `calc(...)` that must reduce to a unitless number.
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

fn parse_calc_product_at<'i>(input: &mut Parser<'i, '_>, depth: u32) -> ParseResult<'i, CalcValue> {
  let mut value = parse_calc_factor_at(input, depth)?;

  loop {
    if input.try_parse(|parser| parser.expect_delim('*')).is_ok() {
      let rhs = parse_calc_factor_at(input, depth)?;
      value = match (value, rhs) {
        (CalcValue::Formula(lhs), CalcValue::Number(rhs)) => CalcValue::Formula(lhs.scale(rhs)),
        (CalcValue::Number(lhs), CalcValue::Formula(rhs)) => CalcValue::Formula(rhs.scale(lhs)),
        (CalcValue::Number(lhs), CalcValue::Number(rhs)) => CalcValue::Number(lhs * rhs),
        _ => {
          return Err(unexpected_token!(
            Length,
            input.current_source_location(),
            &Token::Delim('*'),
          ));
        }
      };
      continue;
    }

    if input.try_parse(|parser| parser.expect_delim('/')).is_ok() {
      let rhs = parse_calc_factor_at(input, depth)?;
      value = match (value, rhs) {
        (_, CalcValue::Number(0.0)) => {
          return Err(unexpected_token!(
            Length,
            input.current_source_location(),
            &Token::Delim('/'),
          ));
        }
        (CalcValue::Formula(lhs), CalcValue::Number(rhs)) => {
          CalcValue::Formula(lhs.scale(1.0 / rhs))
        }
        (CalcValue::Number(lhs), CalcValue::Number(rhs)) => CalcValue::Number(lhs / rhs),
        _ => {
          return Err(unexpected_token!(
            Length,
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

impl CalcFormula {
  /// Bridges a single-unit `Length` to its symbolic `calc(...)` coefficient.
  fn from_length(length: Length) -> Self {
    match length {
      Length::Px(v) => Self::px(v),
      Length::Em(v) => Self::em(v),
      Length::Rem(v) => Self::rem(v),
      Length::Lh(v) => Self::lh(v),
      Length::Rlh(v) => Self::rlh(v),
      Length::Vw(v) => Self::vw(v),
      Length::CqW(v) => Self::cqw(v),
      Length::Vh(v) => Self::vh(v),
      Length::CqH(v) => Self::cqh(v),
      Length::VMin(v) => Self::vmin(v),
      Length::CqMin(v) => Self::cqmin(v),
      Length::VMax(v) => Self::vmax(v),
      Length::CqMax(v) => Self::cqmax(v),
      Length::Cm(v) => Self::cm(v),
      Length::Mm(v) => Self::mm(v),
      Length::In(v) => Self::inch(v),
      Length::Q(v) => Self::q(v),
      Length::Pt(v) => Self::pt(v),
      Length::Pc(v) => Self::pc(v),
      _ => Self::default(),
    }
  }
}

fn parse_calc_factor_at<'i>(input: &mut Parser<'i, '_>, depth: u32) -> ParseResult<'i, CalcValue> {
  let location = input.current_source_location();
  if depth >= MAX_CALC_DEPTH {
    return Err(location.new_unexpected_token_error(Token::ParenthesisBlock));
  }

  if input.try_parse(|parser| parser.expect_delim('+')).is_ok() {
    return parse_calc_factor_at(input, depth + 1);
  }

  if input.try_parse(|parser| parser.expect_delim('-')).is_ok() {
    return Ok(match parse_calc_factor_at(input, depth + 1)? {
      CalcValue::Number(value) => CalcValue::Number(-value),
      CalcValue::Formula(formula) => CalcValue::Formula(formula.neg()),
    });
  }

  let token = input.next()?;

  match token {
    Token::Number { value, .. } => Ok(CalcValue::Number(*value)),
    Token::Percentage { unit_value, .. } => {
      Ok(CalcValue::Formula(CalcFormula::percent(*unit_value)))
    }
    Token::Dimension { value, unit, .. } => {
      match length_from_dimension_unit(unit.as_ref(), *value) {
        Some(length) => Ok(CalcValue::Formula(CalcFormula::from_length(length))),
        None => Err(unexpected_token!(Length, location, token)),
      }
    }
    Token::Function(name) if name.eq_ignore_ascii_case("calc") => {
      input.parse_nested_block(|input| parse_calc_sum_at(input, depth + 1))
    }
    Token::Ident(ident) => match_ignore_ascii_case! {ident.as_ref(),
      "e" => Ok(CalcValue::Number(std::f32::consts::E)),
      "pi" => Ok(CalcValue::Number(std::f32::consts::PI)),
      "infinity" => Ok(CalcValue::Number(f32::INFINITY)),
      "-infinity" => Ok(CalcValue::Number(f32::NEG_INFINITY)),
      "nan" => Ok(CalcValue::Number(f32::NAN)),
      _ => Err(unexpected_token!(Length, location, token)),
    },
    _ => Err(unexpected_token!(Length, location, token)),
  }
}
