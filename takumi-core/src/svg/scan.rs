//! The SVG attribute grammar for numbers, lengths, and lists of them, after Blink's
//! `svg_parser_utilities.cc` (`ParseNumber`, `SkipOptionalSVGSpaces`, `SkipOptionalSVGSpacesOrDelimiter`).

use super::length::{Length, LengthUnit};

/// A cursor over an attribute value.
#[derive(Debug, Clone)]
pub struct Scanner<'a> {
  text: &'a str,
  position: usize,
}

impl<'a> Scanner<'a> {
  /// A scanner at the start of `text`.
  pub fn new(text: &'a str) -> Self {
    Self { text, position: 0 }
  }

  /// The unread rest of the value.
  pub fn rest(&self) -> &'a str {
    &self.text[self.position..]
  }

  /// Whether the whole value is read.
  pub fn is_at_end(&self) -> bool {
    self.position >= self.text.len()
  }

  /// The next byte, unread.
  pub fn peek(&self) -> Option<u8> {
    self.text.as_bytes().get(self.position).copied()
  }

  /// Skips `count` bytes, which must end on a character boundary.
  pub fn advance(&mut self, count: usize) {
    self.position = (self.position + count).min(self.text.len());
  }

  /// Reads `byte` when it is next.
  pub fn eat(&mut self, byte: u8) -> bool {
    if self.peek() == Some(byte) {
      self.position += 1;
      true
    } else {
      false
    }
  }

  /// Skips XML white space.
  pub fn skip_spaces(&mut self) {
    while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r' | 0x0c)) {
      self.position += 1;
    }
  }

  /// Skips white space and at most one comma.
  pub fn skip_comma_spaces(&mut self) {
    self.skip_spaces();
    if self.eat(b',') {
      self.skip_spaces();
    }
  }

  /// Reads a number, then the white space and comma after it.
  pub fn number(&mut self) -> Option<f32> {
    self.number_f64().map(|value| value as f32)
  }

  /// Reads a number at double precision, then the white space and comma after it.
  pub fn number_f64(&mut self) -> Option<f64> {
    self.skip_spaces();

    let value = self.raw_number()?;

    self.skip_comma_spaces();
    Some(value)
  }

  /// Reads a number with nothing after it.
  fn raw_number(&mut self) -> Option<f64> {
    let bytes = self.text.as_bytes();
    let start = self.position;
    let mut end = start;

    if matches!(bytes.get(end), Some(b'+' | b'-')) {
      end += 1;
    }

    let integer_start = end;

    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
      end += 1;
    }

    let mut digits = end - integer_start;

    if bytes.get(end) == Some(&b'.') {
      end += 1;

      let fraction_start = end;

      while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
      }
      digits += end - fraction_start;
    }
    if digits == 0 {
      return None;
    }
    if matches!(bytes.get(end), Some(b'e' | b'E'))
      && !matches!(bytes.get(end + 1), Some(b'm' | b'x'))
    {
      let mut exponent = end + 1;

      if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
        exponent += 1;
      }

      let exponent_start = exponent;

      while bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
        exponent += 1;
      }
      if exponent > exponent_start {
        end = exponent;
      }
    }

    let value = self.text[start..end].parse::<f64>().ok()?;

    if !value.is_finite() {
      return None;
    }

    self.position = end;
    Some(value)
  }

  /// Reads a length, then the white space and comma after it.
  pub fn length(&mut self) -> Option<Length> {
    let length = self.bare_length()?;

    self.skip_comma_spaces();
    Some(length)
  }

  fn bare_length(&mut self) -> Option<Length> {
    self.skip_spaces();

    let number = self.raw_number()? as f32;
    let unit = self.unit();

    Some(Length { number, unit })
  }

  fn unit(&mut self) -> LengthUnit {
    let rest = self.rest();
    let (unit, length) = if rest.starts_with('%') {
      (LengthUnit::Percent, 1)
    } else if let Some(prefix) = rest.get(..2) {
      match prefix {
        "em" => (LengthUnit::Em, 2),
        "ex" => (LengthUnit::Ex, 2),
        "px" => (LengthUnit::Px, 2),
        "in" => (LengthUnit::In, 2),
        "cm" => (LengthUnit::Cm, 2),
        "mm" => (LengthUnit::Mm, 2),
        "pt" => (LengthUnit::Pt, 2),
        "pc" => (LengthUnit::Pc, 2),
        _ => (LengthUnit::None, 0),
      }
    } else {
      (LengthUnit::None, 0)
    };

    self.position += length;
    unit
  }
}

/// A number that fills `text`.
pub fn parse_number(text: &str) -> Option<f32> {
  let mut scanner = Scanner::new(text);

  scanner.skip_spaces();

  let value = scanner.raw_number()? as f32;

  scanner.skip_spaces();
  scanner.is_at_end().then_some(value)
}

/// A length that fills `text`.
pub fn parse_length(text: &str) -> Option<Length> {
  let mut scanner = Scanner::new(text);
  let value = scanner.bare_length()?;

  scanner.skip_spaces();
  scanner.is_at_end().then_some(value)
}

/// Numbers separated by white space or commas, or `None` when one is malformed.
pub fn parse_number_list(text: &str) -> Option<Vec<f32>> {
  let mut scanner = Scanner::new(text);
  let mut list = Vec::new();

  scanner.skip_spaces();
  while !scanner.is_at_end() {
    list.push(scanner.number()?);
  }
  Some(list)
}

/// Lengths separated by white space or commas, or `None` when one is malformed.
pub fn parse_length_list(text: &str) -> Option<Vec<Length>> {
  let mut scanner = Scanner::new(text);
  let mut list = Vec::new();

  scanner.skip_spaces();
  while !scanner.is_at_end() {
    list.push(scanner.length()?);
  }
  Some(list)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn numbers_follow_the_svg_grammar() {
    assert_eq!(parse_number(" 1.5 "), Some(1.5));
    assert_eq!(parse_number("-.5e1"), Some(-5.0));
    assert_eq!(parse_number("1e"), None);
    assert_eq!(parse_number("1px"), None);
    assert_eq!(
      parse_number_list("1-2.5.5,3"),
      Some(vec![1.0, -2.5, 0.5, 3.0])
    );
    assert_eq!(parse_number_list("1 x"), None);
  }

  #[test]
  fn lengths_carry_their_unit() {
    assert_eq!(
      parse_length("2em"),
      Some(Length {
        number: 2.0,
        unit: LengthUnit::Em
      })
    );
    assert_eq!(
      parse_length("1.5e1%").map(|length| (length.number, length.unit)),
      Some((15.0, LengthUnit::Percent))
    );
    assert_eq!(parse_length("3 px"), None);
    assert_eq!(parse_length_list("1,2cm 3").map(|list| list.len()), Some(3));
  }
}
