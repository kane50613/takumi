/// Tile geometry of background and mask layers.
pub mod background_image_geometry;
pub(crate) mod bfc_rect;
pub(crate) mod block_layout_algorithm;
pub(crate) mod block_node;
/// Backend-agnostic border geometry shared across rasterization backends.
pub mod border;
pub mod clip;
pub(crate) mod constraint_space;
pub(crate) mod contoured_rect;
pub(crate) mod corner_shape;
/// Backend-agnostic box-decoration clip geometry.
pub mod decoration;
pub(crate) mod exclusions;
pub(crate) mod flex;
pub(crate) mod floats_utils;
pub(crate) mod grid;
/// Inline-level layout: text shaping, line breaking, and text fitting.
pub mod inline;
/// Resolving what an inline box paints.
pub mod inline_box;
/// Where a glyph outline crosses a horizontal band.
pub mod intercept;
pub(crate) mod layout_result;
pub(crate) mod layout_style;
pub(crate) mod length_utils;
/// Generated marker boxes for `display: list-item`.
pub(crate) mod list_marker;
pub(crate) mod logical_rect;
pub(crate) mod margin_strut;
pub mod measure;
pub(crate) mod min_max_sizes;
/// Node Tree
pub mod node;
pub(crate) mod out_of_flow;
pub(crate) mod relative_utils;
/// Where replaced content sits inside its content box.
pub mod replaced;
/// CSS tables, laid out by Blink's table algorithm.
pub mod table;
pub(crate) mod taffy_bridge;
/// Layout tree: render nodes and their computed layout results.
pub mod tree;
