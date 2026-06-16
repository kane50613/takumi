use std::{
  assert_matches,
  fs::File,
  io::Read,
  path::{Path, PathBuf},
};

use takumi::core::{
  GlobalContext,
  resources::font::{FontError, FontResource},
};

fn font_path(path: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../assets/fonts/")
    .join(path)
    .to_path_buf()
}

#[test]
fn test_ttf_font_loading() {
  let mut context = GlobalContext::default();

  let mut font_data = Vec::new();
  File::open(font_path("noto-sans/NotoColorEmoji.ttf"))
    .unwrap()
    .read_to_end(&mut font_data)
    .unwrap();

  assert!(
    context
      .font_context
      .load_and_store(FontResource::new(font_data))
      .is_ok()
  );
}

#[test]
fn test_ttc_font_loading() {
  let mut context = GlobalContext::default();

  let mut font_data = Vec::new();
  File::open(font_path("ubuntu/Ubuntu.ttc"))
    .unwrap()
    .read_to_end(&mut font_data)
    .unwrap();

  assert!(
    context
      .font_context
      .load_and_store(FontResource::new(font_data))
      .is_ok()
  );
}

#[test]
fn test_woff2_font_loading() {
  let mut context = GlobalContext::default();

  let mut font_data = Vec::new();
  File::open(font_path("geist/Geist[wght].woff2"))
    .unwrap()
    .read_to_end(&mut font_data)
    .unwrap();

  assert!(
    context
      .font_context
      .load_and_store(FontResource::new(font_data))
      .is_ok()
  );
}

#[test]
fn test_invalid_format_detection() {
  // Test with invalid data
  let invalid_data = vec![0x00, 0x01, 0x02, 0x03];
  let mut context = GlobalContext::default();

  let result = context
    .font_context
    .load_and_store(FontResource::new(invalid_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}

#[test]
fn test_empty_data() {
  // Test with empty data
  let empty_data = Vec::<u8>::new();
  let mut context = GlobalContext::default();

  let result = context
    .font_context
    .load_and_store(FontResource::new(empty_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}

#[test]
fn test_too_short_data() {
  // Test with data too short for format detection
  let short_data = vec![0x00, 0x01, 0x00];
  let mut context = GlobalContext::default();

  let result = context
    .font_context
    .load_and_store(FontResource::new(short_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}
