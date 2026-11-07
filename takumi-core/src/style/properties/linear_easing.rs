//! The `linear()` easing function: a piecewise linear curve through its control points.
//! <https://drafts.csswg.org/css-easing-2/#the-linear-easing-function>

use std::{
  fmt::{self, Write},
  sync::{LazyLock, RwLock},
};

use cssparser::{ParseError as CssParseError, Parser, Token};

use crate::style::{
  ParseResult, ToCss,
  interner::{self, Interner},
  lerp,
};

/// A `linear()` curve, by its index among the interned curves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinearEasing(u32);

/// A control point: the output at an input progress.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
  output: f32,
  input: f32,
}

static CURVES: LazyLock<RwLock<Interner<Box<[Point]>>>> = LazyLock::new(RwLock::default);

impl LinearEasing {
  /// The curve the arguments of `linear()` in `input` name, its points canonicalized as
  /// css-easing-2 has it.
  pub(crate) fn parse<'i>(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let mut points: Vec<(f32, Option<f32>)> = Vec::new();

    for stop in input.parse_comma_separated(parse_stop)? {
      points.extend(stop);
    }
    if !(2..=interner::MAX_PARTS).contains(&points.len()) {
      return Err(CssParseError::unexpected_token());
    }

    let points = canonicalize(points);
    let parts = points.len();
    let mut key = String::new();

    write_points(&points, &mut key).map_err(|_| CssParseError::unexpected_token())?;
    interner::intern(&CURVES, key, points.into(), parts)
      .map(Self)
      .ok_or_else(CssParseError::unexpected_token)
  }

  /// The output progress at `progress`, extrapolated from the outermost points past either end.
  pub(crate) fn apply(self, progress: f32) -> f32 {
    interner::with(&CURVES, self.0, |points| {
      let last = points.len() - 1;
      let start = points
        .iter()
        .rposition(|point| point.input <= progress)
        .unwrap_or(0)
        .min(last - 1);
      let (from, to) = (points[start], points[start + 1]);

      if from.input == to.input {
        return to.output;
      }

      lerp(
        from.output,
        to.output,
        (progress - from.input) / (to.input - from.input),
      )
    })
  }
}

impl ToCss for LinearEasing {
  fn to_css<W: Write>(&self, dest: &mut W) -> fmt::Result {
    interner::with(&CURVES, self.0, |points| {
      dest.write_str("linear(")?;
      write_points(points, dest)?;
      dest.write_char(')')
    })
  }
}

/// One stop: a number and up to two percentages, in either order, each percentage a point.
fn parse_stop<'i>(input: &mut Parser<'i>) -> ParseResult<'i, Vec<(f32, Option<f32>)>> {
  let mut output = None;
  let mut inputs = Vec::new();

  while !input.is_exhausted() {
    match *input.next()? {
      Token::Number { value, .. } if output.is_none() => output = Some(value),
      Token::Percentage { unit_value, .. } if inputs.len() < 2 => inputs.push(unit_value),
      _ => return Err(CssParseError::unexpected_token()),
    }
  }

  let output = output.ok_or_else(CssParseError::unexpected_token)?;

  Ok(if inputs.is_empty() {
    vec![(output, None)]
  } else {
    inputs
      .into_iter()
      .map(|input| (output, Some(input)))
      .collect()
  })
}

/// The points with every input set: the first at 0 and the last at 1 when they have none, each
/// at least the one before, and a run without inputs spread evenly between its neighbours.
fn canonicalize(points: Vec<(f32, Option<f32>)>) -> Vec<Point> {
  let last = points.len() - 1;
  let mut inputs: Vec<Option<f32>> = points.iter().map(|&(_, input)| input).collect();

  inputs[0] = inputs[0].or(Some(0.0));
  inputs[last] = inputs[last].or(Some(1.0));

  let mut largest = f32::NEG_INFINITY;

  for input in inputs.iter_mut().flatten() {
    largest = largest.max(*input);
    *input = largest;
  }

  let mut index = 0;

  while index < last {
    let run_end = (index + 1..=last)
      .find(|&end| inputs[end].is_some())
      .unwrap_or(last);
    let (from, to) = (inputs[index].unwrap_or(0.0), inputs[run_end].unwrap_or(1.0));
    let steps = (run_end - index) as f32;

    for (offset, input) in inputs[index + 1..run_end].iter_mut().enumerate() {
      *input = Some(lerp(from, to, (offset + 1) as f32 / steps));
    }
    index = run_end;
  }

  points
    .iter()
    .zip(inputs)
    .map(|(&(output, _), input)| Point {
      output,
      input: input.unwrap_or(0.0),
    })
    .collect()
}

fn write_points<W: Write>(points: &[Point], dest: &mut W) -> fmt::Result {
  for (index, point) in points.iter().enumerate() {
    if index > 0 {
      dest.write_str(", ")?;
    }
    significant(point.output).to_css(dest)?;
    dest.write_char(' ')?;
    significant(point.input * 100.0).to_css(dest)?;
    dest.write_char('%')?;
  }
  Ok(())
}

/// `value` to the six significant digits Blink serializes a number with.
fn significant(value: f32) -> f32 {
  if value == 0.0 || !value.is_finite() {
    return value;
  }

  let scale = 10_f64.powi(5 - f64::from(value).abs().log10().floor() as i32);

  ((f64::from(value) * scale).round() / scale) as f32
}
