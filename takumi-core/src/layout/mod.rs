/// Tile geometry of background and mask layers.
pub mod background_image_geometry;
pub(crate) mod bfc_rect;
/// Backend-agnostic border geometry shared across rasterization backends.
pub mod border;
pub mod clip;
pub(crate) mod constraint_space;
pub(crate) mod contoured_rect;
pub(crate) mod corner_shape;
/// Backend-agnostic box-decoration clip geometry.
pub mod decoration;
pub(crate) mod exclusions;
pub(crate) mod floats_utils;
/// Inline-level layout: text shaping, line breaking, and text fitting.
pub mod inline;
/// Resolving what an inline box paints.
pub mod inline_box;
/// Where a glyph outline crosses a horizontal band.
pub mod intercept;
/// Generated marker boxes for `display: list-item`.
pub(crate) mod list_marker;
pub(crate) mod logical_rect;
pub mod measure;
/// Node Tree
pub mod node;
pub(crate) mod relative_utils;
/// Where replaced content sits inside its content box.
pub mod replaced;
/// CSS tables, laid out by Blink's table algorithm.
pub mod table;
/// Layout tree: render nodes and their computed layout results.
pub mod tree;
