//! The SVG `transform` attribute grammar, after Blink's `SVGTransformList` parser.

use tiny_skia::Transform;

use super::scan::Scanner;

/// Parses a transform list into one matrix, or `None` when the list is malformed. An empty list
/// is the identity.
pub fn parse_transform_list(text: &str) -> Option<Transform> {
  let mut scanner = Scanner::new(text);
  let mut matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

  scanner.skip_spaces();
  while !scanner.is_at_end() {
    let name = identifier(&mut scanner);

    scanner.skip_spaces();
    if !scanner.eat(b'(') {
      return None;
    }

    match name {
      "matrix" => {
        let mut values = [0.0; 6];

        for value in &mut values {
          *value = scanner.number_f64()?;
        }
        matrix = multiply(matrix, values);
      }
      "translate" => {
        let x = scanner.number_f64()?;
        let y = optional_number(&mut scanner)?.unwrap_or(0.0);

        matrix = multiply(matrix, [1.0, 0.0, 0.0, 1.0, x, y]);
      }
      "scale" => {
        let x = scanner.number_f64()?;
        let y = optional_number(&mut scanner)?.unwrap_or(x);

        matrix = multiply(matrix, [x, 0.0, 0.0, y, 0.0, 0.0]);
      }
      "rotate" => {
        let angle = scanner.number_f64()?;
        let center = match optional_number(&mut scanner)? {
          Some(x) => Some((x, scanner.number_f64()?)),
          None => None,
        };

        if let Some((x, y)) = center {
          matrix = multiply(matrix, [1.0, 0.0, 0.0, 1.0, x, y]);
        }

        let radians = angle.to_radians();
        let (sin, cos) = (radians.sin(), radians.cos());

        matrix = multiply(matrix, [cos, sin, -sin, cos, 0.0, 0.0]);
        if let Some((x, y)) = center {
          matrix = multiply(matrix, [1.0, 0.0, 0.0, 1.0, -x, -y]);
        }
      }
      "skewX" => {
        let skew = scanner.number_f64()?.to_radians().tan();

        matrix = multiply(matrix, [1.0, 0.0, skew, 1.0, 0.0, 0.0]);
      }
      "skewY" => {
        let skew = scanner.number_f64()?.to_radians().tan();

        matrix = multiply(matrix, [1.0, skew, 0.0, 1.0, 0.0, 0.0]);
      }
      _ => return None,
    }

    scanner.skip_spaces();
    if !scanner.eat(b')') {
      return None;
    }
    scanner.skip_comma_spaces();
  }

  let [a, b, c, d, e, f] = matrix;

  Some(Transform::from_row(
    a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
  ))
}

fn identifier<'a>(scanner: &mut Scanner<'a>) -> &'a str {
  let rest = scanner.rest();
  let length = rest
    .bytes()
    .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    .count();

  scanner.advance(length);
  &rest[..length]
}

/// The next number, or `Some(None)` at the closing parenthesis, or `None` when it is malformed.
fn optional_number(scanner: &mut Scanner) -> Option<Option<f64>> {
  scanner.skip_spaces();
  if scanner.peek() == Some(b')') {
    return Some(None);
  }
  scanner.number_f64().map(Some)
}

fn multiply(first: [f64; 6], second: [f64; 6]) -> [f64; 6] {
  let [a1, b1, c1, d1, e1, f1] = first;
  let [a2, b2, c2, d2, e2, f2] = second;

  [
    a1 * a2 + c1 * b2,
    b1 * a2 + d1 * b2,
    a1 * c2 + c1 * d2,
    b1 * c2 + d1 * d2,
    a1 * e2 + c1 * f2 + e1,
    b1 * e2 + d1 * f2 + f1,
  ]
}

#[cfg(test)]
mod tests {
  use super::*;

  fn matrix(text: &str) -> Option<[f32; 6]> {
    parse_transform_list(text).map(|ts| [ts.sx, ts.ky, ts.kx, ts.sy, ts.tx, ts.ty])
  }

  #[test]
  fn functions_compose_left_to_right() {
    assert_eq!(
      matrix("translate(10 15) translate(0,5)"),
      Some([1.0, 0.0, 0.0, 1.0, 10.0, 20.0])
    );
    assert_eq!(
      matrix("translate(10)scale(2)"),
      Some([2.0, 0.0, 0.0, 2.0, 10.0, 0.0])
    );
  }

  #[test]
  fn rotate_about_a_center_is_a_conjugation() {
    let [sx, ky, kx, sy, tx, ty] = matrix("rotate(30 10 20)").unwrap();

    assert!((sx - 0.866_025_4).abs() < 1e-6);
    assert!((ky - 0.5).abs() < 1e-6);
    assert!((kx + 0.5).abs() < 1e-6);
    assert!((sy - 0.866_025_4).abs() < 1e-6);
    assert!((tx - 11.339_746).abs() < 1e-5);
    assert!((ty + 2.320_508).abs() < 1e-5);
  }

  #[test]
  fn malformed_lists_fail_and_empty_ones_are_the_identity() {
    assert_eq!(matrix(""), Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    assert_eq!(matrix("translate(1"), None);
    assert_eq!(matrix("shear(1)"), None);
    assert_eq!(matrix("scale(1 2 3)"), None);
  }
}
