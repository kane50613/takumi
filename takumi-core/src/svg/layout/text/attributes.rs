//! The `x`, `y`, `dx`, `dy` and `rotate` attributes of the elements of a `text`, resolved to one
//! value per character, after usvg's `resolve_positions_list` and `resolve_rotate_list`.

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

impl TextTree {
  /// The position of each character, where a character is a code point. The values of an element
  /// reach no further than its own characters, so `<tspan x="10 20 30">ab</tspan>c` gives `c`
  /// none.
  pub(super) fn character_positions(
    &self,
    layout: &SvgLayout,
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
      let mut assign = |name: &str, axis: LengthAxis, set: fn(&mut CharacterPosition, f32)| {
        if let Some(lengths) = layout.length_list(styled.index, name) {
          for (position, length) in positions[offset..]
            .iter_mut()
            .zip(&lengths)
            .take(characters)
          {
            set(position, layout.px(styled, scope, *length, axis));
          }
        }
      };

      assign("x", LengthAxis::X, |position, value| {
        position.x = Some(value)
      });
      assign("y", LengthAxis::Y, |position, value| {
        position.y = Some(value)
      });
      assign("dx", LengthAxis::X, |position, value| {
        position.dx = Some(value)
      });
      assign("dy", LengthAxis::Y, |position, value| {
        position.dy = Some(value)
      });
    }

    positions
  }

  /// The rotation of each character in degrees. A character the `rotate` list of its element does
  /// not reach takes the last angle of the list.
  pub(super) fn rotations(&self, layout: &SvgLayout) -> Vec<f32> {
    let mut rotations = vec![0.0; self.count_chars(0)];
    let mut last = 0.0;
    let mut offset = 0;

    for id in self.descendants(0) {
      let Some(styled) = self.element(id) else {
        offset += self.count_chars(id);
        continue;
      };

      let Some(angles) = layout.number_list(styled.index, "rotate") else {
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
  /// The lengths a list attribute holds, up to the first that does not parse.
  fn length_list(&self, index: usize, name: &str) -> Option<Vec<Length>> {
    let mut parser = Parser::new(self.attribute(index, name)?);
    let mut lengths = Vec::new();

    while !parser.is_exhausted() {
      let Ok(length) = Length::from_css(&mut parser) else {
        break;
      };

      lengths.push(length);

      let _ = parser.try_parse(Parser::expect_comma);
    }

    Some(lengths)
  }
}
