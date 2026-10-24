//! Compact SVG path `d` data, as SVG output and the paint document write it.

use std::{
  f64::consts::{FRAC_PI_2, TAU},
  fmt::{self, Write as _},
};

use crate::{
  geometry::{PathCommand, Point, Rect},
  style::Affine,
};

/// Numbers a single path command serializes (a cubic carries three coordinate
/// pairs), used with [`APPROX_CHARS_PER_NUMBER`] to presize the path buffer.
const NUMBERS_PER_COMMAND: usize = 6;

/// Builds compact SVG path `d` data: numbers are quantized via [`Num`], the
/// command letter is elided when it repeats (and a moveto's trailing pairs fold
/// into implicit linetos), and the separator between two numbers is dropped
/// whenever the grammar still lexes them apart (the next number starts with `-`,
/// or with `.` when the previous number already carried a `.`).
pub struct PathData {
  out: String,
  prev_was_number: bool,
  prev_had_dot: bool,
  last_command: Option<u8>,
}

impl PathData {
  /// Path data commands up to the first error, and whether the whole text parsed, after Blink's
  /// `SVGPathParser` and `SVGPathNormalizer`: absolute commands, `H`/`V` as lines, smooth curves
  /// expanded, arcs as cubics.
  ///
  /// A coordinate that overflows `f32` ends the data as an error.
  pub fn parse(text: &str) -> (Vec<PathCommand>, bool) {
    let mut scanner = Scanner::new(text);
    let mut normalizer = Normalizer::default();
    let mut previous_command = None;

    loop {
      scanner.skip_spaces();

      if scanner.at_end() {
        return (normalizer.commands, true);
      }

      let Some((absolute, segment)) = scanner.segment(&mut previous_command) else {
        return (normalizer.commands, false);
      };

      if !normalizer.push(absolute, segment) {
        return (normalizer.commands, false);
      }
    }
  }

  /// An empty path with room for `capacity` bytes.
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      out: String::with_capacity(capacity),
      prev_was_number: false,
      prev_had_dot: false,
      last_command: None,
    }
  }

  /// Writes a command letter unless the grammar lets it repeat implicitly. Pairs after a moveto
  /// are linetos, so a second moveto keeps its letter.
  pub fn command(&mut self, letter: u8) {
    let elide = matches!(self.last_command, Some(prev) if (prev == letter && !matches!(letter, b'M' | b'm'))
      || (prev == b'M' && letter == b'L')
      || (prev == b'm' && letter == b'l'));
    if !elide {
      self.out.push(letter as char);
      self.prev_was_number = false;
    }
    self.last_command = Some(letter);
  }

  /// Writes a number.
  pub fn number(&mut self, value: f32) {
    let mut buf = NumBuf::new();
    let text = Num(value).format(&mut buf);
    let first = text.as_bytes()[0];
    if self.prev_was_number && first != b'-' && !(first == b'.' && self.prev_had_dot) {
      self.out.push(' ');
    }
    self.out.push_str(text);
    self.prev_was_number = true;
    self.prev_had_dot = text.contains('.');
  }

  /// Writes a coordinate pair.
  pub fn pair(&mut self, x: f32, y: f32) {
    self.number(x);
    self.number(y);
  }

  /// Closes the subpath.
  pub fn close(&mut self) {
    self.out.push('Z');
    self.prev_was_number = false;
    self.last_command = None;
  }

  /// The path data written so far.
  pub fn into_string(self) -> String {
    self.out
  }
}

/// Quantization grid for path coordinates: two decimals. Path `d` data is the
/// bulk of the document and is sub-pixel-insensitive at the sizes takumi renders,
/// so it is quantized harder than the 3-decimal [`Num`] kept for transform
/// matrices (whose error multiplies into every coordinate).
const PATH_COORD_FACTOR: f32 = 100.0;
/// Tolerance (one grid step) for treating a curve's control point as the exact
/// reflection of the previous one, enabling the `s`/`t` shorthands.
const REFLECT_TOLERANCE: f32 = 1.0 / PATH_COORD_FACTOR;

/// Rounds a path coordinate to the grid path data keeps.
pub fn quantize_path(value: f32) -> f32 {
  (value * PATH_COORD_FACTOR).round() / PATH_COORD_FACTOR
}

/// Serializes takumi-core path commands ([`PathCommand`], the shared `Command`
/// type) to compact SVG path `d` data, applying `transform` to every point.
///
/// Coordinates are emitted relative to the previous point (the first move stays
/// absolute), axis-aligned lines collapse to `h`/`v`, and smooth cubics/quadratics
/// use the `s`/`t` shorthands. Each delta is quantized with its rounding error
/// folded into the next, so multi-contour fills stay closed — the core of SVGO's
/// `convertPathData`.
pub fn path_data(commands: &[PathCommand], transform: Affine) -> String {
  let [a, b, c, d, e, f] = transform.to_cols_array();
  let path =
    PathData::with_capacity(commands.len() * NUMBERS_PER_COMMAND * APPROX_CHARS_PER_NUMBER);
  let map = |p: Point<f32>| (a * p.x + c * p.y + e, b * p.x + d * p.y + f);
  let mut emit = RelEmit::new(path);
  for command in commands {
    match command {
      PathCommand::MoveTo(p) => {
        let (x, y) = map(*p);
        emit.move_to(x, y);
      }
      PathCommand::LineTo(p) => {
        let (x, y) = map(*p);
        emit.line_to(x, y);
      }
      PathCommand::QuadTo(c0, p) => {
        let (cx, cy) = map(*c0);
        let (x, y) = map(*p);
        emit.quad_to(cx, cy, x, y);
      }
      PathCommand::CubicTo(c0, c1, p) => {
        let (c1x, c1y) = map(*c0);
        let (c2x, c2y) = map(*c1);
        let (x, y) = map(*p);
        emit.cubic_to(c1x, c1y, c2x, c2y, x, y);
      }
      PathCommand::Close => emit.close(),
    }
  }
  emit.finish()
}

