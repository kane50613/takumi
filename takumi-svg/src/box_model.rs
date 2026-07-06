//! Box-model geometry helpers for the node walk: the element's affine transform,
//! overflow clipping predicate, and serialization of takumi-core path commands to
//! SVG path `d` data. The rounded-rect / border-radius geometry itself is reused
//! from takumi-core's [`BorderProperties`] (the same geometry the raster backend
//! rasterizes), not reimplemented here.

use std::fmt::Write as _;

use takumi_core::{
  context::RenderContext,
  geometry::{PathCommand, Point, Size},
  style::Affine,
};

use crate::{APPROX_CHARS_PER_NUMBER, Num};

/// Numbers a single path command serializes (a cubic carries three coordinate
/// pairs), used with [`APPROX_CHARS_PER_NUMBER`] to presize the path buffer.
const NUMBERS_PER_COMMAND: usize = 6;

/// Builds compact SVG path `d` data: numbers are quantized via [`Num`], the
/// command letter is elided when it repeats (and a moveto's trailing pairs fold
/// into implicit linetos), and the separator between two numbers is dropped
/// whenever the grammar still lexes them apart (the next number starts with `-`,
/// or with `.` when the previous number already carried a `.`).
pub(crate) struct PathData {
  out: String,
  scratch: String,
  prev_was_number: bool,
  prev_had_dot: bool,
  last_command: Option<u8>,
}

impl PathData {
  pub(crate) fn with_capacity(capacity: usize) -> Self {
    Self {
      out: String::with_capacity(capacity),
      scratch: String::new(),
      prev_was_number: false,
      prev_had_dot: false,
      last_command: None,
    }
  }

  pub(crate) fn command(&mut self, letter: u8) {
    let elide = matches!(self.last_command, Some(prev) if prev == letter
      || (prev == b'M' && letter == b'L')
      || (prev == b'm' && letter == b'l'));
    if !elide {
      self.out.push(letter as char);
      self.prev_was_number = false;
    }
    self.last_command = Some(letter);
  }

  pub(crate) fn number(&mut self, value: f32) {
    self.scratch.clear();
    let _ = write!(self.scratch, "{}", Num(value));
    let first = self.scratch.as_bytes()[0];
    if self.prev_was_number && first != b'-' && !(first == b'.' && self.prev_had_dot) {
      self.out.push(' ');
    }
    self.out.push_str(&self.scratch);
    self.prev_was_number = true;
    self.prev_had_dot = self.scratch.contains('.');
  }

  pub(crate) fn pair(&mut self, x: f32, y: f32) {
    self.number(x);
    self.number(y);
  }

  pub(crate) fn close(&mut self) {
    self.out.push('Z');
    self.prev_was_number = false;
    self.last_command = None;
  }

  pub(crate) fn into_string(self) -> String {
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

fn quantize_path(value: f32) -> f32 {
  (value * PATH_COORD_FACTOR).round() / PATH_COORD_FACTOR
}

/// Serializes takumi-core path commands ([`PathCommand`], the shared `Command`
/// type) to compact SVG path `d` data, applying `transform` (`[a, b, c, d, e,
/// f]`, SVG `matrix` order) to every point. Shared by glyph, border, background,
/// and clip emission.
///
/// Coordinates are emitted relative to the previous point (the first move stays
/// absolute), axis-aligned lines collapse to `h`/`v`, and smooth cubics/quadratics
/// use the `s`/`t` shorthands. Each delta is quantized with its rounding error
/// folded into the next, so multi-contour fills stay closed — the core of SVGO's
/// `convertPathData`.
pub(crate) fn path_data(commands: &[PathCommand], [a, b, c, d, e, f]: [f32; 6]) -> String {
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

/// Builds the `d` data for an axis-aligned rectangle (`M x y H V H Z`), shared by
/// the clip-rect and conic-tile emitters.
pub(crate) fn rect_path_data(x: f32, y: f32, width: f32, height: f32) -> String {
  let mut path = PathData::with_capacity(5 * APPROX_CHARS_PER_NUMBER);
  path.command(b'M');
  path.pair(x, y);
  path.command(b'H');
  path.number(x + width);
  path.command(b'V');
  path.number(y + height);
  path.command(b'H');
  path.number(x);
  path.close();
  path.into_string()
}

/// Computes the element's paint transform as an absolute-space matrix (the walk
/// emits children in absolute coordinates, so the transform is conjugated by the
/// box origin). Returns `None` when the element has no transform.
pub(crate) fn element_transform(
  context: &RenderContext,
  border_box: Size<f32>,
  x: f32,
  y: f32,
) -> Option<Affine> {
  let local = context
    .style
    .local_transform(border_box.width, border_box.height, &context.sizing);
  if local.is_identity() {
    return None;
  }
  // Children are emitted in absolute coordinates; move the local transform into
  // that space: M_abs = T(x, y) * local * T(-x, -y).
  Some(Affine::translation(x, y) * local * Affine::translation(-x, -y))
}

#[cfg(test)]
mod tests {
  use super::*;

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

  const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

  #[test]
  fn path_data_is_relative_with_hv_shorthands() {
    let commands = [
      PathCommand::MoveTo(pt(10.0, 10.0)),
      PathCommand::LineTo(pt(20.0, 10.0)),
      PathCommand::LineTo(pt(20.0, 20.0)),
      PathCommand::Close,
    ];
    assert_eq!(path_data(&commands, IDENTITY), "M10 10h10v10Z");
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
    assert_eq!(path_data(&commands, IDENTITY), "M0 0c0 5 5 5 5 0s5-5 5 0");
  }

  #[test]
  fn path_data_quantizes_to_two_decimals() {
    let commands = [
      PathCommand::MoveTo(pt(0.0, 0.0)),
      PathCommand::LineTo(pt(1.2345, 6.789)),
    ];
    assert_eq!(path_data(&commands, IDENTITY), "M0 0l1.23 6.79");
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
}
