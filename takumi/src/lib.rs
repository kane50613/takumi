#![doc(
  html_logo_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg",
  html_favicon_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg"
)]
#![deny(missing_docs)]
//! Takumi renders UI component trees to images. This crate is the facade users
//! depend on: it re-exports a *curated, stable* subset of the backend-agnostic
//! core ([`base`]) and the rendering backends ([`raster`], [`svg`]).
//!
//! The backend crates expose a much larger surface than is meant for general use
//! — layout-engine glue, paint internals, and other cross-crate plumbing are
//! `pub` only because sibling crates need them. Those internals are deliberately
//! *not* re-exported here. If you need them, enable the `unstable` feature and
//! reach them through [`unstable`]; nothing under that module is covered by
//! semver.
//!
//! # Example
//!
//! ```rust
//! use takumi::base::{
//!   Fonts,
//!   layout::{Viewport, node::Node, style::{Length::Px, Style, StyleDeclaration}},
//!   resources::font::FontResource,
//! };
//! use takumi::raster::{RenderOptions, render};
//!
//! let node = Node::container([Node::text("Hello, world!").with_style(
//!   Style::default().with(StyleDeclaration::font_size(Px(32.0).into())),
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
//! - `unstable`: Re-export the backend crates wholesale under [`unstable`]. No
//!   semver guarantee. Opt-in.

pub mod base {
  //! Backend-agnostic core: the node tree, styling, fonts, and viewport.
  //!
  //! Curated stable surface of [`takumi_base`]. The full crate (layout-engine
  //! internals, render context, inline machinery, …) is available under
  //! [`crate::unstable::base`] with the `unstable` feature.
  pub use takumi_base::{Error, Fonts, Result};

  /// Styling, geometry, and the node tree.
  pub mod layout {
    pub use takumi_base::layout::Viewport;

    /// The renderable node tree.
    pub mod node {
      pub use takumi_base::layout::node::{ImageData, ImageSourceInput, Node, NodeKind, TextData};
    }

    /// CSS-like styling: colors, lengths, and declarations.
    pub mod style {
      pub use takumi_base::layout::style::*;
    }
  }

  /// Render-time resources: fonts and images.
  pub mod resources {
    /// Font registration.
    pub mod font {
      pub use takumi_base::resources::font::{FontError, FontResource, RegisteredFamily};
    }

    /// Pre-loaded image sources.
    pub mod image {
      pub use takumi_base::resources::image::{ImageCacheMode, ImageSource};
    }
  }
}

#[cfg(feature = "raster")]
pub mod raster {
  //! Raster (bitmap) rendering backend.
  //!
  //! Curated stable surface of [`takumi_raster`]. The full crate is available
  //! under [`crate::unstable::raster`] with the `unstable` feature.
  pub use takumi_raster::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame,
    DitheringAlgorithm, ImageOutputFormat, MeasuredNode, MeasuredTextRun, Quality, RenderOptions,
    SequentialScene, encode_animated_gif, encode_animated_png, encode_animated_webp,
    measure_layout, render, render_sequence_animation, write_image,
  };
}

#[cfg(feature = "svg-backend")]
pub mod svg {
  //! Vector (SVG) rendering backend.
  //!
  //! Curated stable surface of [`takumi_svg`]. The full crate is available under
  //! [`crate::unstable::svg`] with the `unstable` feature.
  pub use takumi_svg::{SvgOptions, render};
}

/// Unstable, semver-exempt access to the backend crates in full.
///
/// Everything here is implementation surface that may change or disappear in any
/// release. Prefer the curated [`base`]/[`raster`]/[`svg`] modules.
#[cfg(feature = "unstable")]
pub mod unstable {
  pub use takumi_base as base;

  #[cfg(feature = "raster")]
  pub use takumi_raster as raster;

  #[cfg(feature = "svg-backend")]
  pub use takumi_svg as svg;
}
