//! CSS math functions that do not reduce to the linear `px + %` form `calc()` keeps, such as
//! `min(10px, 20%)`, held as a tree until layout knows the percentage basis, as Blink holds a
//! `CSSMathExpressionNode`. A tree is [interned](super::interner), so a `Length` that names one
//! stays `Copy`.
//! <https://drafts.csswg.org/css-values-4/#math>

use std::{
  f32::consts::{E, PI},
  fmt::{self, Write},
  slice,
  sync::{LazyLock, RwLock},
};

use cssparser::{ParseError as CssParseError, Parser, Token, match_ignore_ascii_case};

use crate::{
  layout::layout_style::LayoutLength,
  style::{
    Length, ParseResult, SizingContext, ToCss,
    calc::{CalcFormula, CalcUnit, CalcValue},
    interner::{self, Interner},
    length_from_dimension_unit,
  },
};

/// Blink's `kMaxExpressionDepth`, from css_math_expression_node.h.
const MAX_DEPTH: u32 = 100;

/// A math expression, by its index among the interned trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MathExpr(u32);

/// A node of a math expression tree.
#[derive(Debug, Clone, PartialEq)]
enum Node {
  Number(f32),
  /// Degrees.
  Angle(f32),
  /// A linear length and percentage, the fraction of the basis in `percent`.
  Length(CalcFormula),
  Sum(Box<Node>, Box<Node>),
  Product(Box<Node>, Box<Node>),
  Quotient(Box<Node>, Box<Node>),
  Negate(Box<Node>),
  Min(Box<[Node]>),
  Max(Box<[Node]>),
  Clamp(Box<[Node; 3]>),
  Round(Rounding, Box<Node>, Box<Node>),
  Mod(Box<Node>, Box<Node>),
  Rem(Box<Node>, Box<Node>),
  Abs(Box<Node>),
  Sign(Box<Node>),
  Hypot(Box<[Node]>),
}

/// The type a node computes to: the power of each base type, both zero for a number, so a
/// product adds them and a quotient subtracts them.
/// <https://drafts.csswg.org/css-values-4/#calc-type-checking>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Kind {
  length: i8,
  angle: i8,
}

impl Kind {
  const NUMBER: Self = Self {
    length: 0,
    angle: 0,
  };
  const LENGTH: Self = Self {
    length: 1,
    angle: 0,
  };
  const ANGLE: Self = Self {
    length: 0,
    angle: 1,
  };

  fn times(self, other: Self) -> Option<Self> {
    Some(Self {
      length: self.length.checked_add(other.length)?,
      angle: self.angle.checked_add(other.angle)?,
    })
  }

  fn over(self, other: Self) -> Option<Self> {
    Some(Self {
      length: self.length.checked_sub(other.length)?,
      angle: self.angle.checked_sub(other.angle)?,
    })
  }
}

/// What a node evaluates to: a length in px and an angle in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Value {
  raw: f32,
  kind: Kind,
}

/// The rounding strategy of `round()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rounding {
  Nearest,
  Up,
  Down,
  ToZero,
}

static TREES: LazyLock<RwLock<Interner<Node>>> = LazyLock::new(RwLock::default);

impl MathExpr {
  fn intern(node: Node) -> Option<Self> {
    let size = node.size();

    // A tree past the limit would take long to serialize as a key, so it stops first.
    if size > interner::MAX_PARTS {
      return None;
    }

    let mut key = String::new();

    node.to_css(&mut key).ok()?;
    interner::intern(&TREES, key, node, size).map(Self)
  }

  fn with<R>(self, read: impl FnOnce(&Node) -> R) -> R {
    interner::with(&TREES, self.0, read)
  }

  /// The length in device px against `sizing`, a percentage taking `basis`.
  pub(crate) fn to_px(self, sizing: &SizingContext, basis: f32) -> f32 {
    self.with(|node| {
      node
        .eval(&|formula| {
          let linear = formula.compress()?.resolve(sizing);

          Some(linear.px + linear.percent * basis)
        })
        .map_or(0.0, Value::finite)
    })
  }

  /// The length layout resolves against `sizing`: device px when no percentage takes part, and a
  /// tree of device px and percentages when one does.
  pub(crate) fn to_layout_length(self, sizing: &SizingContext) -> LayoutLength {
    // Interning takes the lock that reading holds, so the tree leaves it first.
    let node = self
      .with(Node::clone)
      .map_lengths(&|formula| resolve_units(formula, sizing, |px| px));

    if !node.has_percent() {
      return LayoutLength::Fixed(
        node
          .eval(&|formula| Some(formula.px))
          .map_or(0.0, Value::finite),
      );
    }

    Self::intern(node).map_or(LayoutLength::Auto, LayoutLength::Math)
  }

  /// The computed length: CSS px when no percentage takes part, and a tree of CSS px and
  /// percentages when one does.
  pub(crate) fn make_computed(self, sizing: &SizingContext) -> Length {
    let node = self
      .with(Node::clone)
      .map_lengths(&|formula| resolve_units(formula, sizing, |px| sizing.to_css(px)));

    if !node.has_percent() {
      return Length::Px(
        node
          .eval(&|formula| Some(formula.px))
          .map_or(0.0, Value::finite),
      );
    }

    Self::intern(node).map_or(Length::Auto, Length::Math)
  }

