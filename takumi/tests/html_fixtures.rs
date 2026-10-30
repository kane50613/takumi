//! Renders every HTML file in `tests/fixtures-html/` into its raster and
//! vector goldens. The HTML is the fixture source: add a file, run this test,
//! eyeball the generated goldens. The same file opens in a browser for a
//! reference render (it links `shared.css` for the font faces).
//! `FIXTURE=text_sup,inline` renders only the fixtures whose names contain one
//! of those comma-separated fragments.
//!
//! `oxfmt` formats the fixtures under `htmlWhitespaceSensitivity: strict`
//! (see `.oxfmtrc.json`), so it never adds or removes whitespace between
//! elements. Whitespace that is itself test content (preserve spans,
//! tab-size) is written as `&#9;`/`&#10;`/`&#32;` entities, which the
//! formatter cannot re-wrap.

use std::{
  collections::BTreeSet,
  env, fs,
  path::{Path, PathBuf},
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use takumi::{
  needs,
  prelude::{
    FontSource, Fonts, FromHtml, FromHtmlOptions, Node, RenderOptions, StylePresets, StyleSheet,
    Viewport,
  },
};

use crate::test_utils::{CONTEXT, TEST_IMAGES, matches_fragments, repo_base_path, write_goldens};

#[test]
fn html_fixtures() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures-html");
  let only = env::var("FIXTURE").ok();
  let mut files: Vec<PathBuf> = fs::read_dir(&dir)
    .expect("tests/fixtures-html directory")
    .filter_map(|entry| {
      let path = entry.ok()?.path();

      (path.extension()? == "html" && is_selected(&path, only.as_deref())).then_some(path)
    })
    .collect();

  files.sort();
  assert!(!files.is_empty(), "no HTML fixtures match {only:?}");

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

/// Whether `only`, the `FIXTURE` fragments, picks the fixture at `path`.
fn is_selected(path: &Path, only: Option<&str>) -> bool {
  matches_fragments(
    only,
    &path.file_stem().unwrap_or_default().to_string_lossy(),
  )
}

fn render_fixture(path: &Path, name: &str) -> Result<(), String> {
  let html = fs::read_to_string(path).map_err(|error| error.to_string())?;
  let body = section(&html, "<body", "</body>")
    .and_then(|tag_onward| tag_onward.split_once('>'))
    .map(|(_, inner)| inner.trim())
    .ok_or("no <body> element")?;
  let viewport = body_viewport(&html)?;
  let css = section(&html, "<style>", "</style>").unwrap_or("").trim();

  // A fixture opts into the element presets with `<meta name="takumi-presets" content="chromium">`.
  let presets = if html.contains(r#"<meta name="takumi-presets" content="chromium" />"#) {
    StylePresets::chromium()
  } else {
    StylePresets::empty()
  };
  let options = FromHtmlOptions::builder().presets(presets).build();
  let node = Node::from_html(body, options).map_err(|error| format!("parse: {error:?}"))?;
  let stylesheet = StyleSheet::parse(css).map_err(|error| format!("stylesheet: {error:?}"))?;
  let mut fonts = CONTEXT.clone();

  load_font_files(&mut fonts, |fonts| {
    needs(fixture_options(fonts, viewport, &node, &stylesheet)).font_files
  });

  write_goldens(fixture_options(&fonts, viewport, &node, &stylesheet), name)
}

fn fixture_options<'f>(
  fonts: &'f Fonts,
  viewport: Viewport,
  node: &Node,
  stylesheet: &StyleSheet,
) -> RenderOptions<'f> {
  RenderOptions::builder()
    .viewport(viewport)
    .node(node.clone())
    .fonts(fonts)
    .images(TEST_IMAGES.clone())
    .stylesheet(stylesheet.clone().into())
    .build()
}

/// Loads each `@font-face` file the render reaches from the repository root, as the fixtures'
/// `<base href>` resolves it, until none is left. A missing file fails, which moves its rule on
/// to its next `src`.
fn load_font_files(fonts: &mut Fonts, mut reached: impl FnMut(&Fonts) -> BTreeSet<String>) {
  loop {
    let urls = reached(fonts);

    if urls.is_empty() {
      return;
    }

    for url in urls {
      match fs::read(repo_base_path(&url)) {
        Ok(bytes) => fonts
          .load_font_file(url.as_str(), FontSource::from(bytes))
          .unwrap_or_else(|_| fonts.fail_font_file(url.as_str())),
        Err(_) => fonts.fail_font_file(url.as_str()),
      }
    }
  }
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
