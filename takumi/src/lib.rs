#![doc(
  html_logo_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg",
  html_favicon_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg"
)]
#![deny(missing_docs)]
#![deny(clippy::all)]
#![deny(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::use_self)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_const_for_fn)]

//! Takumi is a library with different parts to render your React components to images. This crate contains the core logic for layout, rendering.
//!
//! Checkout the [Integrations](https://takumi.kane.tw/docs/integrations) page if you are looking for Node.js / WASM bindings.
//!
//! # Walkthrough
//!
//! Create a [`GlobalContext`](crate::GlobalContext) to store image resources, font caches, the instance should be reused to speed up the rendering.
//!
//! Then call [`render`](crate::rendering::render) with [`Node`](crate::layout::node::Node) and [`Viewport`](crate::layout::viewport::Viewport) to get [`RgbaImage`](image::RgbaImage).
//!
//! Theres a helper function [`write_image`](crate::rendering::write::write_image) to write the image to a destination implements [`Write`](std::io::Write) and [`Seek`](std::io::Seek).
//!
//! # Example
//!
//! ```rust
//! use takumi::{
//!   layout::{
//!     node::{ContainerNode, TextNode, NodeKind, Node},
//!     Viewport,
//!     style::Style,
//!   },
//!   rendering::{render, RenderOptionsBuilder},
//!   GlobalContext,
//! };
//!
//! // Create a node tree with `ContainerNode` and `TextNode`
//! let mut node = NodeKind::Container(ContainerNode {
//!   children: Some(vec![
//!     NodeKind::Text(TextNode {
//!       text: "Hello, world!".to_string(),
//!       style: None, // Construct with `StyleBuilder`
//!       tw: None, // Tailwind properties
//!     }),
//!   ]),
//!   style: None,
//!   tw: None, // Tailwind properties
//! });
//!
//! // Create a context for storing resources, font caches.
//! // You should reuse the context to speed up the rendering.
//! let mut global = GlobalContext::default();
//!
//! // Load fonts
//! // pass an optional [`FontInfoOverride`](parley::FontInfoOverride) to override the font's metadata,
//! // and an optional [`GenericFamily`](parley::GenericFamily) to specify the generic family of the font.
//! global.font_context.load_and_store(include_bytes!("../../assets/fonts/geist/Geist[wght].woff2"), None, None);
//!
//! // Create a viewport
//! let viewport = Viewport::new(1200, 630);
//!
//! // Create render options
//! let options = RenderOptionsBuilder::default()
//!   .viewport(viewport)
//!   .node(node)
//!   .global(&global)
//!   .build()
//!   .unwrap();
//!
//! // Render the layout to an `RgbaImage`
//! let image = render(options).unwrap();
//! ```
//!
//! # Feature Flags
//!
//! - `woff2`: Enable WOFF2 font support.
//! - `woff`: Enable WOFF font support.
//! - `svg`: Enable SVG support.
//! - `rayon`: Enable rayon support.
//! - `avif`: Enable AVIF support.
//!
//! # Credits
//!
//! - [taffy](https://github.com/DioxusLabs/taffy) for the layout system.
//! - [image](https://github.com/image-rs/image) for the image processing.
//! - [parley](https://github.com/linebender/parley) for the text layout.
//! - [wuff](https://github.com/nicoburns/wuff) for woff/woff2 decompression.
//! - [ts-rs](https://github.com/AlephAlpha/ts-rs) for the type-safe serialization.
//! - [resvg](https://github.com/linebender/resvg) for SVG parsing and rendering.

/// Layout related modules, including the node tree, style parsing, and layout calculation.
pub mod layout;

/// Rendering related modules, including the image renderer, canvas operations.
pub mod rendering;

/// External resource management (fonts, images)
pub mod resources;

pub use image;
pub use parley;
pub use taffy;

use crate::resources::{
  font::FontContext,
  image::{ImageResourceError, PersistentImageStore},
};

/// The main context for image rendering.
///
/// This struct holds all the necessary state for rendering images, including
/// font management, image storage, and debug options.
#[derive(Default)]
pub struct GlobalContext {
  /// The font context for text rendering
  pub font_context: FontContext,
  /// The image store for persisting contents
  pub persistent_image_store: PersistentImageStore,
}

/// Represents errors that can occur.
#[derive(Debug)]
pub enum Error {
  /// Represents an error that occurs during image resolution.
  ImageResolveError(ImageResourceError),
  /// Represents an error that occurs during IO operations.
  IoError(std::io::Error),
  /// Represents an error that occurs during PNG encoding.
  PngError(png::EncodingError),
}

impl From<std::io::Error> for Error {
  fn from(error: std::io::Error) -> Self {
    Error::IoError(error)
  }
}

impl From<png::EncodingError> for Error {
  fn from(error: png::EncodingError) -> Self {
    Error::PngError(error)
  }
}