  /// A layout tree's length against `basis`, whose leaves are device px and percentages.
  pub(crate) fn resolve(self, basis: f32) -> f32 {
    self.with(|node| {
      node
        .eval(&|formula| Some(formula.px + formula.percent * basis))
        .map_or(0.0, Value::finite)
    })
  }

  /// The length with its sign flipped.
  pub(crate) fn negate(self) -> Length {
    Self::intern(Node::Negate(Box::new(self.with(Node::clone))))
      .map_or(Length::Px(0.0), Length::Math)
  }

  /// Whether a percentage takes part.
  pub(crate) fn has_percent(self) -> bool {
    self.with(Node::has_percent)
  }
}

impl ToCss for MathExpr {
  fn to_css<W: Write>(&self, dest: &mut W) -> fmt::Result {
    self.with(|node| node.to_css(dest))
  }
}

/// `formula` with its units resolved against `sizing` to the device px `px` converts, and its
/// percentage kept.
fn resolve_units(
  formula: &CalcFormula,
  sizing: &SizingContext,
  px: impl Fn(f32) -> f32,
) -> CalcFormula {
  formula
    .compress()
    .map_or_else(CalcFormula::default, |terms| {
      let linear = terms.resolve(sizing);

      CalcFormula::linear(px(linear.px), linear.percent)
    })
}

/// Whether `name` is a math function a value may open with.
pub(crate) fn is_math_function(name: &str) -> bool {
  MATH_FUNCTIONS
    .iter()
    .any(|function| name.eq_ignore_ascii_case(function))
}

const MATH_FUNCTIONS: [&str; 21] = [
  "calc", "min", "max", "clamp", "round", "mod", "rem", "abs", "sign", "sin", "cos", "tan", "asin",
  "acos", "atan", "atan2", "pow", "sqrt", "hypot", "log", "exp",
];

/// The length the math function `name` reads from `input`: a linear one as `calc()` keeps it, a
/// constant one as px, and any other as a tree.
pub(crate) fn parse_length<'i>(name: &str, input: &mut Parser<'i>) -> ParseResult<'i, Length> {
  let node = input.parse_nested_block(|input| parse_function(name, input, 0))?;

  match node.kind() {
    Some(Kind::LENGTH | Kind::NUMBER) => {}
    _ => return Err(CssParseError::unexpected_token()),
  }
  if let Some(linear) = node.linear() {
    return match linear {
      CalcValue::Number(value) => Ok(Length::Px(value)),
      CalcValue::Formula(formula) => formula
        .compress()
        .map(Length::Calc)
        .ok_or_else(CssParseError::unexpected_token),
    };
  }
  if let Some(value) = node.eval(&CalcFormula::absolute_px) {
    return CalcFormula::linear(value.finite(), 0.0)
      .compress()
      .map(Length::Calc)
      .ok_or_else(CssParseError::unexpected_token);
  }

  MathExpr::intern(node)
    .map(Length::Math)
    .ok_or_else(CssParseError::unexpected_token)
}

/// The number the math function `name` reads from `input`.
pub(crate) fn parse_number<'i>(name: &str, input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  parse_constant(name, input, Kind::NUMBER, &CalcFormula::absolute_px)
}

/// The percentage, as a fraction, the math function `name` reads from `input`.
pub(crate) fn parse_percentage<'i>(name: &str, input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  parse_constant(name, input, Kind::LENGTH, &|formula| {
    formula
      .compress()?
      .terms()
      .all(|term| term.unit == CalcUnit::Percent)
      .then_some(formula.percent)
  })
}

/// The angle in degrees the math function `name` reads from `input`.
pub(crate) fn parse_angle<'i>(name: &str, input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  parse_constant(name, input, Kind::ANGLE, &CalcFormula::absolute_px)
}

/// The value of `kind` the math function `name` reads from `input`, each length leaf as `length`
/// gives it.
fn parse_constant<'i>(
  name: &str,
  input: &mut Parser<'i>,
  kind: Kind,
  length: &dyn Fn(&CalcFormula) -> Option<f32>,
) -> ParseResult<'i, f32> {
  input
    .parse_nested_block(|input| parse_function(name, input, 0))?
    .eval(length)
    .filter(|value| value.kind == kind)
    .map(Value::finite)
    .ok_or_else(CssParseError::unexpected_token)
}

