#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![allow(missing_docs)]
//! Raster (tiny-skia) painting backend for takumi: canvas, drawing, filters, and
//! the `render` entry point. Internal crate; depend on `takumi` instead.
//!
//! Re-exports the `takumi-core` root so the moved `rendering` code keeps resolving
//! `crate::layout`, `crate::resources`, `crate::Result`, etc.

pub use takumi_core::*;

pub mod rendering;
