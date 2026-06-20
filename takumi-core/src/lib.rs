#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![allow(missing_docs)]
//! Backend-agnostic core for takumi: the node tree, style/layout, and resource
//! management.

pub mod layout;

pub mod context;
pub mod font_style;
pub mod shadow;
pub mod text_processing;

pub mod error;
pub use takumi_css::keyframes;
pub mod resources;

use std::collections::HashSet;

pub use error::{Error, Result};

use xxhash_rust::xxh3::Xxh3DefaultBuilder;

pub use crate::resources::font::Fonts;

/// Type alias for HashSet using XXH3 hasher
pub(crate) type Xxh3HashSet<T> = HashSet<T, Xxh3DefaultBuilder>;