/// The arguments of the math function `name`, type-checked into one node.
fn parse_function<'i>(name: &str, input: &mut Parser<'i>, depth: u32) -> ParseResult<'i, Node> {
  if depth >= MAX_DEPTH {
    return Err(CssParseError::unexpected_token());
  }

  let invalid = || CssParseError::unexpected_token();
  let args = |input: &mut Parser<'i>| -> ParseResult<'i, Vec<Node>> {
    input.parse_comma_separated(|input| parse_sum(input, depth + 1))
  };
  let one = |input: &mut Parser<'i>| parse_sum(input, depth + 1);
  let node = match_ignore_ascii_case! { name,
    "calc" => one(input)?,
    "min" => Node::Min(args(input)?.into()),
    "max" => Node::Max(args(input)?.into()),
    "clamp" => {
      let bounds = input.parse_comma_separated(|input| {
        if input.try_parse(|input| input.expect_ident_matching("none")).is_ok() {
          return Ok(None);
        }

        parse_sum(input, depth + 1).map(Some)
      })?;
      let [low, Some(value), high]: [Option<Node>; 3] = bounds.try_into().map_err(|_| invalid())? else {
        return Err(invalid());
      };

      // An unbounded side leaves `min()` or `max()`, as Blink's `CSSMathExpressionOperation`
      // simplifies `clamp()`.
      match (low, high) {
        (Some(low), Some(high)) => Node::Clamp(Box::new([low, value, high])),
        (None, Some(high)) => Node::Min([value, high].into()),
        (Some(low), None) => Node::Max([low, value].into()),
        (None, None) => value,
      }
    },
    "round" => {
      let rounding = input
        .try_parse(|input| {
          let rounding = match_ignore_ascii_case! { &input.expect_ident()?.clone(),
            "nearest" => Rounding::Nearest,
            "up" => Rounding::Up,
            "down" => Rounding::Down,
            "to-zero" => Rounding::ToZero,
            _ => return Err(input.new_error_for_next_token::<()>()),
          };

          input.expect_comma()?;
          Ok(rounding)
        })
        .unwrap_or(Rounding::Nearest);
      let mut nodes = args(input)?;
      let step = match nodes.len() {
        // Only a number may leave the step out, which is then 1.
        1 if nodes[0].kind() == Some(Kind::NUMBER) => Node::Number(1.0),
        2 => nodes.pop().ok_or_else(invalid)?,
        _ => return Err(invalid()),
      };
      let value = nodes.pop().ok_or_else(invalid)?;

      Node::Round(rounding, Box::new(value), Box::new(step))
    },
    "mod" | "rem" | "atan2" | "pow" => {
      let [first, second]: [Node; 2] = args(input)?.try_into().map_err(|_| invalid())?;

      fold_binary(name, first, second).ok_or_else(invalid)?
    },
    "abs" => Node::Abs(Box::new(one(input)?)),
    "sign" => Node::Sign(Box::new(one(input)?)),
    "hypot" => Node::Hypot(args(input)?.into()),
    "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sqrt" | "exp" => {
      fold_unary(name, one(input)?).ok_or_else(invalid)?
    },
    "log" => {
      let nodes = args(input)?;
      let value = nodes.first().and_then(Node::constant_number).ok_or_else(invalid)?;
      let base = match nodes.get(1..) {
        Some([]) => E,
        Some([base]) => base.constant_number().ok_or_else(invalid)?,
        _ => return Err(invalid()),
      };

      Node::Number(value.ln() / base.ln())
    },
    _ => return Err(invalid()),
  };

  node.kind().map(|_| node).ok_or_else(invalid)
}

/// A function of one number or angle, computed now: both are constant.
fn fold_unary(name: &str, argument: Node) -> Option<Node> {
  let value = argument.eval(&CalcFormula::absolute_px)?;
  let number = value.kind == Kind::NUMBER;
  let degrees = match value.kind {
    Kind::NUMBER => value.raw.to_degrees(),
    Kind::ANGLE => value.raw,
    _ => return None,
  };

  Some(match_ignore_ascii_case! { name,
    "sin" => Node::Number(sin_cos_degrees(degrees).0),
    "cos" => Node::Number(sin_cos_degrees(degrees).1),
    "tan" => Node::Number(tan_degrees(degrees)),
    "asin" if number => Node::Angle(value.raw.asin().to_degrees()),
    "acos" if number => Node::Angle(value.raw.acos().to_degrees()),
    "atan" if number => Node::Angle(value.raw.atan().to_degrees()),
    "sqrt" if number => Node::Number(value.raw.sqrt()),
    "exp" if number => Node::Number(value.raw.exp()),
    _ => return None,
  })
}

/// The sine and cosine of `degrees`, exact at multiples of 90deg, as gfx's `SinCosDegrees`.
fn sin_cos_degrees(degrees: f32) -> (f32, f32) {
  let quadrants = degrees / 90.0;

  if degrees.abs() < 90_000_000.0 && quadrants.fract() == 0.0 {
    return [(0.0, 1.0), (1.0, 0.0), (0.0, -1.0), (-1.0, 0.0)][(quadrants as i32 & 3) as usize];
  }

  degrees.to_radians().sin_cos()
}

/// The tangent of `degrees`, exact at multiples of 45deg, as Blink's `TanDegrees`.
fn tan_degrees(degrees: f32) -> f32 {
  let octants = degrees / 45.0;

  if degrees.abs() < 90_000_000.0 && octants.fract() == 0.0 {
    return [
      0.0,
      1.0,
      f32::INFINITY,
      -1.0,
      0.0,
      1.0,
      f32::NEG_INFINITY,
      -1.0,
    ][(octants as i32 & 7) as usize];
  }

  degrees.to_radians().tan()
}