/// The control point of the previous curve (reconstructed in absolute space),
/// used to detect smooth joins for the `s`/`t` shorthands.
#[derive(Clone, Copy)]
enum PrevControl {
  None,
  Cubic(f32, f32),
  Quad(f32, f32),
}

/// Emits path segments as compact relative commands into a [`PathData`], tracking
/// the reconstructed (quantized) pen position so each relative delta is computed
/// against where a consumer actually lands, folding rounding error forward.
struct RelEmit {
  path: PathData,
  x: f32,
  y: f32,
  start_x: f32,
  start_y: f32,
  started: bool,
  prev: PrevControl,
}

impl RelEmit {
  fn new(path: PathData) -> Self {
    Self {
      path,
      x: 0.0,
      y: 0.0,
      start_x: 0.0,
      start_y: 0.0,
      started: false,
      prev: PrevControl::None,
    }
  }

  /// Advances the reconstructed pen by an already-quantized delta.
  fn advance(&mut self, dx: f32, dy: f32) {
    self.x = quantize_path(self.x + dx);
    self.y = quantize_path(self.y + dy);
  }

  fn move_to(&mut self, x: f32, y: f32) {
    if self.started {
      let (dx, dy) = (quantize_path(x - self.x), quantize_path(y - self.y));
      self.path.command(b'm');
      self.path.pair(dx, dy);
      self.advance(dx, dy);
    } else {
      let (qx, qy) = (quantize_path(x), quantize_path(y));
      self.path.command(b'M');
      self.path.pair(qx, qy);
      self.x = qx;
      self.y = qy;
      self.started = true;
    }
    self.start_x = self.x;
    self.start_y = self.y;
    self.prev = PrevControl::None;
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let (dx, dy) = (quantize_path(x - self.x), quantize_path(y - self.y));
    if dx == 0.0 && dy == 0.0 {
      return;
    }
    if dy == 0.0 {
      self.path.command(b'h');
      self.path.number(dx);
    } else if dx == 0.0 {
      self.path.command(b'v');
      self.path.number(dy);
    } else {
      self.path.command(b'l');
      self.path.pair(dx, dy);
    }
    self.advance(dx, dy);
    self.prev = PrevControl::None;
  }

  fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
    let (ox, oy) = (self.x, self.y);
    let (dx, dy) = (quantize_path(x - ox), quantize_path(y - oy));
    let control = match self.prev {
      PrevControl::Quad(px, py) if near(cx, 2.0 * ox - px) && near(cy, 2.0 * oy - py) => {
        self.path.command(b't');
        self.path.pair(dx, dy);
        (2.0 * ox - px, 2.0 * oy - py)
      }
      _ => {
        let (dcx, dcy) = (quantize_path(cx - ox), quantize_path(cy - oy));
        self.path.command(b'q');
        self.path.pair(dcx, dcy);
        self.path.pair(dx, dy);
        (ox + dcx, oy + dcy)
      }
    };
    self.advance(dx, dy);
    self.prev = PrevControl::Quad(control.0, control.1);
  }

  fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
    let (ox, oy) = (self.x, self.y);
    let (dx, dy) = (quantize_path(x - ox), quantize_path(y - oy));
    let (dc2x, dc2y) = (quantize_path(c2x - ox), quantize_path(c2y - oy));
    match self.prev {
      PrevControl::Cubic(px, py) if near(c1x, 2.0 * ox - px) && near(c1y, 2.0 * oy - py) => {
        self.path.command(b's');
      }
      _ => {
        self.path.command(b'c');
        self
          .path
          .pair(quantize_path(c1x - ox), quantize_path(c1y - oy));
      }
    }
    self.path.pair(dc2x, dc2y);
    self.path.pair(dx, dy);
    self.advance(dx, dy);
    self.prev = PrevControl::Cubic(ox + dc2x, oy + dc2y);
  }

  fn close(&mut self) {
    self.path.close();
    self.x = self.start_x;
    self.y = self.start_y;
    self.prev = PrevControl::None;
  }

  fn finish(self) -> String {
    self.path.into_string()
  }
}

fn near(a: f32, b: f32) -> bool {
  (a - b).abs() < REFLECT_TOLERANCE
}

/// Builds the `d` data for an axis-aligned rectangle from its edges.
pub fn edges_path_data(edges: Rect<f32>) -> String {
  let mut path = PathData::with_capacity(5 * APPROX_CHARS_PER_NUMBER);
  path.command(b'M');
  path.pair(edges.left, edges.top);
  path.command(b'H');
  path.number(edges.right);
  path.command(b'V');
  path.number(edges.bottom);
  path.command(b'H');
  path.number(edges.left);
  path.close();
  path.into_string()
}

/// Quantization grid for coordinates, dimensions, and opacities: three decimals.
/// SVG rendering is insensitive below this at the raster sizes takumi targets, so
/// dropping the float tail keeps documents compact without visible drift.
const COORD_FACTOR: f32 = 1000.0;

