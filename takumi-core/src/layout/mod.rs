/// Backend-agnostic border geometry shared across rasterization backends.
pub mod border;
/// Backend-agnostic box-decoration clip geometry.
pub mod decoration;
/// Inline-level layout: text shaping, line breaking, and text fitting.
pub mod inline;
/// Node Tree
pub mod node;
/// Layout tree: render nodes and their computed layout results.
pub mod tree;
