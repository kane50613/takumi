//! Fixtures that cannot be expressed as an HTML source file: they animate,
//! assert on the rendered pixels or measured layout, or use node features
//! `to_html` does not carry (intrinsic image size hints, inline SVG sources).
//! Everything else lives in `tests/fixtures-html/` and runs through the
//! `html_fixtures` test.

mod test_utils;

#[path = "fixtures/animation.rs"]
pub mod animation;
#[path = "fixtures/deep_nesting.rs"]
pub mod deep_nesting;
#[path = "fixtures/inline.rs"]
pub mod inline;
#[path = "fixtures/paint_bounds_text_ink.rs"]
pub mod paint_bounds_text_ink;
#[path = "fixtures/style_background_image.rs"]
pub mod style_background_image;
#[path = "fixtures/style_opacity.rs"]
pub mod style_opacity;
#[path = "fixtures/svg.rs"]
pub mod svg;
#[path = "fixtures/text.rs"]
pub mod text;