/// Rough characters one quantized number serializes to, used to presize buffers.
pub const APPROX_CHARS_PER_NUMBER: usize = 8;

/// Finite-guarded, quantized float formatter shared by every SVG numeric
/// emission site. Non-finite values serialize as `0`; finite values are rounded
/// to [`COORD_FACTOR`]'s grid and printed with the shortest representation, so
/// trailing zeros are dropped.
pub struct Num(pub f32);

/// Stack buffer for one formatted number. 48 bytes holds any `f32` Display.
struct NumBuf {
  bytes: [u8; 48],
  len: usize,
}

impl NumBuf {
  const fn new() -> Self {
    Self {
      bytes: [0; 48],
      len: 0,
    }
  }

  fn push(&mut self, byte: u8) {
    self.bytes[self.len] = byte;
    self.len += 1;
  }

  /// Writes `digits` decimal digits of `value`, zero-padded on the left.
  fn push_digits(&mut self, value: u32, digits: usize) {
    for place in (0..digits).rev() {
      self.push(b'0' + (value / 10u32.pow(place as u32) % 10) as u8);
    }
  }

  fn as_str(&self) -> &str {
    str::from_utf8(&self.bytes[..self.len]).unwrap_or("0")
  }
}

impl fmt::Write for NumBuf {
  fn write_str(&mut self, s: &str) -> fmt::Result {
    let end = self.len + s.len();
    if end > self.bytes.len() {
      return Err(fmt::Error);
    }
    self.bytes[self.len..end].copy_from_slice(s.as_bytes());
    self.len = end;
    Ok(())
  }
}

impl Num {
  /// Formats the number into `buf` and returns its text.
  fn format<'b>(&self, buf: &'b mut NumBuf) -> &'b str {
    if !self.0.is_finite() {
      return "0";
    }
    let units = (self.0 * COORD_FACTOR).round();
    if units == 0.0 {
      return "0";
    }
    // An `f32` round-trips every decimal of up to six significant digits, so the shortest
    // representation of `units / COORD_FACTOR` is that decimal as written.
    if units.abs() < 1_000_000.0 {
      let units = units as i32;
      let magnitude = units.unsigned_abs();
      let (whole, mut fraction) = (magnitude / 1000, magnitude % 1000);

      if units < 0 {
        buf.push(b'-');
      }
      if whole > 0 {
        buf.push_digits(whole, whole.ilog10() as usize + 1);
      }
      if fraction > 0 {
        let mut digits = 3;

        while fraction % 10 == 0 {
          fraction /= 10;
          digits -= 1;
        }
        buf.push(b'.');
        buf.push_digits(fraction, digits);
      }
      return buf.as_str();
    }
    let _ = write!(buf, "{}", units / COORD_FACTOR);
    buf.as_str()
  }
}

impl fmt::Display for Num {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.format(&mut NumBuf::new()))
  }
}

impl PathData {
  /// The points of a `points` attribute: pairs of numbers, up to the first thing that is not one.
  #[cfg(feature = "svg")]
  pub(crate) fn parse_points(text: &str) -> Vec<Point<f32>> {
    let mut scanner = Scanner::new(text);
    let mut points = Vec::new();

    while !scanner.at_end() {
      let Some((x, y)) = scanner.pair() else {
        break;
      };

      points.push(single((x, y)));
    }

    points
  }
}

/// Reads the numbers of an SVG attribute byte by byte, after svgtypes' `Stream`.
struct Scanner<'a> {
  bytes: &'a [u8],
  position: usize,
}

/// One path data segment as authored, before it is made absolute.
enum Segment {
  MoveTo(f64, f64),
  LineTo(f64, f64),
  Horizontal(f64),
  Vertical(f64),
  Cubic([f64; 6]),
  SmoothCubic([f64; 4]),
  Quadratic([f64; 4]),
  SmoothQuadratic(f64, f64),
  Arc(ArcSegment),
  Close,
}

struct ArcSegment {
  radii: (f64, f64),
  rotation: f64,
  large_arc: bool,
  sweep: bool,
  to: (f64, f64),
}

impl<'a> Scanner<'a> {
  fn new(text: &'a str) -> Self {
    Self {
      bytes: text.as_bytes(),
      position: 0,
    }
  }

  fn at_end(&self) -> bool {
    self.position >= self.bytes.len()
  }

  fn peek(&self) -> Option<u8> {
    self.bytes.get(self.position).copied()
  }

  fn skip_spaces(&mut self) {
    while self
      .peek()
      .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
    {
      self.position += 1;
    }
  }

