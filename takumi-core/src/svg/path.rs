//! Path data, after Blink's `SVGPathStringSource`, `SVGPathParser` and `SVGPathNormalizer`.
//!
//! The parser stops at the first error and keeps the segments before it, as SVG requires.

use kurbo::{Arc, SvgArc};

use super::scan::Scanner;

/// An absolute path segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
  /// Starts a subpath.
  MoveTo {
    /// The x coordinate.
    x: f64,
    /// The y coordinate.
    y: f64,
  },
  /// A straight line.
  LineTo {
    /// The x coordinate.
    x: f64,
    /// The y coordinate.
    y: f64,
  },
  /// A quadratic Bezier curve.
  QuadTo {
    /// The control point x coordinate.
    x1: f64,
    /// The control point y coordinate.
    y1: f64,
    /// The end x coordinate.
    x: f64,
    /// The end y coordinate.
    y: f64,
  },
  /// A cubic Bezier curve.
  CubicTo {
    /// The first control point x coordinate.
    x1: f64,
    /// The first control point y coordinate.
    y1: f64,
    /// The second control point x coordinate.
    x2: f64,
    /// The second control point y coordinate.
    y2: f64,
    /// The end x coordinate.
    x: f64,
    /// The end y coordinate.
    y: f64,
  },
  /// Closes the subpath.
  Close,
}

/// The control point the next smooth command reflects.
#[derive(Clone, Copy)]
enum Previous {
  Other,
  Cubic { x2: f64, y2: f64 },
  Quad { x1: f64, y1: f64 },
}

struct Normalizer {
  segments: Vec<PathSegment>,
  x: f64,
  y: f64,
  start_x: f64,
  start_y: f64,
  previous: Previous,
}

impl Normalizer {
  fn push(&mut self, segment: PathSegment) {
    match segment {
      PathSegment::MoveTo { x, y } => {
        self.x = x;
        self.y = y;
        self.start_x = x;
        self.start_y = y;
      }
      PathSegment::LineTo { x, y }
      | PathSegment::QuadTo { x, y, .. }
      | PathSegment::CubicTo { x, y, .. } => {
        self.x = x;
        self.y = y;
      }
      PathSegment::Close => {
        self.x = self.start_x;
        self.y = self.start_y;
      }
    }
    self.segments.push(segment);
  }

  fn ends_closed(&self) -> bool {
    matches!(self.segments.last(), Some(PathSegment::Close))
  }
}

/// Parses path data into absolute move, line, curve, and close segments. Arcs become cubic
/// curves, `H` and `V` become lines, and a command after a close starts a new subpath at the
/// start of the closed one.
pub fn parse_path_data(text: &str) -> Vec<PathSegment> {
  parse(text).0
}

/// Like [`parse_path_data`], but `None` unless the whole text is valid path data.
pub fn parse_path_data_strict(text: &str) -> Option<Vec<PathSegment>> {
  let (segments, complete) = parse(text);

  complete.then_some(segments)
}

/// The segments before the first error, and whether the text ended without one.
fn parse(text: &str) -> (Vec<PathSegment>, bool) {
  let mut scanner = Scanner::new(text);
  let mut state = Normalizer {
    segments: Vec::new(),
    x: 0.0,
    y: 0.0,
    start_x: 0.0,
    start_y: 0.0,
    previous: Previous::Other,
  };
  let mut command: Option<u8> = None;

  loop {
    scanner.skip_spaces();

    let Some(next) = scanner.peek() else {
      return (state.segments, true);
    };

    let (letter, implicit_line) = if is_command(next) {
      scanner.advance(1);
      (next, false)
    } else if is_number_start(next) {
      match command {
        Some(b'Z' | b'z') | None => return (state.segments, false),
        Some(b'M') => (b'L', true),
        Some(b'm') => (b'l', true),
        Some(previous) => (previous, false),
      }
    } else {
      return (state.segments, false);
    };

    if command.is_none() && !matches!(letter, b'M' | b'm') {
      return (state.segments, false);
    }
    if read_segment(&mut scanner, &mut state, letter).is_none() {
      return (state.segments, false);
    }
    command = Some(if implicit_line {
      if letter == b'L' { b'M' } else { b'm' }
    } else {
      letter
    });
  }
}

