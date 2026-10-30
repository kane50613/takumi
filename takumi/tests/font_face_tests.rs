use std::{collections::BTreeSet, fs};

use takumi::{needs, prelude::*};

use crate::test_utils::{CONTEXT, repo_base_path};

const MANROPE: &str = "assets/fonts/manrope/manrope-latin-wght-normal.woff2";
const CODE: &str = "assets/fonts/google-sans-code/GoogleSansCode-VariableFont_wght.woff2";

/// The `@font-face` files a render of `text` in `family` reaches under `css`, with `fonts`.
fn reached(fonts: &Fonts, css: &str, family: &str, text: &str) -> BTreeSet<String> {
  let node = Node::text(text.to_string()).with_style(Style::default().with(
    StyleDeclaration::font_family(FontFamily::from_css_str(family).unwrap()),
  ));

  needs(
    RenderOptions::builder()
      .viewport(Viewport::new((400, 100)))
      .node(node)
      .fonts(fonts)
      .stylesheet(StyleSheet::parse(css).unwrap().into())
      .build(),
  )
  .fonts
}

fn urls<const N: usize>(urls: [&str; N]) -> BTreeSet<String> {
  urls.into_iter().map(str::to_owned).collect()
}

#[test]
fn needs_reports_the_files_the_text_reaches() {
  let css = format!(
    "@font-face {{ font-family: Face; src: url({MANROPE}); }}
     @font-face {{ font-family: Face; src: url({CODE}); unicode-range: U+30-39; }}
     @font-face {{ font-family: Unused; src: url(unused.woff2); }}"
  );

  assert_eq!(reached(&CONTEXT, &css, "Face", "abc"), urls([MANROPE]));
  assert_eq!(reached(&CONTEXT, &css, "Face", "a1"), urls([MANROPE, CODE]));
  assert_eq!(reached(&CONTEXT, &css, "Geist", "abc"), urls([]));
}

#[test]
fn a_failed_source_moves_the_rule_on_to_its_next() {
  let css = format!("@font-face {{ font-family: Face; src: url(missing.woff2), url({MANROPE}); }}");
  let mut fonts = CONTEXT.clone();

  assert_eq!(reached(&fonts, &css, "Face", "a"), urls(["missing.woff2"]));

  fonts.fail_font_file("missing.woff2");
  assert_eq!(reached(&fonts, &css, "Face", "a"), urls([MANROPE]));

  fonts
    .load_font_file(
      MANROPE,
      FontSource::from(fs::read(repo_base_path(MANROPE)).unwrap()),
    )
    .unwrap();
  assert_eq!(reached(&fonts, &css, "Face", "a"), urls([]));
}

#[test]
fn a_rule_outside_its_media_query_adds_no_face() {
  let css =
    "@media (max-width: 100px) { @font-face { font-family: Face; src: url(narrow.woff2); } }";

  assert_eq!(reached(&CONTEXT, css, "Face", "a"), urls([]));
}

#[test]
fn a_file_that_is_not_a_font_fails_to_load() {
  let css = "@font-face { font-family: Face; src: url(bad.woff2); }";
  let mut fonts = CONTEXT.clone();

  assert!(
    fonts
      .load_font_file("bad.woff2", FontSource::from(b"not a font".to_vec()))
      .is_err()
  );
  assert_eq!(reached(&fonts, css, "Face", "a"), urls(["bad.woff2"]));
}
