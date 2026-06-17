#![doc(
  html_logo_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg",
  html_favicon_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg"
)]
#![allow(missing_docs)]
//! Takumi renders UI component trees to images. This crate is a thin facade that
//! re-exports the backend-agnostic core ([`takumi_base`], as [`base`]) and the
//! rendering backends under namespaced modules: the raster backend
//! ([`takumi_raster`], as [`raster`]) and the vector/SVG backend ([`takumi_svg`],
//! as [`svg`]).
//!
//! # Example
//!
//! ```rust
//! use takumi::base::{
//!   GlobalContext,
//!   layout::{Viewport, node::Node, style::{Length::Px, Style, StyleDeclaration}},
//!   resources::font::FontResource,
//! };
//! use takumi::raster::{RenderOptions, render};
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
//! - `raster` (default): Enable the raster rendering backend, available as
//!   [`takumi::raster`](raster).
//! - `svg` (default): Enable SVG image-source support in the core and raster
//!   backend.
//! - `svg-backend`: Enable the vector/SVG output backend, available as
//!   [`takumi::svg`](svg). Opt-in.
//! - `woff2`: Enable WOFF2 font support.
//! - `woff`: Enable WOFF font support.
//! - `rayon`: Enable rayon-based parallelism in the raster backend (implies
//!   `raster`).

pub use takumi_base as base;

#[cfg(feature = "raster")]
pub use takumi_raster as raster;

#[cfg(feature = "svg-backend")]
pub use takumi_svg as svg;
