use std::fmt;

use cssparser::Parser;
#[cfg(feature = "unstable")]
use taffy::Rect;

use crate::style::{
  CssExpectedMessage, CssToken, FromCss, Length, MakeComputed, ParseResult, SizingContext, ToCss,
};

/// Represents the values for the four sides of a box (top, right, bottom, left).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sides<T: Copy>(pub [T; 4]);

/// One of the two box axes.
pub enum Axis {
  /// The horizontal (left/right) axis.
  Horizontal,
  /// The vertical (top/bottom) axis.
  Vertical,
}

impl<T: Copy> Sides<T> {
  /// Maps each side to a new value, passing its axis.
  pub fn map_axis<R: Copy, F: Fn(T, Axis) -> R>(&self, func: F) -> Sides<R> {
    let [top, right, bottom, left] = self.0;

    Sides([
      func(top, Axis::Vertical),
      func(right, Axis::Horizontal),
      func(bottom, Axis::Vertical),
      func(left, Axis::Horizontal),
    ])
  }
}

impl<'i, T: Copy + for<'j> FromCss<'j>> FromCss<'i> for Sides<T> {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Parse between 1 and 4 values of T using FromCss
    let first = T::from_css(input)?;

    // Collect all values by parsing until we can't parse more
    let mut values = Vec::with_capacity(4);

    values.push(first);

    // Keep parsing values separated by whitespace
    loop {
      // Try to parse the next value
      match input.try_parse(T::from_css) {
        Ok(next_value) => values.push(next_value),
        Err(_) => break,
      }

      // Don't allow more than 4 values
      if values.len() >= 4 {
        break;
      }
    }

    // Now create the sides based on how many values we got
    let sides = match values.len() {
      1 => Sides([values[0]; 4]),
      2 => Sides([values[0], values[1], values[0], values[1]]),
      3 => Sides([values[0], values[1], values[2], values[1]]),
      _ => Sides([values[0], values[1], values[2], values[3]]),
    };

    Ok(sides)
  }

  const VALID_TOKENS: &'static [CssToken] = T::VALID_TOKENS;

  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::OneToFourValues;
}

#[cfg(feature = "unstable")]
impl<T: Copy> From<Sides<T>> for Rect<T> {
  fn from(value: Sides<T>) -> Self {
    Rect {
      top: value.0[0],
      right: value.0[1],
      bottom: value.0[2],
      left: value.0[3],
    }
  }
}

impl<T: Default + Copy> Default for Sides<T> {
  fn default() -> Self {
    Self([T::default(); 4])
  }
}

impl<T: Copy> From<T> for Sides<T> {
  fn from(value: T) -> Self {
    Self([value; 4])
  }
}

impl<T: Copy + MakeComputed> MakeComputed for Sides<T> {
  fn make_computed(&mut self, sizing: &SizingContext) {
    for value in &mut self.0 {
      value.make_computed(sizing);
    }
  }
}

impl Sides<Length> {
  /// Creates a new zeroable Sides.
  pub const fn zero() -> Self {
    Self([Length::zero(); 4])
  }

  /// Creates a new autoable Sides.
  pub const fn auto() -> Self {
    Self([Length::Auto; 4])
  }
}

impl<T: Copy + ToCss> ToCss for Sides<T> {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.0[0].to_css(dest)?;
    dest.write_str(" ")?;
    self.0[1].to_css(dest)?;
    dest.write_str(" ")?;
    self.0[2].to_css(dest)?;
    dest.write_str(" ")?;
    self.0[3].to_css(dest)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{FromCssStr, Length};

  #[test]
  fn deserialize_single_number() {
    assert_eq!(
      Sides::<Length>::from_css_str("5"),
      Ok(Sides([Length::Px(5.0); 4]))
    );
  }

  #[test]
  fn deserialize_axis_pair_numbers() {
    assert_eq!(
      Sides::<Length>::from_css_str("10 20"),
      Ok(Sides([
        Length::Px(10.0),
        Length::Px(20.0),
        Length::Px(10.0),
        Length::Px(20.0)
      ]))
    );
  }

  #[test]
  fn deserialize_css_single_value() {
    assert_eq!(
      Sides::<Length>::from_css_str("10px"),
      Ok(Sides([Length::Px(10.0); 4]))
    );
  }

  #[test]
  fn deserialize_css_multi_values() {
    assert_eq!(
      Sides::<Length>::from_css_str("1px 2px 3px 4px"),
      Ok(Sides([
        Length::Px(1.0),
        Length::Px(2.0),
        Length::Px(3.0),
        Length::Px(4.0)
      ]))
    );
  }
}
