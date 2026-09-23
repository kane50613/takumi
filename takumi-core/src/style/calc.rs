use std::{
  cell::RefCell,
  hash::{Hash, Hasher},
};

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

/// A `calc(...)` unit with a non-zero coefficient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CalcUnit {
  #[default]
  Px,
  Percent,
  Rem,
  Em,
  Lh,
  Rlh,
  Vh,
  Vw,
  Cqh,
  Cqw,
  CqMin,
  CqMax,
  VMin,
  VMax,
  Cm,
  Mm,
  Inch,
  Q,
  Pt,
  Pc,
}

impl CalcUnit {
  pub(crate) fn suffix(self) -> &'static str {
    match self {
      Self::Px => "px",
      Self::Percent => "%",
      Self::Rem => "rem",
      Self::Em => "em",
      Self::Lh => "lh",
      Self::Rlh => "rlh",
      Self::Vh => "vh",
      Self::Vw => "vw",
      Self::Cqh => "cqh",
      Self::Cqw => "cqw",
      Self::CqMin => "cqmin",
      Self::CqMax => "cqmax",
      Self::VMin => "vmin",
      Self::VMax => "vmax",
      Self::Cm => "cm",
      Self::Mm => "mm",
      Self::Inch => "in",
      Self::Q => "q",
      Self::Pt => "pt",
      Self::Pc => "pc",
    }
  }
}

/// One `value * unit` term of a compressed `calc(...)` expression.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CalcTerm {
  pub(crate) unit: CalcUnit,
  pub(crate) value: f32,
}

impl CalcTerm {
  /// The value as CSS serializes it: percentages store a fraction.
  pub(crate) fn display_value(self) -> f32 {
    match self.unit {
      CalcUnit::Percent => self.value * 100.0,
      _ => self.value,
    }
  }
}

/// A parsed `calc(...)` expression, compressed to its non-zero terms so it
/// stays inline in `Length`. Naive versus CSS: an expression mixing more than
/// [`MAX_CALC_TERMS`] distinct units fails to parse.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CalcTerms {
  units: [CalcUnit; MAX_CALC_TERMS],
  values: [f32; MAX_CALC_TERMS],
}

/// Distinct units one compressed `calc(...)` expression can carry.
pub(crate) const MAX_CALC_TERMS: usize = 4;

impl CalcTerms {
  /// The stored terms; trailing zero-valued slots are padding.
  pub(crate) fn terms(&self) -> impl Iterator<Item = CalcTerm> {
    self
      .units
      .into_iter()
      .zip(self.values)
      .filter(|(_, value)| *value != 0.0)
      .map(|(unit, value)| CalcTerm { unit, value })
  }

  pub(crate) fn neg(self) -> Self {
    Self {
      units: self.units,
      values: self.values.map(|value| -value),
    }
  }

  /// Collapses the terms into a `px + percent * basis` linear form.
  pub fn resolve(self, sizing: &SizingContext) -> CalcLinear {
    let viewport_width = sizing.viewport.unit_width();
    let viewport_height = sizing.viewport.unit_height();
    let container_width = sizing.query_container_width();
    let container_height = sizing.query_container_height();
    let mut absolute_css = 0.0;
    let mut px = 0.0;
    let mut percent = 0.0;

    for (unit, value) in self.units.into_iter().zip(self.values) {
      match unit {
        // Absolute units are authored in CSS px and cross the dpr boundary
        // once; every relative unit already resolves against a device-pixel
        // basis.
        CalcUnit::Px => absolute_css += value,
        CalcUnit::Cm => absolute_css += value * ONE_CM_IN_PX,
        CalcUnit::Mm => absolute_css += value * ONE_MM_IN_PX,
        CalcUnit::Inch => absolute_css += value * ONE_IN_PX,
        CalcUnit::Q => absolute_css += value * ONE_Q_IN_PX,
        CalcUnit::Pt => absolute_css += value * ONE_PT_IN_PX,
        CalcUnit::Pc => absolute_css += value * ONE_PC_IN_PX,
        CalcUnit::Percent => percent += value,
        CalcUnit::Rem => px += value * sizing.rem_basis(),
        CalcUnit::Em => px += value * sizing.font_size,
        CalcUnit::Lh => px += value * sizing.line_height,
        CalcUnit::Rlh => px += value * sizing.root_line_height_basis(),
        CalcUnit::Vh => px += value * viewport_height / 100.0,
        CalcUnit::Vw => px += value * viewport_width / 100.0,
        CalcUnit::Cqh => px += value * container_height / 100.0,
        CalcUnit::Cqw => px += value * container_width / 100.0,
        CalcUnit::CqMin => px += value * container_width.min(container_height) / 100.0,
        CalcUnit::CqMax => px += value * container_width.max(container_height) / 100.0,
        CalcUnit::VMin => px += value * viewport_width.min(viewport_height) / 100.0,
        CalcUnit::VMax => px += value * viewport_width.max(viewport_height) / 100.0,
      }
    }

    CalcLinear {
      px: sizing.to_device(absolute_css) + px,
      percent,
    }
  }

