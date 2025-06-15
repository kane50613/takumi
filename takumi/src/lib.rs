//! # takumi
//!
//! High-performance Rust library for generating images with CSS Flexbox-like layouts.
//!
//! _Takumi (匠) means "artisan" or "craftsman" in Japanese - reflecting the precision and artistry required to craft beautiful images through code._
//!
//! Checkout the [minimal example](https://github.com/kane50613/takumi/blob/master/example/src/minimal.rs) for a quick start.
//!
//! ## Credits
//!
//! - [taffy](https://github.com/DioxusLabs/taffy) for the layout system.
//! - [image](https://github.com/image-rs/image) for the image processing.
//! - [cosmic-text](https://github.com/kornelski/cosmic-text) for the text rendering.
//! - [woff2-patched](https://github.com/zimond/woff2-rs) for the font processing.
//! - [ts-rs](https://github.com/AlephAlpha/ts-rs) for the type-safe serialization.

#![deny(missing_docs)]

/// Module for handling border radius calculations and styling
pub mod border_radius;

/// Module for color management and conversion between different color formats
pub mod color;

/// Module for managing rendering context, including font systems and image storage
pub mod context;

/// Module for font loading and management, with support for WOFF2 fonts
pub mod font;

/// Module for node-based layout system and drawing primitives
pub mod node;

/// Module for image rendering and canvas management
pub mod render;

/// Module for box shadow operations
pub mod box_shadow;

pub use image;
pub use taffy;
