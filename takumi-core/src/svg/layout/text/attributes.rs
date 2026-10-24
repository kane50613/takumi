//! The `x`, `y`, `dx`, `dy` and `rotate` attributes of the elements of a `text`, resolved to one
//! value per character, after usvg's `resolve_positions_list` and `resolve_rotate_list`.

use std::rc::Rc;

use cssparser::Parser;

use super::{Scope, SvgLayout, chunk::TextTree};
use crate::{
  style::{FromCss, Length},
  svg::length::LengthAxis,
};

/// The position attributes that apply to one character.
#[derive(Clone, Copy, Default)]
pub(super) struct CharacterPosition {
  /// An absolute x position.
  pub(super) x: Option<f32>,
  /// An absolute y position.
  pub(super) y: Option<f32>,
  /// A relative x offset.
  pub(super) dx: Option<f32>,
  /// A relative y offset.
  pub(super) dy: Option<f32>,
}

/// The position and rotation lists of one element, parsed once for every instance of it.
pub(in crate::svg::layout) struct PositionLists {
  x: Box<[Length]>,
  y: Box<[Length]>,
  dx: Box<[Length]>,
  dy: Box<[Length]>,
  rotate: Option<Box<[f32]>>,
}

impl PositionLists {
  fn parse(layout: &SvgLayout, index: usize) -> Self {
    let lengths = |name| {
      layout
        .attribute(index, name)
        .map(length_list)
        .unwrap_or_default()
    };

    Self {
      x: lengths("x"),
      y: lengths("y"),
      dx: lengths("dx"),
      dy: lengths("dy"),
      rotate: layout.attribute(index, "rotate").and_then(number_list),
    }
  }
}

impl TextTree {
  /// The position of each character, where a character is a code point. The values of an element
  /// reach no further than its own characters, so `<tspan x="10 20 30">ab</tspan>c` gives `c`
  /// none.
  pub(super) fn character_positions(
    &self,
    layout: &mut SvgLayout,
    scope: &Scope,
  ) -> Vec<CharacterPosition> {
    let mut positions = vec![CharacterPosition::default(); self.count_chars(0)];
    let mut offset = 0;

    for id in self.descendants(0) {
      let Some(styled) = self.element(id) else {
        offset += self.count_chars(id);
        continue;
      };

      // The positions of a `textPath` are ignored.
      if self.is_text_path(id) {
        continue;
      }

      let characters = self.count_chars(id);
      let lists = layout.position_lists(styled.index);
      let mut assign =
        |lengths: &[Length], axis: LengthAxis, set: fn(&mut CharacterPosition, f32)| {
          for (position, length) in positions[offset..].iter_mut().zip(lengths).take(characters) {
            set(position, layout.px(styled, scope, *length, axis));
          }
        };

      assign(&lists.x, LengthAxis::X, |position, value| {
        position.x = Some(value)
      });
      assign(&lists.y, LengthAxis::Y, |position, value| {
        position.y = Some(value)
      });
      assign(&lists.dx, LengthAxis::X, |position, value| {
        position.dx = Some(value)
      });
      assign(&lists.dy, LengthAxis::Y, |position, value| {
        position.dy = Some(value)
      });
    }

    positions
  }

  /// The rotation of each character in degrees. A character the `rotate` list of its element does
  /// not reach takes the last angle of the list.
  pub(super) fn rotations(&self, layout: &mut SvgLayout) -> Vec<f32> {
    let mut rotations = vec![0.0; self.count_chars(0)];
    let mut last = 0.0;
    let mut offset = 0;

    for id in self.descendants(0) {
      let Some(styled) = self.element(id) else {
        offset += self.count_chars(id);
        continue;
      };

      let lists = layout.position_lists(styled.index);
      let Some(angles) = &lists.rotate else {
        continue;
      };

      for index in 0..self.count_chars(id) {
        last = angles.get(index).copied().unwrap_or(last);
        rotations[offset + index] = last;
      }
    }

    rotations
  }
}

impl SvgLayout<'_> {
  fn position_lists(&mut self, index: usize) -> Rc<PositionLists> {
    if let Some(lists) = self.position_lists.get(&index) {
      return Rc::clone(lists);
    }

    let lists = Rc::new(PositionLists::parse(self, index));

    self.position_lists.insert(index, Rc::clone(&lists));
    lists
  }
}

/// The lengths a list holds, up to the first that does not parse.
fn length_list(text: &str) -> Box<[Length]> {
  let mut parser = Parser::new(text);
  let mut lengths = Vec::new();

  while !parser.is_exhausted() {
    let Ok(length) = Length::from_css(&mut parser) else {
      break;
    };

    lengths.push(length);

    let _ = parser.try_parse(Parser::expect_comma);
  }

  lengths.into()
}

/// The numbers a list holds, none unless every one parses.
fn number_list(text: &str) -> Option<Box<[f32]>> {
  let mut parser = Parser::new(text);
  let mut numbers = Vec::new();

  while !parser.is_exhausted() {
    numbers.push(parser.expect_number().ok()?);

    let _ = parser.try_parse(Parser::expect_comma);
  }

  Some(numbers.into())
}
