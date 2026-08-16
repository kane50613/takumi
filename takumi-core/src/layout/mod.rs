/// Resolving where a background layer's tiles land.
pub mod background;
/// Backend-agnostic border geometry shared across rasterization backends.
pub mod border;
pub mod clip;
pub(crate) mod corner_shape;
/// Backend-agnostic box-decoration clip geometry.
pub mod decoration;
/// Inline-level layout: text shaping, line breaking, and text fitting.
pub mod inline;
/// Resolving what an inline box paints.
pub mod inline_box;
/// Where a glyph outline crosses a horizontal band.
pub mod intercept;
/// Generated marker boxes for `display: list-item`.
mod list_marker;
/// Node Tree
pub mod node;
/// Where replaced content sits inside its content box.
pub mod replaced;
/// Lowering `display: table` onto the grid layout algorithm.
mod table;
/// Layout tree: render nodes and their computed layout results.
pub mod tree;
