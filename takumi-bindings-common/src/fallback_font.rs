//! Geist, the last-resort font every renderer starts with. The napi bindings build it in. The
//! wasm bindings import it from `@takumi-rs/helpers/fallback-font`, which the
//! `takumi-no-fallback-font` export condition empties, so a bundle can leave it out.

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
use std::sync::OnceLock;

use takumi_core::{
  Fonts,
  resources::font::{FontError, FontOverride, FontResource, FontSource},
};
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

/// The family the fallback font registers as.
const FAMILY: &str = "Geist";

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen(raw_module = "@takumi-rs/helpers/fallback-font")]
extern "C" {
  #[wasm_bindgen(thread_local_v2, js_name = fallbackFont)]
  static FALLBACK_FONT: JsValue;
}

/// The fallback font's bytes, read from JavaScript once.
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
fn bytes() -> Option<&'static [u8]> {
  static BYTES: OnceLock<Option<Box<[u8]>>> = OnceLock::new();

  BYTES
    .get_or_init(|| {
      FALLBACK_FONT.with(|font| {
        font
          .dyn_ref::<js_sys::Uint8Array>()
          .map(|font| font.to_vec().into_boxed_slice())
      })
    })
    .as_deref()
}

/// The fallback font's bytes, when built in.
#[cfg(not(all(feature = "wasm", target_arch = "wasm32")))]
fn bytes() -> Option<&'static [u8]> {
  #[cfg(feature = "embedded-fallback-font")]
  return Some(include_bytes!(
    "../../assets/fonts/geist/geist-latin-wght-300-800.ttf"
  ));

  #[cfg(not(feature = "embedded-fallback-font"))]
  None
}

/// Registers the fallback font, claiming no generic family, when `wanted` accepts its family.
pub(crate) fn register(fonts: &mut Fonts, wanted: impl Fn(&str) -> bool) -> Result<(), FontError> {
  let Some(bytes) = bytes().filter(|_| wanted(FAMILY)) else {
    return Ok(());
  };

  let resource = FontResource::new(FontSource::from_static(bytes))
    .override_info(FontOverride {
      family_name: Some(FAMILY.into()),
      ..Default::default()
    })
    .last_resort();

  drop(fonts.register(resource)?);

  Ok(())
}
