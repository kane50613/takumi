#![deny(missing_docs)]
//! CSS parsing and computed-style layer for takumi.
//!
//! Holds the (cold) CSS parsing, cascade, and value types so they can be
//! compiled independently from the hot rendering paths in `takumi`. Selector
//! _matching_ against the node tree lives in `takumi` (`layout::matching`),
//! not here, keeping this crate free of any node/render dependency.

/// Parse and cascade error types.
pub mod error;
/// `@keyframes` rules and animation timing.
pub mod keyframes;
/// CSS value types, parsing, and the cascade.
pub mod style;
mod viewport;

// Public surface re-exported at the crate root (e.g. `takumi_css::Display`).
pub use style::*;
pub use viewport::*;