fn read_segment(scanner: &mut Scanner, state: &mut Normalizer, letter: u8) -> Option<()> {
  let relative = letter.is_ascii_lowercase();
  let (base_x, base_y) = (state.x, state.y);
  let pair = |scanner: &mut Scanner| -> Option<(f64, f64)> {
    let x = scanner.number_f64()?;
    let y = scanner.number_f64()?;

    Some(if relative {
      (x + base_x, y + base_y)
    } else {
      (x, y)
    })
  };
  let mut segments = Vec::new();
  let mut previous = Previous::Other;

  match letter.to_ascii_lowercase() {
    b'm' => {
      let (x, y) = pair(scanner)?;

      segments.push(PathSegment::MoveTo { x, y });
    }
    b'l' => {
      let (x, y) = pair(scanner)?;

      segments.push(PathSegment::LineTo { x, y });
    }
    b'h' => {
      let mut x = scanner.number_f64()?;

      if relative {
        x += base_x;
      }
      segments.push(PathSegment::LineTo { x, y: base_y });
    }
    b'v' => {
      let mut y = scanner.number_f64()?;

      if relative {
        y += base_y;
      }
      segments.push(PathSegment::LineTo { x: base_x, y });
    }
    b'c' => {
      let (x1, y1) = pair(scanner)?;
      let (x2, y2) = pair(scanner)?;
      let (x, y) = pair(scanner)?;

      segments.push(PathSegment::CubicTo {
        x1,
        y1,
        x2,
        y2,
        x,
        y,
      });
      previous = Previous::Cubic { x2, y2 };
    }
    b's' => {
      let (x1, y1) = match state.previous {
        Previous::Cubic { x2, y2 } => (base_x * 2.0 - x2, base_y * 2.0 - y2),
        _ => (base_x, base_y),
      };
      let (x2, y2) = pair(scanner)?;
      let (x, y) = pair(scanner)?;

      segments.push(PathSegment::CubicTo {
        x1,
        y1,
        x2,
        y2,
        x,
        y,
      });
      previous = Previous::Cubic { x2, y2 };
    }
    b'q' => {
      let (x1, y1) = pair(scanner)?;
      let (x, y) = pair(scanner)?;

      segments.push(PathSegment::QuadTo { x1, y1, x, y });
      previous = Previous::Quad { x1, y1 };
    }
    b't' => {
      let (x1, y1) = match state.previous {
        Previous::Quad { x1, y1 } => (base_x * 2.0 - x1, base_y * 2.0 - y1),
        _ => (base_x, base_y),
      };
      let (x, y) = pair(scanner)?;

      segments.push(PathSegment::QuadTo { x1, y1, x, y });
      previous = Previous::Quad { x1, y1 };
    }
    b'a' => {
      let rx = scanner.number_f64()?;
      let ry = scanner.number_f64()?;
      let rotation = scanner.number_f64()?;
      let large_arc = flag(scanner)?;
      let sweep = flag(scanner)?;
      let (x, y) = pair(scanner)?;
      let arc = SvgArc {
        from: kurbo::Point::new(base_x, base_y),
        to: kurbo::Point::new(x, y),
        radii: kurbo::Vec2::new(rx, ry),
        x_rotation: rotation.to_radians(),
        large_arc,
        sweep,
      };

      match Arc::from_svg_arc(&arc) {
        Some(arc) => arc.to_cubic_beziers(0.1, |p1, p2, p| {
          segments.push(PathSegment::CubicTo {
            x1: p1.x,
            y1: p1.y,
            x2: p2.x,
            y2: p2.y,
            x: p.x,
            y: p.y,
          });
        }),
        None => segments.push(PathSegment::LineTo { x, y }),
      }
    }
    _ => {
      if !state.ends_closed() {
        segments.push(PathSegment::Close);
      }
    }
  }

  if state.ends_closed()
    && !matches!(
      segments.first(),
      Some(PathSegment::MoveTo { .. } | PathSegment::Close) | None
    )
  {
    let (x, y) = (state.start_x, state.start_y);

    state.push(PathSegment::MoveTo { x, y });
  }
  for segment in segments {
    state.push(segment);
  }
  state.previous = previous;
  Some(())
}

