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

pub use error::{Error, Result, StyleSheetParseError};

use typed_builder::TypedBuilder;
use xxhash_rust::xxh3::Xxh3DefaultBuilder;

use crate::resources::{font::FontContext, image::PersistentImageStore};

/// The main context for image rendering.
///
/// This struct holds all the necessary state for rendering images, including
/// font management, image storage, and debug options.
#[derive(Default, TypedBuilder)]
#[builder(field_defaults(default))]
pub struct GlobalContext {
  /// The font context for text rendering
  pub font_context: FontContext,
  /// The image store for persisting contents
  pub persistent_image_store: PersistentImageStore,
}

/// Type alias for HashSet using XXH3 hasher
pub type Xxh3HashSet<T> = HashSet<T, Xxh3DefaultBuilder>;
