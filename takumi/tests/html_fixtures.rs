//! Renders every HTML file in `tests/fixtures-html/` into its raster and
//! vector goldens. The HTML is the fixture source: add a file, run this test,
//! eyeball the generated goldens. The same file opens in a browser for a
//! reference render (it links `shared.css` for the font faces).

mod test_utils;

use std::{
  fs::{self, File},
  path::{Path, PathBuf},
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use takumi::{
  prelude::{
    FromHtml, FromHtmlOptions, Node, OutputFormat, RenderOptions, StylePresets, StyleSheet,
    Viewport,
  },
  render, write_image,
};
use takumi_svg::{SvgOptions, render as svg_render};
use test_utils::{CONTEXT, TEST_IMAGES};

#[test]
fn html_fixtures() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures-html");
  let mut files: Vec<PathBuf> = fs::read_dir(&dir)
    .expect("tests/fixtures-html directory")
    .filter_map(|entry| {
      let path = entry.ok()?.path();

      (path.extension()? == "html").then_some(path)
    })
    .collect();

  files.sort();
  assert!(!files.is_empty(), "no HTML fixtures found");

  let failures: Vec<String> = files
    .par_iter()
    .filter_map(|path| {
      let name = path.file_stem().unwrap().to_string_lossy();

      render_fixture(path, &name)
        .err()
        .map(|error| format!("{name}: {error}"))
    })
    .collect();

  assert!(failures.is_empty(), "{failures:#?}");
}

fn render_fixture(path: &Path, name: &str) -> Result<(), String> {
  let html = fs::read_to_string(path).map_err(|error| error.to_string())?;
  let body = section(&html, "<body", "</body>")
    .and_then(|tag_onward| tag_onward.split_once('>'))
    .map(|(_, inner)| inner.trim())
    .ok_or("no <body> element")?;
  let viewport = body_viewport(&html)?;
  let css = section(&html, "<style>", "</style>").unwrap_or("").trim();

  let options = FromHtmlOptions::builder()
    .presets(StylePresets::empty())
    .build();
  let node = Node::from_html(body, options).map_err(|error| format!("parse: {error:?}"))?;
  let stylesheet: std::sync::Arc<StyleSheet> = StyleSheet::parse(css)
    .map_err(|error| format!("stylesheet: {error:?}"))?
    .into();

  let build_options = || {
    RenderOptions::builder()
      .viewport(viewport)
      .node(node.clone())
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
      .stylesheet(stylesheet.clone())
      .build()
  };

  let generated = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures-generated");

  // Vector golden is best-effort, matching the Rust-fixture harness: the SVG
  // backend does not cover every paint feature yet.
  if let Ok(svg) = svg_render(
    SvgOptions::builder()
      .node(node.clone())
      .viewport(viewport)
      .fonts(&CONTEXT)
      .stylesheet(stylesheet.clone())
      .images(TEST_IMAGES.clone())
      .build(),
  ) {
    fs::write(generated.join(format!("{name}.svg")), svg).map_err(|error| error.to_string())?;
  }

  let image = render(build_options()).map_err(|error| format!("render: {error:?}"))?;
  let mut file =
    File::create(generated.join(format!("{name}.webp"))).map_err(|error| error.to_string())?;

  write_image(&image, &mut file, OutputFormat::WebPLossless).map_err(|error| format!("{error:?}"))
}

fn body_viewport(html: &str) -> Result<Viewport, String> {
  let style = section(html, "<body style=\"", "\"").ok_or("no styled <body>")?;
  let pixels = |property: &str| {
    style
      .split(';')
      .filter_map(|declaration| declaration.split_once(':'))
      .find(|(name, _)| name.trim() == property)
      .ok_or(format!("no body {property}"))
      .and_then(|(_, value)| {
        value
          .trim()
          .trim_end_matches("px")
          .parse::<u32>()
          .map_err(|error| error.to_string())
      })
  };

  Ok(Viewport::new((pixels("width")?, pixels("height")?)))
}

fn section<'h>(html: &'h str, start: &str, end: &str) -> Option<&'h str> {
  let after = &html[html.find(start)? + start.len()..];

  Some(&after[..after.find(end)?])
}