  fn skip_digits(&mut self) {
    while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
      self.position += 1;
    }
  }

  /// A number, where `e` not followed by `m` or `x` starts an exponent.
  fn number(&mut self) -> Option<f64> {
    self.skip_spaces();

    let start = self.position;
    let mut byte = self.peek()?;

    if matches!(byte, b'+' | b'-') {
      self.position += 1;
      byte = self.peek()?;
    }

    match byte {
      b'0'..=b'9' => self.skip_digits(),
      b'.' => {}
      _ => return None,
    }

    if self.peek() == Some(b'.') {
      self.position += 1;
      self.skip_digits();
    }

    if matches!(self.peek(), Some(b'e' | b'E')) {
      let next = *self.bytes.get(self.position + 1)?;

      if next != b'm' && next != b'x' {
        self.position += 1;

        match self.peek()? {
          b'+' | b'-' => {
            self.position += 1;
            self.skip_digits();
          }
          b'0'..=b'9' => self.skip_digits(),
          _ => return None,
        }
      }
    }

    let number = str::from_utf8(self.bytes.get(start..self.position)?)
      .ok()?
      .parse::<f64>()
      .ok()?;

    (number as f32).is_finite().then_some(number)
  }

  /// A number followed by its optional comma separator.
  fn list_number(&mut self) -> Option<f64> {
    if self.at_end() {
      return None;
    }

    let number = self.number()?;

    self.skip_spaces();
    self.skip_comma();

    Some(number)
  }

  fn skip_comma(&mut self) {
    if self.peek() == Some(b',') {
      self.position += 1;
    }
  }

  /// An arc flag, which may sit directly against the next one without a separator.
  fn flag(&mut self) -> Option<bool> {
    self.skip_spaces();

    let flag = match self.peek()? {
      b'0' => false,
      b'1' => true,
      _ => return None,
    };

    self.position += 1;
    self.skip_spaces();
    self.skip_comma();
    self.skip_spaces();

    Some(flag)
  }

  fn pair(&mut self) -> Option<(f64, f64)> {
    Some((self.list_number()?, self.list_number()?))
  }

  /// The next segment and whether its coordinates are absolute. `previous_command` is the last
  /// command letter, which a number with no letter repeats (a moveto repeats as a lineto).
  fn segment(&mut self, previous_command: &mut Option<u8>) -> Option<(bool, Segment)> {
    let first = self.peek()?;

    if previous_command.is_none() && !matches!(first, b'M' | b'm') {
      return None;
    }

    let (command, implicit_moveto) = if is_command(first) {
      self.position += 1;

      (first, false)
    } else if matches!(first, b'0'..=b'9' | b'.' | b'-' | b'+') {
      match (*previous_command)? {
        b'Z' | b'z' => return None,
        b'M' => (b'L', true),
        b'm' => (b'l', true),
        repeated => (repeated, false),
      }
    } else {
      return None;
    };

    let absolute = command.is_ascii_uppercase();
    let segment = match command.to_ascii_lowercase() {
      b'm' => {
        let (x, y) = self.pair()?;

        Segment::MoveTo(x, y)
      }
      b'l' => {
        let (x, y) = self.pair()?;

        Segment::LineTo(x, y)
      }
      b'h' => Segment::Horizontal(self.list_number()?),
      b'v' => Segment::Vertical(self.list_number()?),
      b'c' => Segment::Cubic([
        self.list_number()?,
        self.list_number()?,
        self.list_number()?,
        self.list_number()?,
        self.list_number()?,
        self.list_number()?,
      ]),
      b's' => Segment::SmoothCubic([
        self.list_number()?,
        self.list_number()?,
        self.list_number()?,
        self.list_number()?,
      ]),
      b'q' => Segment::Quadratic([
        self.list_number()?,
        self.list_number()?,
        self.list_number()?,
        self.list_number()?,
      ]),
      b't' => {
        let (x, y) = self.pair()?;

        Segment::SmoothQuadratic(x, y)
      }
      b'a' => Segment::Arc(ArcSegment {
        radii: self.pair()?,
        rotation: self.list_number()?,
        large_arc: self.flag()?,
        sweep: self.flag()?,
        to: self.pair()?,
      }),
      _ => Segment::Close,
    };

    *previous_command = Some(match (implicit_moveto, absolute) {
      (true, true) => b'M',
      (true, false) => b'm',
      (false, _) => command,
    });

    Some((absolute, segment))
  }
}

fn is_command(byte: u8) -> bool {
  matches!(
    byte.to_ascii_lowercase(),
    b'm' | b'z' | b'l' | b'h' | b'v' | b'c' | b's' | b'q' | b't' | b'a'
  )
}

/// What the last segment leaves a smooth curve to reflect.
#[derive(Clone, Copy, Default)]
enum Reflection {
  #[default]
  None,
  Cubic {
    control: (f64, f64),
    end: (f64, f64),
  },
  Quadratic {
    control: (f64, f64),
    end: (f64, f64),
  },
  SmoothQuadratic {
    end: (f64, f64),
  },
}

/// Makes segments absolute and appends them as [`PathCommand`]s, after svgtypes'
/// `SimplifyingPathParser`.
#[derive(Default)]
struct Normalizer {
  commands: Vec<PathCommand>,
  current: (f64, f64),
  subpath_start: (f64, f64),
  reflection: Reflection,
  smooth_quadratic_control: (f64, f64),
  closed: bool,
}

impl Normalizer {
  /// Appends the segment, or drops it and returns false when a coordinate overflows `f32`.
  fn push(&mut self, absolute: bool, segment: Segment) -> bool {
    let appended_from = self.commands.len();
    let replaced = self.commands.last().copied();

    self.append(absolute, segment);

    let mut finite = true;

    // A moveto after a moveto replaces it in place, so the last old command is checked too.
    for command in &self.commands[appended_from.saturating_sub(1)..] {
      command.map_points(|point| {
        finite &= point.x.is_finite() && point.y.is_finite();
        point
      });
    }

    if !finite {
      self.commands.truncate(appended_from);

      if let (Some(replaced), Some(last)) = (replaced, appended_from.checked_sub(1)) {
        self.commands[last] = replaced;
      }
    }

    finite
  }

