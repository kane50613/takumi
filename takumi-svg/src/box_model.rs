//! Box-model geometry helpers for the node walk: the element's affine transform,
//! overflow clipping predicate, and serialization of takumi-core path commands to
//! SVG path `d` data. The rounded-rect / border-radius geometry itself is reused
//! from takumi-core's [`BorderProperties`] (the same geometry the raster backend
//! rasterizes), not reimplemented here.

use std::fmt::Write as _;

use taffy::Size;
use takumi_core::context::RenderContext;
use takumi_core::layout::style::Affine;
use tiny_skia::{PathSegment, Point};

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
    let elide =
      matches!(self.last_command, Some(prev) if prev == letter || (prev == b'M' && letter == b'L'));
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

/// Serializes takumi-core path commands ([`tiny_skia::PathSegment`], the shared
/// `Command` type) to SVG path `d` data, applying `transform` (`[a, b, c, d, e, f]`,
/// SVG `matrix` order) to every point. Shared by glyph, border, background, and
/// clip emission.
pub(crate) fn path_data(commands: &[PathSegment], [a, b, c, d, e, f]: [f32; 6]) -> String {
  let mut path =
    PathData::with_capacity(commands.len() * NUMBERS_PER_COMMAND * APPROX_CHARS_PER_NUMBER);
  let map = |p: Point| (a * p.x + c * p.y + e, b * p.x + d * p.y + f);
  for command in commands {
    match command {
      PathSegment::MoveTo(p) => {
        let (x, y) = map(*p);
        path.command(b'M');
        path.pair(x, y);
      }
      PathSegment::LineTo(p) => {
        let (x, y) = map(*p);
        path.command(b'L');
        path.pair(x, y);
      }
      PathSegment::QuadTo(c0, p) => {
        let (x0, y0) = map(*c0);
        let (x, y) = map(*p);
        path.command(b'Q');
        path.pair(x0, y0);
        path.pair(x, y);
      }
      PathSegment::CubicTo(c0, c1, p) => {
        let (x0, y0) = map(*c0);
        let (x1, y1) = map(*c1);
        let (x, y) = map(*p);
        path.command(b'C');
        path.pair(x0, y0);
        path.pair(x1, y1);
        path.pair(x, y);
      }
      PathSegment::Close => path.close(),
    }
  }
  path.into_string()
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
  let local = context.style.local_transform(border_box, &context.sizing);
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
