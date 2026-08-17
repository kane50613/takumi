#![doc(
  html_logo_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg",
  html_favicon_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg"
)]
#![deny(missing_docs)]
//! Takumi renders a UI component tree to an image.
//!
//! This crate is the facade. The entry-point functions live at the crate root;
//! the curated, stable types live in [`prelude`]. Glob the prelude, build a node
//! tree, and call [`render`].
//!
//! # Example
//!
//! ```rust
//! use takumi::prelude::*;
//! use takumi::render;
//!
//! # fn main() -> takumi::prelude::Result<()> {
//! let node = Node::container([Node::text("Hello, world!").with_style(
//!   Style::default().with(StyleDeclaration::font_size(Length::Px(32.0).into())),
//! )]);
//!
//! // Reuse one font context across renders to share the decode cache.
//! let mut fonts = Fonts::default();
//! fonts.register(FontResource::new(include_bytes!(
//!   "../../assets/fonts/geist/Geist[wght].woff2"
//! )))?;
//!
//! let options = RenderOptions::builder()
//!   .viewport(Viewport::new((1200, 630)))
//!   .node(node)
//!   .fonts(&fonts)
//!   .build();
//!
//! let image = render(options)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Feature flags
//!
//! - `raster-backend` (default): raster rendering backend.
//! - `svg-source` (default): SVG image sources in the core and raster backend.
//! - `svg-backend`: vector SVG output backend (`render_svg`).
//! - `woff2`: WOFF2 font support.
//! - `woff`: WOFF font support.
//! - `image-decoding` (default): `jpeg`, `webp` and `gif` together.
//! - `jpeg`, `webp`, `gif`: one image source format each. PNG and ICO are always
//!   decoded.
//! - `rayon`: parallelism in the raster backend; needs `raster-backend`.
//! - `unstable`: re-export the backend crates with no semver guarantee.

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
    layout::node::{ImageData, ImageSourceInput, Node, NodeKind, RgbaImage, TextData},
    resources::{
      font::{FontError, FontOverride, FontResource, FontSource, GenericFamily, RegisteredFamily},
      image::{ImageCacheMode, ImageSource},
    },
    style::*,
    viewport::Viewport,
  };
  #[cfg(feature = "from-html")]
  pub use takumi_html::{DEFAULT_MAX_DEPTH, FromHtml, FromHtmlOptions, HtmlError, StylePresets};
  #[cfg(feature = "raster-backend")]
  pub use takumi_raster::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFormat, AnimationFrame,
    Bitmap, DitheringAlgorithm, MeasuredNode, MeasuredTextRun, OutputFormat, Quality,
    RenderOptions, SequentialScene,
  };
  #[cfg(feature = "svg-backend")]
  pub use takumi_svg::SvgOptions;
}

#[cfg(feature = "from-html")]
pub use takumi_html::from_html;
#[cfg(feature = "raster-backend")]
pub use takumi_raster::{
  measure, render, render_animation, write_animated_gif, write_animated_png, write_animated_webp,
  write_animation, write_image,
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
  #[cfg(feature = "from-html")]
  pub use takumi_html as html;
  #[cfg(feature = "raster-backend")]
  pub use takumi_raster as raster;
  #[cfg(feature = "svg-backend")]
  pub use takumi_svg as svg;
}
