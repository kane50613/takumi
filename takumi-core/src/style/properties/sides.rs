use std::{array, fmt};

use cssparser::Parser;

use crate::{
  geometry::Rect,
  style::{
    Animatable, Color, CssExpectedMessage, FromCss, Length, MakeComputed, ParseResult,
    SizingContext, ToCss,
  },
};

/// Represents the values for the four sides of a box (top, right, bottom, left).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sides<T>(pub [T; 4]);

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

impl<'i, T: Clone + for<'j> FromCss<'j>> FromCss<'i> for Sides<T> {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let top = T::from_css(input)?;
    let Ok(right) = input.try_parse(T::from_css) else {
      return Ok(Sides::from(top));
    };
    let Ok(bottom) = input.try_parse(T::from_css) else {
      return Ok(Sides([top.clone(), right.clone(), top, right]));
    };
    let Ok(left) = input.try_parse(T::from_css) else {
      return Ok(Sides([top, right.clone(), bottom, right]));
    };

    Ok(Sides([top, right, bottom, left]))
  }

  const VALID_TOKENS: &'static [&'static str] = T::VALID_TOKENS;

  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::OneToFourValues;
}

impl<T> From<Sides<T>> for Rect<T> {
  fn from(Sides([top, right, bottom, left]): Sides<T>) -> Self {
    Rect {
      top,
      right,
      bottom,
      left,
    }
  }
}

impl<T: Default> Default for Sides<T> {
  fn default() -> Self {
    Self(array::from_fn(|_| T::default()))
  }
}

impl<T: Clone> From<T> for Sides<T> {
  fn from(value: T) -> Self {
    Self(array::from_fn(|_| value.clone()))
  }
}

impl<T: MakeComputed> MakeComputed for Sides<T> {
  fn make_computed(&mut self, sizing: &SizingContext) {
    for value in &mut self.0 {
      value.make_computed(sizing);
    }
  }
}

impl<T: Animatable> Animatable for Sides<T> {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    for ((value, from), to) in self.0.iter_mut().zip(&from.0).zip(&to.0) {
      value.interpolate(from, to, progress, sizing, current_color);
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

impl<T: ToCss> ToCss for Sides<T> {
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
  fn deserialize_axis_pair_lengths() {
    assert_eq!(
      Sides::<Length>::from_css_str("10px 20px"),
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
