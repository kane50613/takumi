#![deny(missing_docs)]
//! CSS parsing and computed-style layer for takumi.
//!
//! Holds the (cold) CSS parsing, cascade, value types, and selector matching so
//! they can be compiled independently from the hot rendering paths in `takumi`.
//! Matching is generic over a [`matching::MatchableNode`] the caller implements,
//! keeping this crate free of any node/render dependency and the `selectors`
//! crate out of takumi's public API.

/// Parse and cascade error types.
pub mod error;
/// `@keyframes` rules and animation timing.
pub mod keyframes;
/// Selector matching against an abstract node tree.
pub mod matching;
/// CSS value types, parsing, and the cascade.
pub mod style;
mod viewport;

// Public surface re-exported at the crate root (e.g. `takumi_css::Display`).
pub use style::*;
pub use viewport::*;