/// `mod()` and `rem()` as nodes, and `atan2()` and `pow()` computed now.
fn fold_binary(name: &str, first: Node, second: Node) -> Option<Node> {
  if first.kind()? != second.kind()? {
    return None;
  }

  match_ignore_ascii_case! { name,
    "mod" => Some(Node::Mod(Box::new(first), Box::new(second))),
    "rem" => Some(Node::Rem(Box::new(first), Box::new(second))),
    "atan2" => {
      let (y, x) = (first.eval(&CalcFormula::absolute_px)?, second.eval(&CalcFormula::absolute_px)?);

      Some(Node::Angle(y.raw.atan2(x.raw).to_degrees()))
    },
    "pow" => Some(Node::Number(
      first.constant_number()?.powf(second.constant_number()?),
    )),
    _ => None,
  }
}

fn parse_sum<'i>(input: &mut Parser<'i>, mut depth: u32) -> ParseResult<'i, Node> {
  let mut node = parse_product(input, depth)?;

  // Each operand of a chain nests the tree one level deeper, so a chain counts toward the depth.
  while let Some(operator) = sum_operator(input) {
    depth += 1;
    let rhs = parse_product(input, depth)?;

    node = if operator == '+' {
      Node::Sum(Box::new(node), Box::new(rhs))
    } else {
      Node::Sum(Box::new(node), Box::new(Node::Negate(Box::new(rhs))))
    };
  }
  node
    .kind()
    .map(|_| node)
    .ok_or_else(CssParseError::unexpected_token)
}

fn parse_product<'i>(input: &mut Parser<'i>, mut depth: u32) -> ParseResult<'i, Node> {
  let mut node = parse_factor(input, depth)?;

  while let Some(operator) = product_operator(input) {
    depth += 1;
    let rhs = parse_factor(input, depth)?;

    node = if operator == '*' {
      Node::Product(Box::new(node), Box::new(rhs))
    } else {
      Node::Quotient(Box::new(node), Box::new(rhs))
    };
  }
  Ok(node)
}

/// Consumes the next `+` or `-`, which needs whitespace on both sides.
/// <https://drafts.csswg.org/css-values-4/#calc-syntax>
fn sum_operator(input: &mut Parser<'_>) -> Option<char> {
  input
    .try_parse(|input| {
      let Ok(Token::WhiteSpace(_)) = input.next_including_whitespace() else {
        return Err(());
      };
      let operator = match input.next_including_whitespace() {
        Ok(&Token::Delim(operator @ ('+' | '-'))) => operator,
        _ => return Err(()),
      };

      match input.next_including_whitespace() {
        Ok(Token::WhiteSpace(_)) => Ok(operator),
        _ => Err(()),
      }
    })
    .ok()
}

/// Consumes the next `*` or `/`.
fn product_operator(input: &mut Parser<'_>) -> Option<char> {
  input
    .try_parse(|input| match input.next() {
      Ok(&Token::Delim(operator @ ('*' | '/'))) => Ok(operator),
      _ => Err(()),
    })
    .ok()
}

fn parse_factor<'i>(input: &mut Parser<'i>, depth: u32) -> ParseResult<'i, Node> {
  if depth >= MAX_DEPTH {
    return Err(CssParseError::unexpected_token());
  }

  let token = input.next()?.clone();

  match token {
    Token::Number { value, .. } => Ok(Node::Number(value)),
    Token::Percentage { unit_value, .. } => Ok(Node::Length(CalcFormula::linear(0.0, unit_value))),
    Token::Dimension { value, unit, .. } => {
      if let Some(degrees) = angle_degrees(value, &unit) {
        return Ok(Node::Angle(degrees));
      }

      length_from_dimension_unit(unit.as_ref(), value)
        .map(|length| Node::Length(CalcFormula::from_length(length)))
        .ok_or_else(CssParseError::unexpected_token)
    }
    Token::Function(name) if is_math_function(&name) => {
      input.parse_nested_block(|input| parse_function(&name, input, depth + 1))
    }
    Token::ParenthesisBlock => input.parse_nested_block(|input| parse_sum(input, depth + 1)),
    Token::Ident(ident) => match_ignore_ascii_case! { ident.as_ref(),
      "e" => Ok(Node::Number(E)),
      "pi" => Ok(Node::Number(PI)),
      "infinity" => Ok(Node::Number(f32::INFINITY)),
      "-infinity" => Ok(Node::Number(f32::NEG_INFINITY)),
      "nan" => Ok(Node::Number(f32::NAN)),
      _ => Err(CssParseError::unexpected_token()),
    },
    _ => Err(CssParseError::unexpected_token()),
  }
}

/// `value` in an angle `unit`, in degrees.
fn angle_degrees(value: f32, unit: &str) -> Option<f32> {
  Some(match_ignore_ascii_case! { unit,
    "deg" => value,
    "grad" => value * 0.9,
    "rad" => value.to_degrees(),
    "turn" => value * 360.0,
    _ => return None,
  })
}

