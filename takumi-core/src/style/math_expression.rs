//! CSS math functions that do not reduce to the linear `px + %` form `calc()` keeps, such as
//! `min(10px, 20%)`, held as a tree until layout knows the percentage basis, as Blink holds a
//! `CSSMathExpressionNode`. A tree is interned, so a `Length` that names one stays `Copy`.
//! <https://drafts.csswg.org/css-values-4/#math>

use std::{
  collections::HashMap,
  f32::consts::{E, PI},
  fmt::{self, Write},
  slice,
  sync::{LazyLock, PoisonError, RwLock},
};

use cssparser::{ParseError as CssParseError, Parser, Token, match_ignore_ascii_case};

use crate::{
  layout::layout_style::LayoutLength,
  style::{
    Length, ParseResult, SizingContext, ToCss,
    calc::{CalcFormula, CalcValue},
    length_from_dimension_unit,
  },
};

/// Blink's `kMaxExpressionDepth`, from css_math_expression_node.h.
const MAX_DEPTH: u32 = 100;

/// How many distinct trees the process keeps. A value past it does not parse, which bounds the
/// memory a stream of distinct stylesheets can take.
const MAX_EXPRESSIONS: usize = 1 << 16;

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

/// The interned trees, and the index of each by its serialization.
#[derive(Default)]
struct Interner {
  nodes: Vec<Node>,
  indices: HashMap<String, u32>,
}

static INTERNER: LazyLock<RwLock<Interner>> = LazyLock::new(RwLock::default);

impl MathExpr {
  fn intern(node: Node) -> Option<Self> {
    let mut key = String::new();

    node.to_css(&mut key).ok()?;
    if let Some(&index) = INTERNER.read().ok()?.indices.get(&key) {
      return Some(Self(index));
    }

    let mut interner = INTERNER.write().ok()?;

    if let Some(&index) = interner.indices.get(&key) {
      return Some(Self(index));
    }
    if interner.nodes.len() >= MAX_EXPRESSIONS {
      return None;
    }

    let index = u32::try_from(interner.nodes.len()).ok()?;

    interner.nodes.push(node);
    interner.indices.insert(key, index);
    Some(Self(index))
  }

  fn with<R>(self, read: impl FnOnce(&Node) -> R) -> R {
    let interner = INTERNER.read().unwrap_or_else(PoisonError::into_inner);

    read(&interner.nodes[self.0 as usize])
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
    return Ok(Length::Px(value.finite()));
  }

  MathExpr::intern(node)
    .map(Length::Math)
    .ok_or_else(CssParseError::unexpected_token)
}

/// The number the math function `name` reads from `input`.
pub(crate) fn parse_number<'i>(name: &str, input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  match input
    .parse_nested_block(|input| parse_function(name, input, 0))?
    .eval(&CalcFormula::absolute_px)
  {
    Some(Value {
      raw,
      kind: Kind::NUMBER,
    }) => Ok(raw),
    _ => Err(CssParseError::unexpected_token()),
  }
}

/// The angle in degrees the math function `name` reads from `input`.
pub(crate) fn parse_angle<'i>(name: &str, input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  match input
    .parse_nested_block(|input| parse_function(name, input, 0))?
    .eval(&CalcFormula::absolute_px)
  {
    Some(Value {
      raw,
      kind: Kind::ANGLE,
    }) => Ok(finite(raw)),
    _ => Err(CssParseError::unexpected_token()),
  }
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
      let [low, value, high]: [Node; 3] = args(input)?.try_into().map_err(|_| invalid())?;

      Node::Clamp(Box::new([low, value, high]))
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
  let radians = match value.kind {
    Kind::NUMBER => value.raw,
    Kind::ANGLE => value.raw.to_radians(),
    _ => return None,
  };

  Some(match_ignore_ascii_case! { name,
    "sin" => Node::Number(radians.sin()),
    "cos" => Node::Number(radians.cos()),
    "tan" => Node::Number(radians.tan()),
    "asin" if number => Node::Angle(radians.asin().to_degrees()),
    "acos" if number => Node::Angle(radians.acos().to_degrees()),
    "atan" if number => Node::Angle(radians.atan().to_degrees()),
    "sqrt" if number => Node::Number(radians.sqrt()),
    "exp" if number => Node::Number(radians.exp()),
    _ => return None,
  })
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

