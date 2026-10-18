/// Tile geometry of background and mask layers.
pub mod background_image_geometry;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: the inline layout algorithm port places floats"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod bfc_rect;
/// Backend-agnostic border geometry shared across rasterization backends.
pub mod border;
pub mod clip;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: the inline layout algorithm port places floats"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod constraint_space;
pub(crate) mod contoured_rect;
pub(crate) mod corner_shape;
/// Backend-agnostic box-decoration clip geometry.
pub mod decoration;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: the inline layout algorithm port places floats"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod exclusions;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: the inline layout algorithm port places floats"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod floats_utils;
/// Inline-level layout: text shaping, line breaking, and text fitting.
pub mod inline;
/// Resolving what an inline box paints.
pub mod inline_box;
/// Where a glyph outline crosses a horizontal band.
pub mod intercept;
/// Generated marker boxes for `display: list-item`.
mod list_marker;
pub mod measure;
/// Node Tree
pub mod node;
/// Where replaced content sits inside its content box.
pub mod replaced;
/// Lowering `display: table` onto the grid layout algorithm.
mod table;

mod table_borders;
mod table_columns;
/// Layout tree: render nodes and their computed layout results.
pub mod tree;