impl Node {
  /// The type the node computes to, or `None` when its operands' types rule it out.
  /// <https://drafts.csswg.org/css-values-4/#calc-type-checking>
  fn kind(&self) -> Option<Kind> {
    let same = |nodes: &[Self]| {
      let mut kinds = nodes.iter().map(Self::kind);
      let first = kinds.next()??;

      kinds.all(|kind| kind == Some(first)).then_some(first)
    };

    match self {
      Self::Number(_) => Some(Kind::NUMBER),
      Self::Angle(_) => Some(Kind::ANGLE),
      Self::Length(_) => Some(Kind::LENGTH),
      Self::Sum(lhs, rhs) => {
        let kind = lhs.kind()?;

        (kind == rhs.kind()?).then_some(kind)
      }
      Self::Product(lhs, rhs) => lhs.kind()?.times(rhs.kind()?),
      Self::Quotient(lhs, rhs) => lhs.kind()?.over(rhs.kind()?),
      Self::Negate(node) | Self::Abs(node) => node.kind(),
      Self::Min(nodes) | Self::Max(nodes) | Self::Hypot(nodes) => same(nodes),
      Self::Clamp(nodes) => same(&nodes[..]),
      Self::Round(_, value, step) | Self::Mod(value, step) | Self::Rem(value, step) => {
        let kind = value.kind()?;

        (kind == step.kind()?).then_some(kind)
      }
      Self::Sign(node) => node.kind().map(|_| Kind::NUMBER),
    }
  }

  /// How many nodes the tree has.
  fn size(&self) -> usize {
    1 + match self {
      Self::Number(_) | Self::Angle(_) | Self::Length(_) => 0,
      Self::Sum(lhs, rhs)
      | Self::Product(lhs, rhs)
      | Self::Quotient(lhs, rhs)
      | Self::Round(_, lhs, rhs)
      | Self::Mod(lhs, rhs)
      | Self::Rem(lhs, rhs) => lhs.size() + rhs.size(),
      Self::Negate(node) | Self::Abs(node) | Self::Sign(node) => node.size(),
      Self::Min(nodes) | Self::Max(nodes) | Self::Hypot(nodes) => {
        nodes.iter().map(Self::size).sum()
      }
      Self::Clamp(nodes) => nodes.iter().map(Self::size).sum(),
    }
  }

  /// Whether a percentage takes part.
  fn has_percent(&self) -> bool {
    match self {
      Self::Number(_) | Self::Angle(_) => false,
      Self::Length(formula) => formula.percent != 0.0,
      Self::Sum(lhs, rhs)
      | Self::Product(lhs, rhs)
      | Self::Quotient(lhs, rhs)
      | Self::Round(_, lhs, rhs)
      | Self::Mod(lhs, rhs)
      | Self::Rem(lhs, rhs) => lhs.has_percent() || rhs.has_percent(),
      Self::Negate(node) | Self::Abs(node) | Self::Sign(node) => node.has_percent(),
      Self::Min(nodes) | Self::Max(nodes) | Self::Hypot(nodes) => {
        nodes.iter().any(Self::has_percent)
      }
      Self::Clamp(nodes) => nodes.iter().any(Self::has_percent),
    }
  }

  /// The node as `calc()` keeps it, when only sums and products by a number make it up.
  fn linear(&self) -> Option<CalcValue> {
    Some(match self {
      Self::Number(value) => CalcValue::Number(*value),
      Self::Length(formula) => CalcValue::Formula(*formula),
      Self::Sum(lhs, rhs) => lhs.linear()?.combine('+', rhs.linear()?)?,
      Self::Product(lhs, rhs) => lhs.linear()?.combine('*', rhs.linear()?)?,
      Self::Quotient(lhs, rhs) => lhs.linear()?.combine('/', rhs.linear()?)?,
      Self::Negate(node) => match node.linear()? {
        CalcValue::Number(value) => CalcValue::Number(-value),
        CalcValue::Formula(formula) => CalcValue::Formula(formula.neg()),
      },
      _ => return None,
    })
  }

  /// The number the node computes to, when it is a constant number.
  fn constant_number(&self) -> Option<f32> {
    let value = self.eval(&CalcFormula::absolute_px)?;

    (value.kind == Kind::NUMBER).then_some(value.raw)
  }

  /// The node with each length leaf replaced by what `length` makes of it.
  fn map_lengths(&self, length: &dyn Fn(&CalcFormula) -> CalcFormula) -> Self {
    let map = |node: &Self| Box::new(node.map_lengths(length));
    let all = |nodes: &[Self]| nodes.iter().map(|node| node.map_lengths(length)).collect();

    match self {
      Self::Number(_) | Self::Angle(_) => self.clone(),
      Self::Length(formula) => Self::Length(length(formula)),
      Self::Sum(lhs, rhs) => Self::Sum(map(lhs), map(rhs)),
      Self::Product(lhs, rhs) => Self::Product(map(lhs), map(rhs)),
      Self::Quotient(lhs, rhs) => Self::Quotient(map(lhs), map(rhs)),
      Self::Negate(node) => Self::Negate(map(node)),
      Self::Min(nodes) => Self::Min(all(nodes)),
      Self::Max(nodes) => Self::Max(all(nodes)),
      Self::Clamp(nodes) => Self::Clamp(Box::new(
        nodes.each_ref().map(|node| node.map_lengths(length)),
      )),
      Self::Round(rounding, value, step) => Self::Round(*rounding, map(value), map(step)),
      Self::Mod(value, step) => Self::Mod(map(value), map(step)),
      Self::Rem(value, step) => Self::Rem(map(value), map(step)),
      Self::Abs(node) => Self::Abs(map(node)),
      Self::Sign(node) => Self::Sign(map(node)),
      Self::Hypot(nodes) => Self::Hypot(all(nodes)),
    }
  }