  fn append(&mut self, absolute: bool, segment: Segment) {
    if self.closed && !matches!(segment, Segment::MoveTo(..) | Segment::Close) {
      self.move_to(self.subpath_start);
    }

    let (origin_x, origin_y) = if absolute { (0.0, 0.0) } else { self.current };
    let (current_x, current_y) = self.current;

    match segment {
      Segment::MoveTo(x, y) => {
        let (origin_x, origin_y) = match (absolute, self.closed) {
          (true, _) => (0.0, 0.0),
          (false, true) => self.subpath_start,
          (false, false) => self.current,
        };

        self.move_to((x + origin_x, y + origin_y));
        self.reflection = Reflection::None;
      }
      Segment::LineTo(x, y) => {
        self.line_to((x + origin_x, y + origin_y));
        self.reflection = Reflection::None;
      }
      Segment::Horizontal(x) => {
        self.line_to((x + origin_x, current_y));
        self.reflection = Reflection::None;
      }
      Segment::Vertical(y) => {
        self.line_to((current_x, y + origin_y));
        self.reflection = Reflection::None;
      }
      Segment::Cubic([x1, y1, x2, y2, x, y]) => {
        let control = (x2 + origin_x, y2 + origin_y);
        let end = (x + origin_x, y + origin_y);

        self.cubic_to((x1 + origin_x, y1 + origin_y), control, end);
        self.reflection = Reflection::Cubic { control, end };
      }
      Segment::SmoothCubic([x2, y2, x, y]) => {
        let first = match self.reflection {
          Reflection::Cubic { control, end } => (end.0 * 2.0 - control.0, end.1 * 2.0 - control.1),
          _ => self.current,
        };
        let control = (x2 + origin_x, y2 + origin_y);
        let end = (x + origin_x, y + origin_y);

        self.cubic_to(first, control, end);
        self.reflection = Reflection::Cubic { control, end };
      }
      Segment::Quadratic([x1, y1, x, y]) => {
        let control = (x1 + origin_x, y1 + origin_y);
        let end = (x + origin_x, y + origin_y);

        self.quadratic_to(control, end);
        self.reflection = Reflection::Quadratic { control, end };
      }
      Segment::SmoothQuadratic(x, y) => {
        let control = match self.reflection {
          Reflection::Quadratic { control, end } => {
            (end.0 * 2.0 - control.0, end.1 * 2.0 - control.1)
          }
          Reflection::SmoothQuadratic { end } => (
            end.0 * 2.0 - self.smooth_quadratic_control.0,
            end.1 * 2.0 - self.smooth_quadratic_control.1,
          ),
          _ => self.current,
        };
        let end = (x + origin_x, y + origin_y);

        self.smooth_quadratic_control = control;
        self.quadratic_to(control, end);
        self.reflection = Reflection::SmoothQuadratic { end };
      }
      Segment::Arc(arc) => {
        let end = (arc.to.0 + origin_x, arc.to.1 + origin_y);

        self.arc_to(&arc, end);
        self.reflection = Reflection::None;
      }
      Segment::Close => {
        if !self.closed {
          self.commands.push(PathCommand::Close);
        }

        self.current = self.subpath_start;
        self.closed = true;
        self.reflection = Reflection::None;

        return;
      }
    }

    self.closed = false;
  }

  /// A moveto right after a moveto replaces it, as tiny-skia's `PathBuilder::move_to`.
  fn move_to(&mut self, point: (f64, f64)) {
    let command = PathCommand::MoveTo(single(point));

    if let Some(last @ PathCommand::MoveTo(_)) = self.commands.last_mut() {
      *last = command;
    } else {
      self.commands.push(command);
    }

    self.current = point;
    self.subpath_start = point;
  }

  fn line_to(&mut self, point: (f64, f64)) {
    self.commands.push(PathCommand::LineTo(single(point)));
    self.current = point;
  }

  fn quadratic_to(&mut self, control: (f64, f64), end: (f64, f64)) {
    self
      .commands
      .push(PathCommand::QuadTo(single(control), single(end)));
    self.current = end;
  }

  fn cubic_to(&mut self, first: (f64, f64), second: (f64, f64), end: (f64, f64)) {
    self.commands.push(PathCommand::CubicTo(
      single(first),
      single(second),
      single(end),
    ));
    self.current = end;
  }

  fn arc_to(&mut self, arc: &ArcSegment, end: (f64, f64)) {
    match arc.cubics(self.current, end) {
      Some(cubics) => {
        for [first, second, to] in cubics {
          self.cubic_to(first, second, to);
        }
      }
      None => self.line_to(end),
    }
  }
}

