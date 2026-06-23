#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]
//! Backend-agnostic core for takumi: the node tree, CSS-driven style and layout,
//! and resource (font and image) management. The rendering backends
//! ([`takumi-raster`](https://docs.rs/takumi-raster) and
//! [`takumi-svg`](https://docs.rs/takumi-svg)) build on these types. Most users
//! should depend on the [`takumi`](https://docs.rs/takumi) umbrella crate instead
//! of this one directly.

/// Style resolution and box/inline layout.
pub mod layout;

/// Render context threading style, sizing, and resources through layout.
pub mod context;
/// Font style resolved against a sizing context.
pub mod font_style;
/// Box, text, and inset shadow resolution.
pub mod shadow;
pub mod text_processing;

/// Error types.
pub mod error;
pub mod geometry;
pub use takumi_css::keyframes;
/// Font and image resource management.
pub mod resources;
pub mod scene;

use std::collections::HashSet;

pub use error::{Error, Result};

use xxhash_rust::xxh3::Xxh3DefaultBuilder;

pub use crate::resources::font::Fonts;

/// Type alias for HashSet using XXH3 hasher
pub(crate) type Xxh3HashSet<T> = HashSet<T, Xxh3DefaultBuilder>;
