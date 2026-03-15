use std::{
  fs::File,
  io::Read,
  path::{Path, PathBuf},
};

use takumi::{GlobalContext, resources::font::FontError};

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
      .font_context_mut()
      .load_and_store(font_data.into(), None, None)
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
      .font_context_mut()
      .load_and_store(font_data.into(), None, None)
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
      .font_context_mut()
      .load_and_store(font_data.into(), None, None)
      .is_ok()
  );
}

#[test]
fn test_invalid_format_detection() {
  // Test with invalid data
  let invalid_data = vec![0x00, 0x01, 0x02, 0x03];
  let mut context = GlobalContext::default();

  let result = context
    .font_context_mut()
    .load_and_store(invalid_data.into(), None, None);
  assert!(matches!(result, Err(FontError::UnsupportedFormat)));
}

#[test]
fn test_empty_data() {
  // Test with empty data
  let empty_data = &[];
  let mut context = GlobalContext::default();

  let result = context
    .font_context_mut()
    .load_and_store(empty_data.into(), None, None);
  assert!(matches!(result, Err(FontError::UnsupportedFormat)));
}

#[test]
fn test_too_short_data() {
  // Test with data too short for format detection
  let short_data = &[0x00, 0x01, 0x00];
  let mut context = GlobalContext::default();

  let result = context
    .font_context_mut()
    .load_and_store(short_data.into(), None, None);
  assert!(matches!(result, Err(FontError::UnsupportedFormat)));
}
