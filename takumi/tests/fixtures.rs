//! Fixtures that cannot be expressed as an HTML source file: they animate or
//! assert on the rendered pixels or measured layout. Everything else lives in
//! `tests/fixtures-html/` and runs through the `html_fixtures` test.

#[path = "fixtures/animated_image_sources.rs"]
pub mod animated_image_sources;
#[path = "fixtures/animation.rs"]
pub mod animation;
#[path = "fixtures/deep_nesting.rs"]
pub mod deep_nesting;
#[path = "fixtures/html_byte_order_mark.rs"]
pub mod html_byte_order_mark;
#[path = "fixtures/html_parser_recovery.rs"]
pub mod html_parser_recovery;
#[path = "fixtures/jpeg_output.rs"]
pub mod jpeg_output;
#[path = "fixtures/paint_bounds_ink_reach.rs"]
pub mod paint_bounds_ink_reach;
#[path = "fixtures/paint_bounds_text_ink.rs"]
pub mod paint_bounds_text_ink;
#[path = "fixtures/png_output.rs"]
pub mod png_output;
#[path = "fixtures/png_palette_output.rs"]
pub mod png_palette_output;
#[path = "fixtures/style_background_image.rs"]
pub mod style_background_image;
#[path = "fixtures/style_opacity.rs"]
pub mod style_opacity;
#[path = "fixtures/text.rs"]
pub mod text;
#[cfg(not(target_arch = "wasm32"))]
#[path = "fixtures/webp_lossy_output.rs"]
pub mod webp_lossy_output;