impl ArcSegment {
  /// The arc from `from` to `to` as at most four cubics, or none where it is a straight line.
  /// Blink's `SVGPathNormalizer::DecomposeArcToCubic` (core/svg/svg_path_parser.cc).
  fn cubics(&self, from: (f64, f64), to: (f64, f64)) -> Option<Vec<[(f64, f64); 3]>> {
    let angle = self.rotation.to_radians();
    let (mut rx, mut ry) = (self.radii.0.abs(), self.radii.1.abs());

    if rx == 0.0 || ry == 0.0 || from == to {
      return None;
    }

    let mid = ((from.0 - to.0) * 0.5, (from.1 - to.1) * 0.5);
    let mid = rotated(mid, -angle);
    let radii_scale = mid.0 * mid.0 / (rx * rx) + mid.1 * mid.1 / (ry * ry);

    if radii_scale > 1.0 {
      rx *= radii_scale.sqrt();
      ry *= radii_scale.sqrt();
    }

    let to_unit_circle = |point| {
      let (x, y) = rotated(point, -angle);

      (x / rx, y / ry)
    };
    let point1 = to_unit_circle(from);
    let point2 = to_unit_circle(to);
    let delta = (point2.0 - point1.0, point2.1 - point1.1);
    let mut scale_factor = (1.0 / (delta.0 * delta.0 + delta.1 * delta.1) - 0.25)
      .max(0.0)
      .sqrt()
      .min(f64::from(f32::MAX));

    if self.sweep == self.large_arc {
      scale_factor = -scale_factor;
    }

    let center = (
      (point1.0 + point2.0) * 0.5 - delta.1 * scale_factor,
      (point1.1 + point2.1) * 0.5 + delta.0 * scale_factor,
    );
    let theta1 = (point1.1 - center.1).atan2(point1.0 - center.0);
    let theta2 = (point2.1 - center.1).atan2(point2.0 - center.0);
    let mut sweep = theta2 - theta1;

    if sweep < 0.0 && self.sweep {
      sweep += TAU;
    } else if sweep > 0.0 && !self.sweep {
      sweep -= TAU;
    }

    if !sweep.is_finite() {
      return None;
    }

    // The extra 0.001 keeps a quarter turn that atan2 rounds a little over from becoming two cubics.
    let segments = (sweep / (FRAC_PI_2 + 0.001)).abs().ceil() as usize;
    let to_user_space = |(x, y): (f64, f64)| rotated((x * rx, y * ry), angle);
    let mut cubics = Vec::with_capacity(segments);

    for index in 0..segments {
      let start = theta1 + index as f64 * sweep / segments as f64;
      let end = theta1 + (index + 1) as f64 * sweep / segments as f64;
      let tangent = (8.0 / 6.0) * (0.25 * (end - start)).tan();

      if !tangent.is_finite() {
        return None;
      }

      let (sin_start, cos_start) = start.sin_cos();
      let (sin_end, cos_end) = end.sin_cos();
      let first = (
        cos_start - tangent * sin_start + center.0,
        sin_start + tangent * cos_start + center.1,
      );
      let end_point = (cos_end + center.0, sin_end + center.1);
      let second = (
        end_point.0 + tangent * sin_end,
        end_point.1 - tangent * cos_end,
      );

      cubics.push([
        to_user_space(first),
        to_user_space(second),
        to_user_space(end_point),
      ]);
    }

    Some(cubics)
  }
}

impl PathData {
  /// The commands of the clockwise arc of less than half a turn from `from` to `to` on an ellipse
  /// with radii `rx` and `ry`, as the corners of a `rect` and the sides of an `ellipse` take.
  #[cfg(feature = "svg")]
  pub(crate) fn corner_arc(from: (f32, f32), to: (f32, f32), rx: f32, ry: f32) -> Vec<PathCommand> {
    let point = |(x, y): (f32, f32)| (f64::from(x), f64::from(y));
    let arc = ArcSegment {
      radii: (f64::from(rx), f64::from(ry)),
      rotation: 0.0,
      large_arc: false,
      sweep: true,
      to: point(to),
    };

    match arc.cubics(point(from), point(to)) {
      Some(cubics) => cubics
        .into_iter()
        .map(|[first, second, to]| PathCommand::CubicTo(single(first), single(second), single(to)))
        .collect(),
      None => vec![PathCommand::LineTo(Point::new(to.0, to.1))],
    }
  }
}

/// `point` turned by `angle` radians.
fn rotated((x, y): (f64, f64), angle: f64) -> (f64, f64) {
  let (sin, cos) = angle.sin_cos();

  (x * cos - y * sin, x * sin + y * cos)
}