  /// What the node evaluates to, each length leaf in px as `length` gives it, or `None` when one
  /// gives none.
  fn eval(&self, length: &dyn Fn(&CalcFormula) -> Option<f32>) -> Option<Value> {
    Some(match self {
      Self::Number(value) => Value::new(*value, Kind::NUMBER),
      Self::Angle(degrees) => Value::new(*degrees, Kind::ANGLE),
      Self::Length(formula) => Value::new(length(formula)?, Kind::LENGTH),
      Self::Sum(lhs, rhs) => lhs
        .eval(length)?
        .zip(rhs.eval(length)?, |lhs, rhs| lhs + rhs)?,
      Self::Product(lhs, rhs) => {
        let (lhs, rhs) = (lhs.eval(length)?, rhs.eval(length)?);

        Value::new(lhs.raw * rhs.raw, lhs.kind.times(rhs.kind)?)
      }
      Self::Quotient(lhs, rhs) => {
        let (lhs, rhs) = (lhs.eval(length)?, rhs.eval(length)?);

        Value::new(lhs.raw / rhs.raw, lhs.kind.over(rhs.kind)?)
      }
      Self::Negate(node) => node.eval(length)?.map(|value| -value),
      Self::Min(nodes) => fold(nodes, length, minimum)?,
      Self::Max(nodes) => fold(nodes, length, maximum)?,
      Self::Clamp(nodes) => {
        let [low, value, high] = [&nodes[0], &nodes[1], &nodes[2]].map(|node| node.eval(length));
        let (low, value, high) = (low?, value?, high?);

        // The lower bound wins over the upper one.
        value.zip(high, minimum)?.zip(low, maximum)?
      }
      Self::Round(rounding, value, step) => {
        value.eval(length)?.zip(step.eval(length)?, |value, step| {
          Step::Round(*rounding).apply(value, step)
        })?
      }
      Self::Mod(value, step) => value.eval(length)?.zip(step.eval(length)?, |value, step| {
        Step::Mod.apply(value, step)
      })?,
      Self::Rem(value, step) => value.eval(length)?.zip(step.eval(length)?, |value, step| {
        Step::Rem.apply(value, step)
      })?,
      Self::Abs(node) => node.eval(length)?.map(f32::abs),
      Self::Sign(node) => {
        let value = node.eval(length)?.raw;

        let sign = if value == 0.0 || value.is_nan() {
          value
        } else {
          value.signum()
        };

        Value::new(sign, Kind::NUMBER)
      }
      // Blink's `EvaluateHypotIteration`: a scaled hypotenuse, so no square overflows.
      Self::Hypot(nodes) => fold(nodes, length, |sum, value| {
        if sum.is_nan() || value.is_nan() {
          f32::NAN
        } else {
          sum.hypot(value)
        }
      })?
      .map(f32::abs),
    })
  }
}

/// `nodes` folded with `combine`, all of one type.
fn fold(
  nodes: &[Node],
  length: &dyn Fn(&CalcFormula) -> Option<f32>,
  combine: fn(f32, f32) -> f32,
) -> Option<Value> {
  let mut values = nodes.iter().map(|node| node.eval(length));
  let first = values.next()??;

  values.try_fold(first, |folded, value| folded.zip(value?, combine))
}

/// A function that steps its value by a multiple of a step, as Blink's
/// `EvaluateSteppedValueFunction`.
/// <https://drafts.csswg.org/css-values-4/#round-infinities>
#[derive(Clone, Copy)]
enum Step {
  Round(Rounding),
  Mod,
  Rem,
}

impl Step {
  fn apply(self, value: f32, step: f32) -> f32 {
    if value.is_nan() || step.is_nan() || step == 0.0 || (value.is_infinite() && step.is_infinite())
    {
      return f32::NAN;
    }

    let finite_step = step.is_finite();

    match self {
      Self::Round(_) if value % step == 0.0 || value.is_infinite() => value,
      Self::Mod | Self::Rem if value.is_infinite() => f32::NAN,
      Self::Round(rounding) => {
        let (lower, upper) = nearest_multiples(value, step);
        let negative = value.is_sign_negative();

        match rounding {
          Rounding::Nearest if !finite_step => 0.0_f32.copysign(value),
          Rounding::Nearest => {
            let (lower, upper) = if value < 0.0 {
              (upper, lower)
            } else {
              (lower, upper)
            };
            let distance = (value % step).abs();
            let half = step.abs() / 2.0;

            if distance < half || (value < 0.0 && distance == half) {
              lower
            } else {
              upper
            }
          }
          Rounding::Up if !finite_step && value == 0.0 => value,
          Rounding::Up if !finite_step => {
            if negative {
              -0.0
            } else {
              f32::INFINITY
            }
          }
          Rounding::Up => upper,
          Rounding::Down if !finite_step && value == 0.0 => value,
          Rounding::Down if !finite_step => {
            if negative {
              f32::NEG_INFINITY
            } else {
              0.0
            }
          }
          Rounding::Down => lower,
          Rounding::ToZero if !finite_step => 0.0_f32.copysign(value),
          Rounding::ToZero if upper.abs() < lower.abs() => upper,
          Rounding::ToZero => lower,
        }
      }
      Self::Mod => {
        if !finite_step && value.is_sign_negative() != step.is_sign_negative() {
          return f32::NAN;
        }

        let remainder = value % step;

        if remainder == 0.0 {
          0.0_f32.copysign(step)
        } else if remainder.is_sign_negative() != step.is_sign_negative() {
          remainder + step
        } else {
          remainder
        }
      }
      Self::Rem => value % step,
    }
  }
}

