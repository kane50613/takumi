//! Node.js N-API bindings for Takumi.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]

mod load_font_task;
mod measure_task;
mod pool;
mod render_animation_task;
mod render_task;
pub(crate) mod renderer;
mod svg_render_task;

use std::fmt::Display;

use napi::{De, Env, Error, bindgen_prelude::*};
use napi_derive::napi;
pub use renderer::Renderer;
use serde::{
  Deserialize, Deserializer,
  de::{DeserializeOwned, Error as DeError},
};
use takumi_bindings_common::build_font_resource;
use takumi_core::{
  resources::{font::FontResource, glyph_cache},
  style::{FontStyle, FromCssStr},
};

/// Sets the byte budget shared by the resolved-glyph and glyph-mask caches;
/// `0` stops caching. Defaults to 8 MiB.
///
/// These caches live in the module, not in a `Renderer`, so this budget covers
/// every renderer in the process. The value is read when a cache is first used,
/// so call this before the first render.
///
/// Raise it for scripts with large glyph sets: a CJK outline runs a few
/// kilobytes, so the default holds around a thousand of them and a page of
/// Chinese re-rasterizes glyphs it just evicted.
#[napi(js_name = "setGlyphCacheMaxBytes")]
pub fn set_glyph_cache_max_bytes(bytes: f64) {
  glyph_cache::set_glyph_cache_max_bytes(bytes.max(0.0) as usize);
}

/// A font family produced by `registerFont`, with the faces it contains.
#[napi(object)]
pub struct RegisteredFamily {
  /// Family name as stored by the font system (normalized; reflects any override).
  pub name: String,
  /// Faces registered under this family.
  pub faces: Vec<RegisteredFace>,
}

/// A single face within a `RegisteredFamily`.
#[napi(object)]
pub struct RegisteredFace {
  /// Weight class, typically `1`–`1000`.
  pub weight: f64,
  /// CSS `font-style` value (`normal`, `italic`, or `oblique [<angle>deg]`).
  pub style: String,
  /// Width as a percentage of normal (e.g. `100`).
  pub width: f64,
  /// Index of the face within its source collection.
  pub index: u32,
}

impl From<takumi_core::resources::font::RegisteredFamily> for RegisteredFamily {
  fn from(family: takumi_core::resources::font::RegisteredFamily) -> Self {
    Self {
      name: family.name,
      faces: family.faces.into_iter().map(Into::into).collect(),
    }
  }
}

impl From<takumi_core::resources::font::RegisteredFace> for RegisteredFace {
  fn from(face: takumi_core::resources::font::RegisteredFace) -> Self {
    Self {
      weight: face.weight as f64,
      style: face.style,
      width: face.width as f64,
      index: face.index,
    }
  }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FontInput {
  pub name: Option<String>,
  pub weight: Option<f64>,
  pub style: Option<FontStyleInput>,
  /// Logical family this font is a coverage subset of; expands at render time.
  pub subset_of: Option<String>,
  /// Where this subset sits in its group's fallback order; lowest is tried first.
  pub subset_rank: Option<u32>,
  /// CSS generic family keyword (e.g. `monospace`) this font resolves for.
  pub generic: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) struct FontStyleInput(pub FontStyle);

impl<'de> Deserialize<'de> for FontStyleInput {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    Ok(FontStyleInput(
      FontStyle::from_css_str(&s).map_err(D::Error::custom)?,
    ))
  }
}

/// Ref-counted view of JS-owned bytes, sendable into async tasks without copying.
/// Callers must not mutate the bytes on the JS side while a task reads them —
/// the same aliasing contract Buffer inputs have always had.
pub(crate) enum JsBytes {
  Buffer(Buffer),
  Array(Uint8Array),
}

impl AsRef<[u8]> for JsBytes {
  fn as_ref(&self) -> &[u8] {
    match self {
      JsBytes::Buffer(buffer) => buffer,
      JsBytes::Array(array) => array,
    }
  }
}

impl JsBytes {
  pub(crate) fn from_object(env: Env, value: Object) -> Result<Self> {
    if value.is_buffer()? {
      let buffer = unsafe { BufferSlice::from_napi_value(env.raw(), value.raw()) }?;
      return Ok(JsBytes::Buffer(buffer.into_buffer(&env)?));
    }

    if value.is_typedarray()? {
      let array = unsafe { Uint8Array::from_napi_value(env.raw(), value.raw()) }?;
      return Ok(JsBytes::Array(array));
    }

    if value.is_arraybuffer()? {
      let buffer = unsafe { ArrayBuffer::from_napi_value(env.raw(), value.raw()) }?;
      return Ok(JsBytes::Buffer(Buffer::from(buffer.to_vec())));
    }

    Err(Error::from_reason(
      "Expected Buffer, ArrayBuffer, or Uint8Array".to_owned(),
    ))
  }
}

pub(crate) fn parse_font_input(env: Env, font: Object) -> Result<(FontInput, JsBytes)> {
  if let Ok(buffer) = JsBytes::from_object(env, font) {
    Ok((FontInput::default(), buffer))
  } else {
    let buffer = font
      .get_named_property("data")
      .and_then(|buffer| JsBytes::from_object(env, buffer))?;
    let font: FontInput = deserialize_with_tracing(font).map_err(map_error)?;

    Ok((font, buffer))
  }
}

pub(crate) fn resolve_font_resource<'a>(
  font: &'a FontInput,
  buffer: &'a [u8],
) -> Result<FontResource<'a>> {
  build_font_resource(
    buffer,
    font.name.clone(),
    font.weight.map(|weight| weight as f32),
    font.style.map(|style| style.0),
    font.subset_of.clone(),
    font.subset_rank,
    font.generic.clone(),
  )
  .map_err(map_error)?
  .into_resolved()
  .map_err(map_error)
}

pub(crate) fn deserialize_with_tracing<T: DeserializeOwned>(value: Object) -> Result<T> {
  let mut de = De::new(&value);
  T::deserialize(&mut de).map_err(|e| Error::from_reason(e.to_string()))
}

pub(crate) fn map_error<E: Display>(err: E) -> napi::Error {
  napi::Error::from_reason(err.to_string())
}

/// Writes to the host's `console.warn`, staying silent if the call fails.
pub(crate) fn console_warn(env: Env, message: &str) {
  let Ok(global) = env.get_global() else {
    return;
  };
  let Ok(console) = global.get_named_property::<Object>("console") else {
    return;
  };
  let Ok(warn) = console.get_named_property::<Function<String, Unknown>>("warn") else {
    return;
  };

  let _ = warn.call(message.to_owned());
}
