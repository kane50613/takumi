//! Fonts, fixtures, and page values the unit tests share.

use std::{fs, path::Path};

use takumi_core::{Fonts, resources::font::FontResource, style::Direction};

use crate::page_kinds::PerPage;

pub(crate) fn fonts() -> Fonts {
  let mut fonts = Fonts::default();

  for path in [
    "../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
    "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
  ] {
    let data = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).expect("read test font");

    fonts
      .register(FontResource::new(data))
      .expect("load test font");
  }
  fonts
}

pub(crate) fn fixture(name: &str) -> String {
  fs::read_to_string(
    Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("fixtures")
      .join(name),
  )
  .expect("read html fixture")
}

/// `value` for every page.
pub(crate) fn uniform_pages<T: Clone>(value: T) -> PerPage<T> {
  PerPage::new(Direction::Ltr, value.clone(), value.clone(), value)
}