/// The multiples of `step` either side of `value`, as Blink's `GetNearestMultiples`.
fn nearest_multiples(value: f32, step: f32) -> (f32, f32) {
  let (mut a, mut b) = (value.abs(), step.abs());
  let mut c = -(a % b);
  let lower = a + c;

  // Adding by increasing magnitude keeps a small term from rounding away.
  if a.abs() > c.abs() {
    (a, c) = (c, a);
  }
  if a.abs() > b.abs() {
    (a, b) = (b, a);
  }
  if b.abs() > c.abs() {
    (b, c) = (c, b);
  }

  let upper = a + b + c;

  if value < 0.0 {
    (-upper, -lower)
  } else {
    (lower, upper)
  }
}

/// The smaller value, NaN when either is, and -0 below +0.
fn minimum(a: f32, b: f32) -> f32 {
  if a.is_nan() || b.is_nan() {
    f32::NAN
  } else if a == 0.0 && b == 0.0 && (a.is_sign_negative() || b.is_sign_negative()) {
    -0.0
  } else {
    a.min(b)
  }
}

/// The larger value, NaN when either is, and +0 above -0.
fn maximum(a: f32, b: f32) -> f32 {
  if a.is_nan() || b.is_nan() {
    f32::NAN
  } else if a == 0.0 && b == 0.0 && (a.is_sign_positive() || b.is_sign_positive()) {
    0.0
  } else {
    a.max(b)
  }
}

impl Value {
  fn new(raw: f32, kind: Kind) -> Self {
    Self { raw, kind }
  }

  fn map(self, apply: impl Fn(f32) -> f32) -> Self {
    Self::new(apply(self.raw), self.kind)
  }

  /// Both values combined, when they have one type.
  fn zip(self, other: Self, combine: impl Fn(f32, f32) -> f32) -> Option<Self> {
    (self.kind == other.kind).then(|| Self::new(combine(self.raw, other.raw), self.kind))
  }

  fn finite(self) -> f32 {
    finite(self.raw)
  }
}

/// `value` as a property takes it: NaN as 0, and infinities as the largest finite values.
/// <https://drafts.csswg.org/css-values-4/#top-level-calculation>
fn finite(value: f32) -> f32 {
  if value.is_nan() {
    0.0
  } else {
    value.clamp(f32::MIN, f32::MAX)
  }
}