/// A flag is one character, and needs no separator after it.
fn flag(scanner: &mut Scanner) -> Option<bool> {
  scanner.skip_spaces();

  let value = match scanner.peek()? {
    b'0' => false,
    b'1' => true,
    _ => return None,
  };

  scanner.advance(1);
  scanner.skip_comma_spaces();
  Some(value)
}

fn is_command(byte: u8) -> bool {
  matches!(
    byte,
    b'M'
      | b'm'
      | b'Z'
      | b'z'
      | b'L'
      | b'l'
      | b'H'
      | b'h'
      | b'V'
      | b'v'
      | b'C'
      | b'c'
      | b'S'
      | b's'
      | b'Q'
      | b'q'
      | b'T'
      | b't'
      | b'A'
      | b'a'
  )
}

fn is_number_start(byte: u8) -> bool {
  matches!(byte, b'0'..=b'9' | b'.' | b'-' | b'+')
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn relative_commands_become_absolute() {
    assert_eq!(
      parse_path_data("m10 10 l5 0 h5 v5 z"),
      vec![
        PathSegment::MoveTo { x: 10.0, y: 10.0 },
        PathSegment::LineTo { x: 15.0, y: 10.0 },
        PathSegment::LineTo { x: 20.0, y: 10.0 },
        PathSegment::LineTo { x: 20.0, y: 15.0 },
        PathSegment::Close,
      ]
    );
  }

  #[test]
  fn extra_pairs_after_a_move_are_lines() {
    assert_eq!(
      parse_path_data("M0 0 1 1"),
      vec![
        PathSegment::MoveTo { x: 0.0, y: 0.0 },
        PathSegment::LineTo { x: 1.0, y: 1.0 },
      ]
    );
  }

  #[test]
  fn a_command_after_close_restarts_at_the_subpath_start() {
    assert_eq!(
      parse_path_data("M1 2 L5 5 Z L9 9"),
      vec![
        PathSegment::MoveTo { x: 1.0, y: 2.0 },
        PathSegment::LineTo { x: 5.0, y: 5.0 },
        PathSegment::Close,
        PathSegment::MoveTo { x: 1.0, y: 2.0 },
        PathSegment::LineTo { x: 9.0, y: 9.0 },
      ]
    );
  }

  #[test]
  fn parsing_keeps_the_segments_before_an_error() {
    assert_eq!(
      parse_path_data("M0 0 L1 1 L2"),
      vec![
        PathSegment::MoveTo { x: 0.0, y: 0.0 },
        PathSegment::LineTo { x: 1.0, y: 1.0 },
      ]
    );
    assert!(parse_path_data("L1 1").is_empty());
  }

  #[test]
  fn smooth_curves_reflect_the_previous_control_point() {
    let segments = parse_path_data("M0 0 C0 10 10 10 10 0 S20 -10 20 0");

    assert_eq!(
      segments[2],
      PathSegment::CubicTo {
        x1: 10.0,
        y1: -10.0,
        x2: 20.0,
        y2: -10.0,
        x: 20.0,
        y: 0.0
      }
    );
  }

  #[test]
  fn arc_flags_need_no_separators() {
    let segments = parse_path_data("M0 0 a5 5 0 1110 0");

    assert!(segments.len() > 2);
    assert!(matches!(
      segments.last(),
      Some(PathSegment::CubicTo { x, .. }) if (*x - 10.0).abs() < 1e-9
    ));
  }
}
