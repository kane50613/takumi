//! Node.js N-API bindings for Takumi.

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![deny(missing_docs)]

mod encode_frames_task;
mod load_font_task;
mod measure_task;
mod pool;
mod render_animation_task;
mod render_task;
pub(crate) mod renderer;

use std::{fmt::Display, ops::Deref};

use napi::{De, Env, Error, bindgen_prelude::*};
use napi_derive::napi;
use parley::{FontStyle, FontWeight, fontique::FontInfoOverride};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use takumi_base::{
  layout::style::{KeyframesRule, StyleSheet},
  resources::font::FontResource,
};

pub use renderer::Renderer;

/// Options for `Renderer.configureImageCache`.
#[napi(object)]
pub struct ImageCacheOptions {
  /// Maximum bytes of decoded images to keep in this renderer's cache. `0` disables (and clears) it.
  pub max_bytes: Option<f64>,
  /// Maximum number of decoded images to keep in this renderer's cache. `0` disables (and clears) it.
  pub max_size: Option<f64>,
}

/// A font family produced by `registerFonts`, with the faces it contains.
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

impl From<takumi_base::resources::font::RegisteredFamily> for RegisteredFamily {
  fn from(family: takumi_base::resources::font::RegisteredFamily) -> Self {
    Self {
      name: family.name,
      faces: family.faces.into_iter().map(Into::into).collect(),
    }
  }
}

impl From<takumi_base::resources::font::RegisteredFace> for RegisteredFace {
  fn from(face: takumi_base::resources::font::RegisteredFace) -> Self {
    Self {
      weight: face.weight as f64,
      style: face.style,
      width: face.width as f64,
      index: face.index,
    }
  }
}

#[derive(Deserialize, Default)]
pub(crate) struct FontInput {
  pub name: Option<String>,
  pub weight: Option<f64>,
  pub style: Option<FontStyleInput>,
}

#[derive(Clone, Copy)]
pub(crate) struct FontStyleInput(pub FontStyle);

impl<'de> Deserialize<'de> for FontStyleInput {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    Ok(FontStyleInput(FontStyle::parse_css(&s).unwrap_or_default()))
  }
}

pub(crate) fn buffer_from_object(env: Env, value: Object) -> Result<Buffer> {
  if value.is_buffer()? {
    let buffer = unsafe { BufferSlice::from_napi_value(env.raw(), value.raw()) }?;
    return buffer.into_buffer(&env);
  }

  let bytes = buffer_slice_from_object(env, value)?;
  Ok(Buffer::from(bytes.as_ref().to_vec()))
}

pub(crate) fn parse_font_input(env: Env, font: Object) -> Result<(FontInput, Buffer)> {
  if let Ok(buffer) = buffer_from_object(env, font) {
    Ok((FontInput::default(), buffer))
  } else {
    let buffer = font
      .get_named_property("data")
      .and_then(|buffer| buffer_from_object(env, buffer))?;
    let font: FontInput = deserialize_with_tracing(font).map_err(map_error)?;

    Ok((font, buffer))
  }
}

pub(crate) fn resolve_font_resource<'a>(
  font: &'a FontInput,
  buffer: &'a [u8],
) -> Result<FontResource<'a>> {
  FontResource::new(buffer)
    .override_info(FontInfoOverride {
      family_name: font.name.as_deref(),
      width: None,
      style: font.style.map(|style| style.0),
      weight: font.weight.map(|weight| FontWeight::new(weight as f32)),
      axes: None,
    })
    .into_resolved()
    .map_err(|e| Error::from_reason(format!("Failed to load font: {e}")))
}

pub(crate) enum BufferOrSlice<'env> {
  ArrayBuffer(ArrayBuffer<'env>),
  Buffer(BufferSlice<'env>),
  Uint8Array(Uint8ArraySlice<'env>),
}

impl AsRef<[u8]> for BufferOrSlice<'_> {
  fn as_ref(&self) -> &[u8] {
    match self {
      BufferOrSlice::ArrayBuffer(buffer) => buffer,
      BufferOrSlice::Buffer(buffer) => buffer,
      BufferOrSlice::Uint8Array(buffer) => buffer,
    }
  }
}

impl Deref for BufferOrSlice<'_> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.as_ref()
  }
}

pub(crate) fn buffer_slice_from_object<'env>(
  env: Env,
  value: Object<'env>,
) -> Result<BufferOrSlice<'env>> {
  if value.is_buffer()? {
    let buffer = unsafe { BufferSlice::from_napi_value(env.raw(), value.raw()) }?;
    return Ok(BufferOrSlice::Buffer(buffer));
  }

  if value.is_arraybuffer()? {
    let buffer = unsafe { ArrayBuffer::from_napi_value(env.raw(), value.raw()) }?;
    return Ok(BufferOrSlice::ArrayBuffer(buffer));
  }

  if value.is_typedarray()? {
    let buffer = unsafe { Uint8ArraySlice::from_napi_value(env.raw(), value.raw()) }?;
    return Ok(BufferOrSlice::Uint8Array(buffer));
  }

  Err(Error::from_reason(
    "Expected Buffer, ArrayBuffer, or Uint8Array".to_owned(),
  ))
}

pub(crate) fn deserialize_with_tracing<T: DeserializeOwned>(value: Object) -> Result<T> {
  let mut de = De::new(&value);
  T::deserialize(&mut de).map_err(|e| Error::from_reason(e.to_string()))
}

pub(crate) fn map_error<E: Display>(err: E) -> napi::Error {
  napi::Error::from_reason(err.to_string())
}

pub(crate) fn parse_stylesheet(
  stylesheets: Option<Vec<String>>,
  keyframes: Vec<KeyframesRule>,
) -> Result<StyleSheet> {
  let mut stylesheet = StyleSheet::parse_owned_list_loosy(stylesheets.unwrap_or_default());
  stylesheet.extend_keyframes(keyframes);
  Ok(stylesheet)
}

/// Trait for accounting external memory to V8's garbage collector.
///
/// Similar to the optimization in resvg-js PR #393:
/// https://github.com/thx/resvg-js/pull/393
///
/// This allows V8 to be aware of memory allocated in Rust, enabling
/// the garbage collector to trigger based on actual memory pressure.
pub(crate) trait ExternalMemoryAccountable {
  /// Account external memory to V8 by calling adjust_external_memory.
  fn account_external_memory(&self, env: &mut Env) -> Result<()>;
}

impl ExternalMemoryAccountable for Vec<u8> {
  fn account_external_memory(&self, env: &mut Env) -> Result<()> {
    let bytes = self.len() as i64;

    if bytes != 0 {
      env.adjust_external_memory(bytes)?;
    }

    Ok(())
  }
}
