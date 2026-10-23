mod grid_area;
mod grid_auto_flow;
mod grid_length;
mod grid_line;
mod grid_min_max_size;
mod grid_placement;
mod grid_repeat_track;
mod grid_repetition_count;
mod grid_shorthand;
mod grid_template;
mod grid_template_areas;
mod grid_template_component;
mod grid_track_size;

pub use grid_area::*;
pub use grid_auto_flow::*;
pub use grid_length::*;
pub use grid_line::*;
pub use grid_min_max_size::*;
pub use grid_placement::*;
pub use grid_repeat_track::*;
pub use grid_repetition_count::*;
pub use grid_shorthand::*;
pub use grid_template::*;
pub use grid_template_areas::*;
pub use grid_template_component::*;
pub use grid_track_size::*;

use std::fmt;

use cssparser::{Parser, Token};

use crate::style::ToCss;

/// Whether `input` is at the end of a declaration's value, before any `!important`.
pub(crate) fn at_value_end(input: &mut Parser<'_>) -> bool {
  if input.is_exhausted() {
    return true;
  }

  let state = input.state();
  let is_bang = matches!(input.next(), Ok(Token::Delim('!')));

  input.reset(&state);
  is_bang
}

pub(crate) fn write_space_separated<W: fmt::Write, T: ToCss>(
  dest: &mut W,
  items: &[T],
) -> fmt::Result {
  for (index, item) in items.iter().enumerate() {
    if index > 0 {
      dest.write_char(' ')?;
    }

    item.to_css(dest)?;
  }

  Ok(())
}
