#![doc(
  html_logo_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg",
  html_favicon_url = "https://raw.githubusercontent.com/kane50613/takumi/master/assets/images/takumi.svg"
)]
#![deny(missing_docs)]
//! Takumi lays out a node tree with CSS and renders it to an image, an animation, or SVG,
//! without a browser. Its text and inline layout port Chromium's Blink.
//!
//! This crate is the stable entry point. The render functions live at the crate root, and the
//! types to build a tree and configure a render live in [`prelude`]. PDF output and the paint
//! tree ship as the npm packages [`takumi-pdf`](https://www.npmjs.com/package/takumi-pdf) and
//! [`takumi-paint`](https://www.npmjs.com/package/takumi-paint).
//!
//! # Example
//!
//! Build a node tree, register a font, and render a 1200 × 630 PNG:
//!
//! ```rust
//! use takumi::prelude::*;
//! use takumi::{render, write_image};
//!
//! # fn main() -> takumi::prelude::Result<()> {
//! let node = Node::container([Node::text("Hello, world!").with_style(
//!   Style::default().with(StyleDeclaration::font_size(Length::Px(32.0).into())),
//! )]);
//!
//! // Takumi loads no system fonts. Register fonts once and reuse `fonts` across renders.
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
//!
//! let mut png = Vec::new();
//! write_image(&image, &mut png, OutputFormat::Png)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Entry points
//!
//! - [`render`] draws one frame. [`write_image`] encodes it as PNG, JPEG, WebP, or ICO.
//! - [`render_animation`] renders every frame of a scene timeline at a fixed frame rate.
//!   [`write_animation`] streams the frames into animated WebP, APNG, or GIF instead.
//! - [`measure`] lays out the tree and returns box sizes and text runs without drawing.
//! - `render_svg` writes vector SVG. It needs the `svg-backend` feature.
//! - `from_html` parses HTML into a node tree. It needs the `from-html` feature.
//!
//! # Feature flags
//!
//! - `raster-backend` (default) enables the raster rendering backend.
//! - `svg-source` (default) enables SVG image sources in the core and raster backend.
//! - `svg-backend` enables the vector SVG output backend (`render_svg`).
//! - `from-html` enables HTML parsing (`from_html`).
//! - `woff2` (default) enables WOFF2 font support.
//! - `woff` (default) enables WOFF font support.
//! - `image-decoding` (default) enables `png`, `jpeg`, `webp` and `gif` together.
//! - `png`, `jpeg`, `webp`, and `gif` each enable one image source format. A source whose
//!   decoder is off still lays out from its header size.
//! - `rayon` (default) adds parallelism in the raster backend. It needs `raster-backend`.
//! - `unstable` re-exports the backend crates with no semver guarantee.

/// The curated, stable data structures for building a node tree and configuring a
/// render.
///
/// A glob import (`use takumi::prelude::*;`) brings the types into scope; call the
/// entry-point functions (e.g. [`render`], [`write_image`]) from the crate root.
/// The glob pulls in common names like `Error`, `Result`, `Style`, and `Color`;
/// that breadth is intentional for a prelude.
pub mod prelude {
  #[cfg(feature = "from-html")]
  pub use takumi_core::layout::node::MAXIMUM_DOM_TREE_DEPTH as DEFAULT_MAX_DEPTH;
  pub use takumi_core::{
    Error, Fonts, Result,
    layout::node::{ImageData, ImageSourceInput, Node, NodeKind, RgbaImage, TextData},
    resources::{
      font::{FontError, FontOverride, FontResource, FontSource, GenericFamily, RegisteredFamily},
      image::{ImageCacheMode, ImageSource},
      unicode_range::{UnicodeRange, UnicodeRangeSet},
    },
    style::*,
    viewport::Viewport,
  };
  #[cfg(feature = "from-html")]
  pub use takumi_html::{FromHtml, FromHtmlOptions, HtmlError, StylePresets};
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