fn parse_sum<'i>(input: &mut Parser<'i>, depth: u32) -> ParseResult<'i, Node> {
  let mut node = parse_product(input, depth)?;

  while let Some(operator) = operator(input, ['+', '-']) {
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

fn parse_product<'i>(input: &mut Parser<'i>, depth: u32) -> ParseResult<'i, Node> {
  let mut node = parse_factor(input, depth)?;

  while let Some(operator) = operator(input, ['*', '/']) {
    let rhs = parse_factor(input, depth)?;

    node = if operator == '*' {
      Node::Product(Box::new(node), Box::new(rhs))
    } else {
      Node::Quotient(Box::new(node), Box::new(rhs))
    };
  }
  Ok(node)
}

/// Consumes the next token when it is a delimiter among `operators`.
fn operator(input: &mut Parser<'_>, operators: [char; 2]) -> Option<char> {
  input
    .try_parse(|input| match input.next() {
      Ok(&Token::Delim(operator)) if operators.contains(&operator) => Ok(operator),
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
      Self::Sum(lhs, rhs) => (lhs.kind()? == rhs.kind()?).then(|| lhs.kind())?,
      Self::Product(lhs, rhs) => lhs.kind()?.times(rhs.kind()?),
      Self::Quotient(lhs, rhs) => lhs.kind()?.over(rhs.kind()?),
      Self::Negate(node) | Self::Abs(node) => node.kind(),
      Self::Min(nodes) | Self::Max(nodes) | Self::Hypot(nodes) => same(nodes),
      Self::Clamp(nodes) => same(&nodes[..]),
      Self::Round(_, value, step) | Self::Mod(value, step) | Self::Rem(value, step) => {
        (value.kind()? == step.kind()?).then(|| value.kind())?
      }
      Self::Sign(node) => node.kind().map(|_| Kind::NUMBER),
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
      Self::Min(nodes) => fold(nodes, length, f32::min)?,
      Self::Max(nodes) => fold(nodes, length, f32::max)?,
      Self::Clamp(nodes) => {
        let [low, value, high] = [&nodes[0], &nodes[1], &nodes[2]].map(|node| node.eval(length));
        let (low, value, high) = (low?, value?, high?);

        // The lower bound wins over the upper one.
        value.zip(high, f32::min)?.zip(low, f32::max)?
      }
      Self::Round(rounding, value, step) => {
        value.eval(length)?.zip(step.eval(length)?, |value, step| {
          rounding.round(value, step)
        })?
      }
      Self::Mod(value, step) => value.eval(length)?.zip(step.eval(length)?, |value, step| {
        if step.is_infinite() && value.is_finite() && value.signum() != step.signum() {
          step
        } else {
          value - step * (value / step).floor()
        }
      })?,
      Self::Rem(value, step) => value.eval(length)?.zip(step.eval(length)?, |value, step| {
        value - step * (value / step).trunc()
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
      Self::Hypot(nodes) => {
        let mut kind = None;
        let mut sum = 0.0;

        for node in nodes.iter() {
          let value = node.eval(length)?;

          kind = Some(value);
          sum += value.raw * value.raw;
        }
        kind?.map(|_| sum.sqrt())
      }
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

impl Rounding {
  /// `value` rounded to a multiple of `step`.
  /// <https://drafts.csswg.org/css-values-4/#round-func>
  fn round(self, value: f32, step: f32) -> f32 {
    if step == 0.0 {
      return f32::NAN;
    }
    if value.is_infinite() {
      return if step.is_infinite() { f32::NAN } else { value };
    }
    if step.is_infinite() {
      return match self {
        Self::Up if value > 0.0 => f32::INFINITY,
        Self::Down if value < 0.0 => f32::NEG_INFINITY,
        _ => 0.0_f32.copysign(value),
      };
    }

    let step = step.abs();
    let (lower, upper) = ((value / step).floor() * step, (value / step).ceil() * step);

    match self {
      Self::Up => upper,
      Self::Down => lower,
      Self::ToZero if value < 0.0 => upper,
      Self::ToZero => lower,
      // A half rounds up, toward positive infinity.
      Self::Nearest if value - lower < upper - value => lower,
      Self::Nearest => upper,
    }
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
    assert_eq!(Length::from_css_str("max(1in, 10px)"), Ok(Length::Px(96.0)));
    assert_eq!(serialized("min(10px, 20%)"), "min(10px, 20%)");
    assert_eq!(serialized("clamp(1em, 5%, 3rem)"), "clamp(1em, 5%, 3rem)");
  }
}
