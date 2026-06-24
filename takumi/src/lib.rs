#![doc(
  html_logo_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg",
  html_favicon_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg"
)]
#![deny(missing_docs)]
//! Takumi renders UI component trees to images. This crate is the facade users
//! depend on: entry-point **functions** live at the crate root and the _curated,
//! stable_ data structures live in [`crate::prelude`]. Glob the prelude for the types
//! and call the functions from the crate root.
//!
//! The backend crates expose a much larger surface than is meant for general use
//! — layout-engine glue, paint internals, and other cross-crate plumbing are
//! `pub` only because sibling crates need them. Those internals are deliberately
//! _not_ re-exported here. If you need them, enable the `unstable` feature.
//! nothing under that module is covered by semver.
//!
//! # Example
//!
//! ```rust
//! use takumi::prelude::*;
//! use takumi::render;
//!
//! let node = Node::container([Node::text("Hello, world!").with_style(
//!   Style::default().with(StyleDeclaration::font_size(Length::Px(32.0).into())),
//! )]);
//!
//! // Create a font context. Reuse it across renders to share the decode cache.
//! let mut fonts = Fonts::default();
//!
//! // Load fonts
//! fonts
//!   .register(FontResource::new(include_bytes!(
//!     "../../assets/fonts/geist/Geist[wght].woff2"
//!   )))
//!   .unwrap();
//!
//! let viewport = Viewport::new((1200, 630));
//!
//! let options = RenderOptions::builder()
//!   .viewport(viewport)
//!   .node(node)
//!   .fonts(&fonts)
//!   .build();
//!
//! let image = render(options).unwrap();
//! ```
//!
//! # Feature Flags
//!
//! - `raster-backend` (default): Enable the raster rendering backend.
//! - `svg` (default): Enable SVG image-source support in the core and raster
//!   backend.
//! - `svg-backend`: Enable the vector/SVG output backend ([`render_svg`]). Opt-in.
//! - `woff2`: Enable WOFF2 font support.
//! - `woff`: Enable WOFF font support.
//! - `rayon`: Enable rayon-based parallelism in the raster backend, when
//!   `raster-backend` is also enabled.
//! - `unstable`: Re-export the backend crates wholesale under [`unstable`]. No
//!   semver guarantee. Opt-in.

/// The curated, stable data structures for building a node tree and configuring a
/// render.
///
/// A glob import (`use takumi::prelude::*;`) brings the types into scope; call the
/// entry-point functions (e.g. [`render`], [`write_image`]) from the crate root.
/// The glob pulls in common names like `Error`, `Result`, `Style`, and `Color`;
/// that breadth is intentional for a prelude.
pub mod prelude {
  pub use takumi_core::{
    Error, Fonts, Result,
    layout::{
      Viewport,
      node::{ImageData, ImageSourceInput, Node, NodeKind, TextData},
      style::*,
    },
    resources::{
      font::{FontError, FontResource, RegisteredFamily},
      image::{ImageCacheMode, ImageSource},
    },
  };

  #[cfg(feature = "raster-backend")]
  pub use takumi_raster::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, Bitmap,
    DitheringAlgorithm, MeasuredNode, MeasuredTextRun, OutputFormat, Quality, RenderOptions,
    SequentialScene,
  };

  #[cfg(feature = "svg-backend")]
  pub use takumi_svg::SvgOptions;
}

#[cfg(feature = "raster-backend")]
pub use takumi_raster::{
  encode_animated_gif, encode_animated_png, encode_animated_webp, measure, render,
  render_animation, write_image,
};

#[cfg(feature = "svg-backend")]
pub use takumi_svg::render as render_svg;

/// Unstable, semver-exempt access to the backend crates in full.
///
/// Everything here is implementation surface that may change or disappear in any
/// release. Prefer the curated [`prelude`] and crate-root functions.
#[cfg(feature = "unstable")]
pub mod unstable {
  pub use takumi_core as base;

  #[cfg(feature = "raster-backend")]
  pub use takumi_raster as raster;

  #[cfg(feature = "svg-backend")]
  pub use takumi_svg as svg;
}