impl ToCss for Node {
  fn to_css<W: Write>(&self, dest: &mut W) -> fmt::Result {
    let list = |dest: &mut W, name: &str, nodes: &[Self]| {
      write!(dest, "{name}(")?;
      for (index, node) in nodes.iter().enumerate() {
        if index > 0 {
          dest.write_str(", ")?;
        }
        node.to_css(dest)?;
      }
      dest.write_char(')')
    };
    let binary = |dest: &mut W, open: &str, lhs: &Self, separator: &str, rhs: &Self| {
      dest.write_str(open)?;
      lhs.to_css(dest)?;
      dest.write_str(separator)?;
      rhs.to_css(dest)?;
      dest.write_char(')')
    };

    match self {
      Self::Number(value) => value.to_css(dest),
      Self::Angle(degrees) => {
        degrees.to_css(dest)?;
        dest.write_str("deg")
      }
      Self::Length(formula) => {
        let Some(terms) = formula.compress() else {
          return dest.write_str("0px");
        };
        let mut single = terms.terms();

        match (single.next(), single.next()) {
          (Some(term), None) => {
            term.display_value().to_css(dest)?;
            dest.write_str(term.unit.suffix())
          }
          _ => Length::Calc(terms).to_css(dest),
        }
      }
      Self::Sum(lhs, rhs) => binary(dest, "calc(", lhs, " + ", rhs),
      Self::Product(lhs, rhs) => binary(dest, "calc(", lhs, " * ", rhs),
      Self::Quotient(lhs, rhs) => binary(dest, "calc(", lhs, " / ", rhs),
      Self::Negate(node) => binary(dest, "calc(", &Self::Number(-1.0), " * ", node),
      Self::Min(nodes) => list(dest, "min", nodes),
      Self::Max(nodes) => list(dest, "max", nodes),
      Self::Clamp(nodes) => list(dest, "clamp", &nodes[..]),
      Self::Round(rounding, value, step) => {
        dest.write_str("round(")?;
        dest.write_str(match rounding {
          Rounding::Nearest => "",
          Rounding::Up => "up, ",
          Rounding::Down => "down, ",
          Rounding::ToZero => "to-zero, ",
        })?;
        binary(dest, "", value, ", ", step)
      }
      Self::Mod(value, step) => binary(dest, "mod(", value, ", ", step),
      Self::Rem(value, step) => binary(dest, "rem(", value, ", ", step),
      Self::Abs(node) => list(dest, "abs", slice::from_ref(node)),
      Self::Sign(node) => list(dest, "sign", slice::from_ref(node)),
      Self::Hypot(nodes) => list(dest, "hypot", nodes),
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::style::{
    Angle, FlexGrow, FontWeight, FromCssStr, Length, Order, PercentageNumber, ToCss, ZIndex,
    parse_calc_number_expression,
  };
  use cssparser::Parser;

  fn number(source: &str) -> Option<f32> {
    parse_calc_number_expression(&mut Parser::new(source)).ok()
  }

  /// A number context takes every function that reduces to a number.
  #[test]
  fn numbers() {
    assert_eq!(number("min(2, 3, 1)"), Some(1.0));
    assert_eq!(number("clamp(0, 1.5, 1)"), Some(1.0));
    assert_eq!(number("round(2.5)"), Some(3.0));
    assert_eq!(number("round(down, 2.5)"), Some(2.0));
    assert_eq!(number("pow(2, 10)"), Some(1024.0));
    assert_eq!(number("calc(1em / 1px)"), None);
    assert_eq!(number("min(1px, 2)"), None);
  }

  /// The number properties take a math function, an integer one rounding as CSS rounds.
  #[test]
  fn number_properties() {
    assert_eq!(
      PercentageNumber::from_css_str("min(0.5, 1)"),
      Ok(PercentageNumber(0.5))
    );
    assert_eq!(FlexGrow::from_css_str("max(1, 2)"), Ok(FlexGrow(2.0)));
    assert_eq!(ZIndex::from_css_str("calc(2.5)"), Ok(ZIndex::Integer(3)));
    assert_eq!(ZIndex::from_css_str("calc(-2.5)"), Ok(ZIndex::Integer(-2)));
    assert_eq!(Order::from_css_str("round(down, 7, 2)"), Ok(Order(6)));
    assert_eq!(
      FontWeight::from_css_str("calc(1200)"),
      Ok(FontWeight::from(1000.0))
    );
    assert_eq!(
      FontWeight::from_css_str("calc(NaN)"),
      Ok(FontWeight::from(1.0))
    );
    assert_eq!(FlexGrow::from_css_str("calc(-1)"), Ok(FlexGrow(0.0)));
    assert_eq!(
      PercentageNumber::from_css_str("min(50%, 75%)"),
      Ok(PercentageNumber(0.5))
    );
    assert!(PercentageNumber::from_css_str("calc(50% * 50%)").is_err());
    assert_eq!(
      ZIndex::from_css_str("calc(8388609)"),
      Ok(ZIndex::Integer(8_388_609))
    );
  }

  /// A constant calculation keeps its identity, so a negative one is not rejected as a literal is.
  #[test]
  fn a_negative_calculation_is_not_a_negative_literal() {
    let length = Length::from_css_str("min(-10px, -20px)").expect("the length parses");

    assert!(!length.is_negative());
    assert_eq!(length.to_css_string(), "calc(-20px)");
    assert!(Length::from_css_str("max(200px, 20%)").is_ok_and(Length::has_percentage));
  }

  /// A tree past the size limit does not intern, however shallow.
  #[test]
  fn a_wide_tree_stops_at_the_size_limit() {
    let wide = |terms: usize| format!("min({}, 20%)", vec!["1px"; terms].join(", "));

    assert!(matches!(
      Length::from_css_str(&wide(100)),
      Ok(Length::Math(_))
    ));
    assert!(Length::from_css_str(&wide(5_000)).is_err());
  }

  /// A long chain counts toward the depth limit, so it cannot nest the tree past it.
  #[test]
  fn a_long_chain_stops_at_the_depth_limit() {
    let chain = |terms: usize| format!("min({}, 20%)", vec!["1px"; terms].join(" + "));

    assert!(matches!(
      Length::from_css_str(&chain(50)),
      Ok(Length::Math(_))
    ));
    assert!(Length::from_css_str(&chain(100_000)).is_err());
  }

  /// An angle context takes a math function that reduces to an angle.
  #[test]
  fn angles() {
    let degrees = |source: &str| Angle::from_css_str(source).map(|angle| *angle).ok();

    assert_eq!(degrees("calc(1turn / 4)"), Some(90.0));
    assert_eq!(degrees("atan2(1, 1)"), Some(45.0));
    assert_eq!(degrees("max(10deg, 0.1rad)"), Some(10.0));
    assert_eq!(degrees("calc(10deg * 2px)"), None);
  }

  /// A length that needs the percentage basis keeps its tree, and serializes back to it.
  #[test]
  fn lengths_keep_a_tree_only_when_they_need_one() {
    let serialized = |source: &str| {
      let mut css = String::new();

      Length::from_css_str(source)
        .expect("the length parses")
        .to_css(&mut css)
        .expect("the length serializes");
      css
    };

    assert!(matches!(
      Length::from_css_str("min(10px, 20%)"),
      Ok(Length::Math(_))
    ));
    assert!(matches!(
      Length::from_css_str("calc(10px + 20%)"),
      Ok(Length::Calc(_))
    ));
    assert_eq!(serialized("max(1in, 10px)"), "calc(96px)");
    assert_eq!(serialized("min(10px, 20%)"), "min(10px, 20%)");
    assert_eq!(serialized("clamp(1em, 5%, 3rem)"), "clamp(1em, 5%, 3rem)");
  }
}
