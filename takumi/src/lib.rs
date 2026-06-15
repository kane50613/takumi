#![doc(
  html_logo_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg",
  html_favicon_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg"
)]
#![allow(missing_docs)]
//! Takumi renders UI component trees to images. This crate is a thin facade that
//! re-exports the backend-agnostic core ([`takumi-core`]) and the raster painting
//! backend ([`takumi-paint`]) so existing `takumi::…` paths keep working.
//!
//! # Example
//!
//! ```rust
//! use takumi::{
//!   layout::{
//!     node::Node,
//!     Viewport,
//!     style::{Length::Px, Style, StyleDeclaration},
//!   },
//!   resources::font::FontResource,
//!   rendering::{render, RenderOptions},
//!   GlobalContext,
//! };
//!
//! let node = Node::container([Node::text("Hello, world!").with_style(
//!   Style::default().with(StyleDeclaration::font_size(Px(32.0).into())),
//! )]);
//!
//! let mut global = GlobalContext::default();
//!
//! global.font_context.load_and_store(
//!   FontResource::new(include_bytes!("../../assets/fonts/geist/Geist[wght].woff2"))
//! );
//!
//! let viewport = Viewport::new((1200, 630));
//!
//! let options = RenderOptions::builder()
//!   .viewport(viewport)
//!   .node(node)
//!   .global(&global)
//!   .build();
//!
//! let image = render(options).unwrap();
//! ```
//!
//! # Feature Flags
//!
//! - `woff2`: Enable WOFF2 font support.
//! - `woff`: Enable WOFF font support.
//! - `svg`: Enable SVG support.
//! - `rayon`: Enable rayon support.

pub use takumi_core::{
  Error, GlobalContext, Result, StyleSheetParseError, Xxh3HashSet, context, error, font_style,
  keyframes, layout, resources, shadow, text_processing,
};

/// Rendering: the image renderer, canvas operations, and output encoding.
pub use takumi_paint::rendering;
