use cssparser::{ParseError as CssParseError, Parser, Token};

use crate::style::{
  Length, ONE_CM_IN_PX, ONE_IN_PX, ONE_MM_IN_PX, ONE_PC_IN_PX, ONE_PT_IN_PX, ONE_Q_IN_PX,
  ParseResult, SizingContext, is_math_function, parse_number, parse_percentage,
};

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
  pub(crate) ch: f32,
  pub(crate) ex: f32,
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
  Ch,
  Ex,
  Vh,
  Vw,
  Cqh,
  Cqw,
  CqMin,
  CqMax,
  VMin,
  VMax,
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
      Self::Ch => "ch",
      Self::Ex => "ex",
      Self::Vh => "vh",
      Self::Vw => "vw",
      Self::Cqh => "cqh",
      Self::Cqw => "cqw",
      Self::CqMin => "cqmin",
      Self::CqMax => "cqmax",
      Self::VMin => "vmin",
      Self::VMax => "vmax",
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

/// A `calc()` expression's non-zero terms. Naive: more than [`MAX_CALC_TERMS`] units fail to parse.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CalcTerms {
  units: [CalcUnit; MAX_CALC_TERMS],
  values: [f32; MAX_CALC_TERMS],
}

/// Distinct units one compressed `calc(...)` expression can carry.
pub(crate) const MAX_CALC_TERMS: usize = 4;

impl CalcTerms {
  /// `100%` less `offset`, an offset from the far edge measured from the near one, or `None`
  /// when it needs more than [`MAX_CALC_TERMS`] units.
  pub(crate) fn from_far_edge(offset: Length) -> Option<Self> {
    let offset = match offset {
      Length::Auto => return None,
      Length::Percentage(percent) => CalcFormula::percent(percent / 100.0).compress()?,
      Length::Calc(terms) => terms,
      length => CalcFormula::from_length(length).compress()?,
    };
    let percent = 1.0
      - offset
        .terms()
        .filter(|term| term.unit == CalcUnit::Percent)
        .map(|term| term.value)
        .sum::<f32>();
    let mut terms = Self::default();
    let others = offset.terms().filter(|term| term.unit != CalcUnit::Percent);

    // The percentage leads, as Blink serializes `calc(100% - 2px)`.
    for (index, term) in [CalcTerm {
      unit: CalcUnit::Percent,
      value: percent,
    }]
    .into_iter()
    .chain(others.map(|term| CalcTerm {
      value: -term.value,
      ..term
    }))
    .filter(|term| term.value != 0.0)
    .enumerate()
    {
      *terms.units.get_mut(index)? = term.unit;
      terms.values[index] = term.value;
    }
    Some(terms)
  }

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
        // Only absolute units scale by the device pixel ratio here.
        CalcUnit::Px => absolute_css += value,
        CalcUnit::Percent => percent += value,
        CalcUnit::Rem => px += value * sizing.rem_basis(),
        CalcUnit::Em => px += value * sizing.font_size,
        CalcUnit::Lh => px += value * sizing.line_height,
        CalcUnit::Rlh => px += value * sizing.root_line_height_basis(),
        CalcUnit::Ch => px += value * sizing.ch,
        CalcUnit::Ex => px += value * sizing.ex,
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
}

/// Invokes `$callback!` with every `CalcFormula` field, keeping constructors and ops in sync.
macro_rules! for_each_unit {
  ($callback:ident) => {
    $callback!(
      px, percent, rem, em, lh, rlh, ch, ex, vh, vw, cqh, cqw, cqmin, cqmax, vmin, vmax, cm, mm,
      inch, q, pt, pc
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

  /// `px` CSS px plus the fraction `percent` of the basis.
  pub(crate) fn linear(px: f32, percent: f32) -> Self {
    Self {
      px,
      ..Self::percent(percent)
    }
  }

  /// The formula in CSS px, when only absolute units make it up.
  pub(crate) fn absolute_px(&self) -> Option<f32> {
    let mut px = 0.0;

    for term in self.compress()?.terms() {
      if term.unit != CalcUnit::Px {
        return None;
      }
      px += term.value;
    }
    Some(px)
  }

  /// Bridges a single-unit `Length` to its symbolic `calc(...)` coefficient.
  pub(crate) fn from_length(length: Length) -> Self {
    match length {
      Length::Px(v) => Self::px(v),
      Length::Em(v) => Self::em(v),
      Length::Rem(v) => Self::rem(v),
      Length::Lh(v) => Self::lh(v),
      Length::Rlh(v) => Self::rlh(v),
      Length::Ch(v) => Self::ch(v),
      Length::Ex(v) => Self::ex(v),
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

  /// The non-zero terms, absolute lengths summed into px, or `None` past [`MAX_CALC_TERMS`].
  pub(crate) fn compress(self) -> Option<CalcTerms> {
    let px = self.px
      + self.cm * ONE_CM_IN_PX
      + self.mm * ONE_MM_IN_PX
      + self.inch * ONE_IN_PX
      + self.q * ONE_Q_IN_PX
      + self.pt * ONE_PT_IN_PX
      + self.pc * ONE_PC_IN_PX;
    let coefficients = [
      (CalcUnit::Px, px),
      (CalcUnit::Percent, self.percent),
      (CalcUnit::Rem, self.rem),
      (CalcUnit::Em, self.em),
      (CalcUnit::Lh, self.lh),
      (CalcUnit::Rlh, self.rlh),
      (CalcUnit::Ch, self.ch),
      (CalcUnit::Ex, self.ex),
      (CalcUnit::Vh, self.vh),
      (CalcUnit::Vw, self.vw),
      (CalcUnit::Cqh, self.cqh),
      (CalcUnit::Cqw, self.cqw),
      (CalcUnit::CqMin, self.cqmin),
      (CalcUnit::CqMax, self.cqmax),
      (CalcUnit::VMin, self.vmin),
      (CalcUnit::VMax, self.vmax),
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
  pub(crate) fn combine(self, operator: char, rhs: Self) -> Option<Self> {
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

/// Parses a `calc(...)` that must reduce to a unitless number.
pub(crate) fn parse_calc_number_expression<'i>(input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  let token = input.next()?.clone();

  match &token {
    Token::Function(function) if is_math_function(function) => parse_number(function, input),
    _ => Err(CssParseError::unexpected_token()),
  }
}

/// A `<number>`, or a math function that reduces to one.
pub(crate) fn parse_number_value<'i>(input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  match input.try_parse(Parser::expect_number) {
    Ok(number) => Ok(number),
    Err(_) => parse_calc_number_expression(input),
  }
}

/// A `<percentage>`, or a math function that reduces to one, as a fraction.
pub(crate) fn parse_percentage_value<'i>(input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  let token = input.next()?.clone();

  match token {
    Token::Percentage { unit_value, .. } => Ok(unit_value),
    Token::Function(name) if is_math_function(&name) => parse_percentage(&name, input),
    _ => Err(CssParseError::unexpected_token()),
  }
}

/// An `<integer>`, or a math function that reduces to a number, rounded as CSS rounds one: a
/// half toward positive infinity.
/// <https://drafts.csswg.org/css-values-4/#calc-type-checking>
pub(crate) fn parse_integer_value<'i>(input: &mut Parser<'i>) -> ParseResult<'i, i32> {
  match input.try_parse(Parser::expect_integer) {
    Ok(integer) => Ok(integer),
    Err(_) => {
      parse_calc_number_expression(input).map(|number| (f64::from(number) + 0.5).floor() as i32)
    }
  }
}
