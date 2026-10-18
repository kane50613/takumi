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

mod test_utils;

use std::{env, path::Path};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use takumi::prelude::RenderOptions;
use test_utils::{CONTEXT, HtmlFixture, TEST_IMAGES, html_fixture_paths, write_goldens};

#[test]
fn html_fixtures() {
  let files = html_fixture_paths(env::var("FIXTURE").ok().as_deref());
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
  let fixture = HtmlFixture::read(path)?;

  write_goldens(
    RenderOptions::builder()
      .viewport(fixture.viewport)
      .node(fixture.node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
      .stylesheet(fixture.stylesheet.into())
      .build(),
    name,
  )
}