  /// Hashes each term's unit and value by bit pattern.
  pub(crate) fn hash_bits(&self, hasher: &mut impl Hasher) {
    for (unit, value) in self.units.into_iter().zip(self.values) {
      (unit as u8).hash(hasher);
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

  /// Compresses to the non-zero terms, or `None` when more than
  /// [`MAX_CALC_TERMS`] distinct units appear.
  pub(crate) fn compress(self) -> Option<CalcTerms> {
    let coefficients = [
      (CalcUnit::Px, self.px),
      (CalcUnit::Percent, self.percent),
      (CalcUnit::Rem, self.rem),
      (CalcUnit::Em, self.em),
      (CalcUnit::Lh, self.lh),
      (CalcUnit::Rlh, self.rlh),
      (CalcUnit::Vh, self.vh),
      (CalcUnit::Vw, self.vw),
      (CalcUnit::Cqh, self.cqh),
      (CalcUnit::Cqw, self.cqw),
      (CalcUnit::CqMin, self.cqmin),
      (CalcUnit::CqMax, self.cqmax),
      (CalcUnit::VMin, self.vmin),
      (CalcUnit::VMax, self.vmax),
      (CalcUnit::Cm, self.cm),
      (CalcUnit::Mm, self.mm),
      (CalcUnit::Inch, self.inch),
      (CalcUnit::Q, self.q),
      (CalcUnit::Pt, self.pt),
      (CalcUnit::Pc, self.pc),
    ];
    let mut terms = CalcTerms::default();
    let mut count = 0;

    for (unit, value) in coefficients {
      if value == 0.0 {
        continue;
      }
      if count == MAX_CALC_TERMS {
        return None;
      }
      terms.units[count] = unit;
      terms.values[count] = value;
      count += 1;
    }
    Some(terms)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CalcValue {
  Number(f32),
  Formula(CalcFormula),
}

impl CalcValue {
  /// Applies a binary operator, or `None` when the operand types rule it out.
  fn combine(self, operator: char, rhs: Self) -> Option<Self> {
    Some(match (operator, self, rhs) {
      ('+', Self::Number(lhs), Self::Number(rhs)) => Self::Number(lhs + rhs),
      ('+', Self::Formula(lhs), Self::Formula(rhs)) => Self::Formula(lhs.add(rhs)),
      ('-', Self::Number(lhs), Self::Number(rhs)) => Self::Number(lhs - rhs),
      ('-', Self::Formula(lhs), Self::Formula(rhs)) => Self::Formula(lhs.sub(rhs)),
      ('*', Self::Formula(formula), Self::Number(factor))
      | ('*', Self::Number(factor), Self::Formula(formula)) => Self::Formula(formula.scale(factor)),
      ('*', Self::Number(lhs), Self::Number(rhs)) => Self::Number(lhs * rhs),
      ('/', _, Self::Number(0.0)) => return None,
      ('/', Self::Formula(lhs), Self::Number(rhs)) => Self::Formula(lhs.scale(1.0 / rhs)),
      ('/', Self::Number(lhs), Self::Number(rhs)) => Self::Number(lhs / rhs),
      _ => return None,
    })
  }
}

// Matches Blink's `kMaxExpressionDepth` (css_math_expression_node.h).
const MAX_CALC_DEPTH: u32 = 100;

pub(crate) fn parse_calc_sum<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CalcValue> {
  parse_calc_sum_at(input, 0)
}

fn parse_calc_sum_at<'i>(input: &mut Parser<'i, '_>, depth: u32) -> ParseResult<'i, CalcValue> {
  parse_calc_chain(input, ['+', '-'], |input| {
    parse_calc_product_at(input, depth)
  })
}

/// Parses a `calc(...)` that must reduce to a unitless number.
pub(crate) fn parse_calc_number_expression<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, f32> {
  let location = input.current_source_location();
  let token = input.next()?.clone();

  if let Token::Function(function) = &token
    && function.eq_ignore_ascii_case("calc")
    && let CalcValue::Number(value) = input.parse_nested_block(parse_calc_sum)?
  {
    return Ok(value);
  }

  Err(location.new_unexpected_token_error(token))
}

fn parse_calc_product_at<'i>(input: &mut Parser<'i, '_>, depth: u32) -> ParseResult<'i, CalcValue> {
  parse_calc_chain(input, ['*', '/'], |input| {
    parse_calc_factor_at(input, depth)
  })
}

/// An operand followed by any number of `operators` and further operands, folded left.
fn parse_calc_chain<'i, 't>(
  input: &mut Parser<'i, 't>,
  operators: [char; 2],
  mut parse_operand: impl FnMut(&mut Parser<'i, 't>) -> ParseResult<'i, CalcValue>,
) -> ParseResult<'i, CalcValue> {
  let mut value = parse_operand(input)?;

  while let Some(operator) = parse_calc_operator(input, operators) {
    let rhs = parse_operand(input)?;

    value = value.combine(operator, rhs).ok_or_else(|| {
      unexpected_token!(
        Length,
        input.current_source_location(),
        &Token::Delim(operator),
      )
    })?;
  }

  Ok(value)
}

/// Consumes the next token when it is a delimiter among `operators`.
fn parse_calc_operator(input: &mut Parser<'_, '_>, operators: [char; 2]) -> Option<char> {
  input
    .try_parse(|input| match input.next() {
      Ok(&Token::Delim(operator)) if operators.contains(&operator) => Ok(operator),
      _ => Err(()),
    })
    .ok()
}

fn parse_calc_factor_at<'i>(input: &mut Parser<'i, '_>, depth: u32) -> ParseResult<'i, CalcValue> {
  let location = input.current_source_location();
  if depth >= MAX_CALC_DEPTH {
    return Err(location.new_unexpected_token_error(Token::ParenthesisBlock));
  }

  match parse_calc_operator(input, ['+', '-']) {
    Some('+') => return parse_calc_factor_at(input, depth + 1),
    Some('-') => {
      return Ok(match parse_calc_factor_at(input, depth + 1)? {
        CalcValue::Number(value) => CalcValue::Number(-value),
        CalcValue::Formula(formula) => CalcValue::Formula(formula.neg()),
      });
    }
    _ => {}
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