fn single((x, y): (f64, f64)) -> Point<f32> {
  Point::new(x as f32, y as f32)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// `Num` as float `Display` prints it, the integer-part zero dropped.
  fn displayed(value: f32) -> String {
    if !value.is_finite() {
      return "0".into();
    }
    let value = (value * COORD_FACTOR).round() / COORD_FACTOR;
    if value == 0.0 {
      return "0".into();
    }
    let text = value.to_string();
    if let Some(rest) = text.strip_prefix("0.") {
      format!(".{rest}")
    } else if let Some(rest) = text.strip_prefix("-0.") {
      format!("-.{rest}")
    } else {
      text
    }
  }

  #[test]
  fn num_matches_float_display() {
    let mut state = 0x2545_F491_4F6C_DD1D_u64;

    for index in 0..200_000_u64 {
      state ^= state << 13;
      state ^= state >> 7;
      state ^= state << 17;

      let value = match index % 4 {
        0 => f32::from_bits(state as u32),
        1 => ((state % 2_000_000) as f32 - 1_000_000.0) / 1000.0,
        2 => ((state % 200_000_000) as f32 - 100_000_000.0) / 7919.0,
        _ => (state as u32 as f32 / u32::MAX as f32 - 0.5) * 4.0,
      };

      assert_eq!(Num(value).to_string(), displayed(value), "{value:?}");
    }
    for value in [
      f32::MAX,
      f32::MIN,
      999.9995,
      -999.9995,
      1000.0,
      0.0005,
      -0.0004,
    ] {
      assert_eq!(Num(value).to_string(), displayed(value), "{value:?}");
    }
  }

  fn build(f: impl FnOnce(&mut PathData)) -> String {
    let mut path = PathData::with_capacity(0);
    f(&mut path);
    path.into_string()
  }

  #[test]
  fn drops_separator_only_when_grammar_allows() {
    // digit-led number needs a separator; minus and `.`-after-`.` do not.
    assert_eq!(
      build(|p| {
        p.command(b'L');
        p.pair(10.0, 10.0);
      }),
      "L10 10"
    );
    assert_eq!(
      build(|p| {
        p.command(b'L');
        p.pair(10.0, -5.0);
      }),
      "L10-5"
    );
    assert_eq!(
      build(|p| {
        p.command(b'L');
        p.pair(0.5, 0.5);
      }),
      "L.5.5"
    );
    // `.`-led after an integer must keep the separator or it would merge.
    assert_eq!(
      build(|p| {
        p.command(b'L');
        p.pair(1.0, 0.5);
      }),
      "L1 .5"
    );
    assert_eq!(
      build(|p| {
        p.command(b'L');
        p.pair(1.5, 0.5);
      }),
      "L1.5.5"
    );
  }

  #[test]
  fn elides_repeated_and_moveto_implicit_line() {
    let path = build(|p| {
      p.command(b'M');
      p.pair(0.0, 0.0);
      p.command(b'L');
      p.pair(1.0, 1.0);
      p.command(b'L');
      p.pair(2.0, 2.0);
    });
    assert_eq!(path, "M0 0 1 1 2 2");
  }

  fn pt(x: f32, y: f32) -> Point<f32> {
    Point::new(x, y)
  }

  #[test]
  fn path_data_is_relative_with_hv_shorthands() {
    let commands = [
      PathCommand::MoveTo(pt(10.0, 10.0)),
      PathCommand::LineTo(pt(20.0, 10.0)),
      PathCommand::LineTo(pt(20.0, 20.0)),
      PathCommand::Close,
    ];
    assert_eq!(path_data(&commands, Affine::IDENTITY), "M10 10h10v10Z");
  }

  #[test]
  fn path_data_uses_smooth_cubic_shorthand() {
    // The second cubic's first control is the reflection of the first cubic's
    // second control about the join, so it collapses to `s`.
    let commands = [
      PathCommand::MoveTo(pt(0.0, 0.0)),
      PathCommand::CubicTo(pt(0.0, 5.0), pt(5.0, 5.0), pt(5.0, 0.0)),
      PathCommand::CubicTo(pt(5.0, -5.0), pt(10.0, -5.0), pt(10.0, 0.0)),
    ];
    assert_eq!(
      path_data(&commands, Affine::IDENTITY),
      "M0 0c0 5 5 5 5 0s5-5 5 0"
    );
  }

  #[test]
  fn path_data_quantizes_to_two_decimals() {
    let commands = [
      PathCommand::MoveTo(pt(0.0, 0.0)),
      PathCommand::LineTo(pt(1.2345, 6.789)),
    ];
    assert_eq!(path_data(&commands, Affine::IDENTITY), "M0 0l1.23 6.79");
  }

  #[test]
  fn consecutive_moves_keep_their_letter() {
    let commands = [
      PathCommand::MoveTo(pt(0.0, 0.0)),
      PathCommand::LineTo(pt(10.0, 0.0)),
      PathCommand::MoveTo(pt(20.0, 0.0)),
      PathCommand::MoveTo(pt(30.0, 5.0)),
      PathCommand::LineTo(pt(40.0, 5.0)),
    ];
    assert_eq!(
      path_data(&commands, Affine::IDENTITY),
      "M0 0h10m10 0m10 5h10"
    );
  }

  #[test]
  fn close_forces_next_command_letter() {
    let path = build(|p| {
      p.command(b'M');
      p.pair(0.0, 0.0);
      p.close();
      p.command(b'M');
      p.pair(1.0, 1.0);
    });
    assert_eq!(path, "M0 0ZM1 1");
  }

  fn p(x: f32, y: f32) -> Point<f32> {
    Point::new(x, y)
  }

  fn parsed(text: &str) -> Vec<PathCommand> {
    PathData::parse(text).0
  }

  #[test]
  fn a_moveto_followed_by_pairs_continues_as_linetos() {
    assert_eq!(
      parsed("m 30 40 110 120 -20 -130"),
      [
        PathCommand::MoveTo(p(30.0, 40.0)),
        PathCommand::LineTo(p(140.0, 160.0)),
        PathCommand::LineTo(p(120.0, 30.0)),
      ]
    );
  }

  #[test]
  fn a_command_after_close_starts_at_the_subpath_start() {
    assert_eq!(
      parsed("M 10 20 L 30 40 Z L 50 60"),
      [
        PathCommand::MoveTo(p(10.0, 20.0)),
        PathCommand::LineTo(p(30.0, 40.0)),
        PathCommand::Close,
        PathCommand::MoveTo(p(10.0, 20.0)),
        PathCommand::LineTo(p(50.0, 60.0)),
      ]
    );
  }

  #[test]
  fn a_relative_moveto_after_close_is_relative_to_the_subpath_start() {
    assert_eq!(
      parsed("M 10 10 L 20 20 z m 5 5"),
      [
        PathCommand::MoveTo(p(10.0, 10.0)),
        PathCommand::LineTo(p(20.0, 20.0)),
        PathCommand::Close,
        PathCommand::MoveTo(p(15.0, 15.0)),
      ]
    );
  }

  #[test]
  fn repeated_closes_collapse() {
    assert_eq!(
      parsed("M 10 20 L 30 40 Z Z Z"),
      [
        PathCommand::MoveTo(p(10.0, 20.0)),
        PathCommand::LineTo(p(30.0, 40.0)),
        PathCommand::Close,
      ]
    );
  }

  #[test]
  fn consecutive_movetos_collapse() {
    assert_eq!(
      parsed("M 10 10 M 20 20 L 30 30"),
      [
        PathCommand::MoveTo(p(20.0, 20.0)),
        PathCommand::LineTo(p(30.0, 30.0)),
      ]
    );
  }

  #[test]
  fn smooth_curves_reflect_the_previous_control_point() {
    assert_eq!(
      parsed("M 30 40 C 16 137 171 45 100 90 S 171 45 180 155"),
      [
        PathCommand::MoveTo(p(30.0, 40.0)),
        PathCommand::CubicTo(p(16.0, 137.0), p(171.0, 45.0), p(100.0, 90.0)),
        PathCommand::CubicTo(p(29.0, 135.0), p(171.0, 45.0), p(180.0, 155.0)),
      ]
    );
    assert_eq!(
      parsed("M 30 40 q 171 45 50 40 t 60 80"),
      [
        PathCommand::MoveTo(p(30.0, 40.0)),
        PathCommand::QuadTo(p(201.0, 85.0), p(80.0, 80.0)),
        PathCommand::QuadTo(p(-41.0, 75.0), p(140.0, 160.0)),
      ]
    );
    assert_eq!(
      parsed("M 30 40 S 171 45 180 155"),
      [
        PathCommand::MoveTo(p(30.0, 40.0)),
        PathCommand::CubicTo(p(30.0, 40.0), p(171.0, 45.0), p(180.0, 155.0)),
      ]
    );
  }

  #[test]
  fn arc_flags_need_no_separators() {
    let (commands, complete) = PathData::parse("M10-20A5.5.3-4 010-.1");

    assert!(complete);
    assert!(matches!(commands.first(), Some(PathCommand::MoveTo(_))));
    assert!(matches!(commands.last(), Some(PathCommand::CubicTo(..))));
    assert_eq!(parsed("M0 0 a1 1 0 00 10 10").len(), 3);
  }

  #[test]
  fn an_arc_with_zero_radii_is_a_line() {
    assert_eq!(
      parsed("M 0 0 A 0 0 0 0 0 10 10"),
      [
        PathCommand::MoveTo(p(0.0, 0.0)),
        PathCommand::LineTo(p(10.0, 10.0)),
      ]
    );
  }

  #[test]
  fn numbers_follow_the_svg_grammar() {
    assert_eq!(
      parsed("M 1e2 .5.5-.5 +1.5e-1 2"),
      [
        PathCommand::MoveTo(p(100.0, 0.5)),
        PathCommand::LineTo(p(0.5, -0.5)),
        PathCommand::LineTo(p(0.15, 2.0)),
      ]
    );
  }

  #[test]
  fn the_commands_before_an_error_are_kept() {
    let (commands, complete) = PathData::parse("M 10 10 L 20 20 X 30 30");

    assert!(!complete);
    assert_eq!(
      commands,
      [
        PathCommand::MoveTo(p(10.0, 10.0)),
        PathCommand::LineTo(p(20.0, 20.0)),
      ]
    );
  }

  #[test]
  fn path_data_must_start_with_a_moveto() {
    assert_eq!(PathData::parse("L 10 10"), (Vec::new(), false));
    assert_eq!(
      PathData::parse("M 0 0 Z 2"),
      (
        vec![PathCommand::MoveTo(p(0.0, 0.0)), PathCommand::Close],
        false
      )
    );
    assert_eq!(PathData::parse(""), (Vec::new(), true));
  }

  #[test]
  fn an_incomplete_number_stops_parsing() {
    assert!(!PathData::parse("M 0 0 L 10").1);
    assert!(!PathData::parse("M 0 0 L 10 1e").1);
  }

  #[test]
  fn an_arc_is_at_most_four_cubics_whatever_its_radius() {
    for radius in ["1", "1e30", "3.4e38"] {
      let (commands, complete) = PathData::parse(&format!("M 0 0 A {radius} {radius} 0 1 1 10 10"));

      assert!(complete, "{radius}");
      assert!(commands.len() <= 5, "{radius}: {}", commands.len());
    }

    assert_eq!(parsed("M 0 0 A 5 5 0 1 1 0 0.0001").len(), 5);
  }

  #[test]
  fn a_value_that_overflows_f32_is_invalid() {
    assert_eq!(
      PathData::parse("M 0 0 A 1e39 1 0 0 0 10 10"),
      (parsed("M 0 0"), false)
    );
    assert!(!PathData::parse("M 0 0 L 1e39 0").1);
    assert!(!PathData::parse("M 3e38 0 l 3e38 0").1);
    assert!(!PathData::parse("M 3e38 0 m 3e38 0").1);
  }

  #[test]
  fn an_arc_flag_takes_spaces_before_its_comma() {
    assert!(PathData::parse("M 0 0 A 5 5 0 1 ,0 10 10").1);
    assert!(PathData::parse("M 0 0 A 5 5 0 1 , 0 10 10").1);
  }
}
