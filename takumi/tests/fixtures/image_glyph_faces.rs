use std::fs;

use takumi::prelude::*;
use takumi_core::resources::font::ImageFontFace;

use crate::test_utils::{CONTEXT, create_test_viewport, repo_base_path, write_goldens};

/// The Twemoji SVG at `name` in the test assets.
fn twemoji(name: &str) -> ImageSource {
  let path = repo_base_path(&format!("assets/images/twemoji/{name}.svg"));

  ImageSource::from_bytes(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn image_glyph_faces() {
  let node = Node::from_html(
    r#"<div style="font-size: 48px; padding: 24px">
      <p style="margin: 0">Hi 👋 and 😆🍔, then 🚀!</p>
      <p style="margin: 0; text-decoration: underline; letter-spacing: 12px">👋🍔 text 😆</p>
      <p style="margin: 0; font-size: 24px; line-height: 1">Small 🍔 and 👋</p>
    </div>"#,
    FromHtmlOptions::default(),
  )
  .unwrap();
  let mut fonts = CONTEXT.clone();

  fonts.register_image_face(
    ImageFontFace::new("Twemoji Images")
      .glyph("👋", twemoji("waving-hand"))
      .glyph("😆", twemoji("grinning-squinting-face"))
      .glyph("🍔", twemoji("hamburger")),
  );

  write_goldens(
    RenderOptions::builder()
      .viewport(create_test_viewport())
      .node(node)
      .fonts(&fonts)
      .build(),
    "image_glyph_faces",
  )
  .unwrap();
}
