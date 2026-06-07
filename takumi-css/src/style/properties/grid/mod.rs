mod grid_area;
mod grid_auto_flow;
mod grid_length;
mod grid_line;
mod grid_min_max_size;
mod grid_placement;
mod grid_repeat_track;
mod grid_repetition_count;
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
pub use grid_template_areas::*;
pub use grid_template_component::*;
pub use grid_track_size::*;

pub(crate) fn write_space_separated<W: std::fmt::Write>(
  dest: &mut W,
  items: &[String],
) -> std::fmt::Result {
  let mut first = true;
  for item in items {
    if !first {
      dest.write_str(" ")?;
    }
    first = false;
    dest.write_str(item)?;
  }
  Ok(())
}
