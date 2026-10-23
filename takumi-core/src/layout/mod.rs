/// Tile geometry of background and mask layers.
pub mod background_image_geometry;
pub(crate) mod bfc_rect;
#[cfg(feature = "fragmentation")]
pub(crate) mod block_break_token;
pub(crate) mod block_layout_algorithm;
pub(crate) mod block_node;
/// Backend-agnostic border geometry shared across rasterization backends.
pub mod border;
#[cfg(feature = "fragmentation")]
pub(crate) mod box_fragment_builder;
#[cfg(feature = "fragmentation")]
pub(crate) mod break_appeal;
#[cfg(feature = "fragmentation")]
pub(crate) mod break_token;
#[cfg(feature = "fragmentation")]
pub(crate) mod break_token_algorithm_data;
pub mod clip;
pub(crate) mod constraint_space;
pub(crate) mod content_alignment;
pub(crate) mod contoured_rect;
pub(crate) mod corner_shape;
/// Backend-agnostic box-decoration clip geometry.
pub mod decoration;
#[cfg(feature = "fragmentation")]
pub(crate) mod early_break;
pub(crate) mod exclusions;
pub(crate) mod flex;
pub(crate) mod floats_utils;
pub(crate) mod fragmentation_layout_input;
#[cfg(all(test, feature = "fragmentation"))]
mod fragmentation_tests;
#[cfg(feature = "fragmentation")]
pub(crate) mod fragmentation_utils;
pub(crate) mod grid;
/// Inline-level layout: text shaping, line breaking, and text fitting.
pub mod inline;
/// Resolving what an inline box paints.
pub mod inline_box;
/// Where a glyph outline crosses a horizontal band.
pub mod intercept;
pub(crate) mod layout_result;
pub(crate) mod layout_style;
/// A layout read while it is made or after.
pub mod layout_view;
pub(crate) mod length_utils;
pub(crate) mod line_clamp_data;
/// Generated marker boxes for `display: list-item`.
pub(crate) mod list_marker;
pub(crate) mod logical_rect;
pub(crate) mod margin_strut;
pub mod measure;
pub(crate) mod min_max_sizes;
/// Node Tree
pub mod node;
pub(crate) mod out_of_flow;
/// A render tree laid out across pages, one page at a time.
#[cfg(feature = "fragmentation")]
pub mod paged_layout;
#[cfg(all(test, feature = "fragmentation"))]
mod paged_layout_oracles;
pub(crate) mod relative_utils;
/// Where replaced content sits inside its content box.
pub mod replaced;
/// CSS tables, laid out by Blink's table algorithm.
pub mod table;
/// Layout tree: render nodes and their computed layout results.
pub mod tree;
