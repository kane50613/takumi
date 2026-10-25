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
use serde::de::DeserializeOwned;
use takumi_bindings_common::input::FontOptions;
use takumi_core::{layout::node::Node, resources::glyph_cache, style::InvalidDeclarations};

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

/// Bytes copied out of a JS buffer, so a worker thread never reads memory JS can rewrite or detach.
pub(crate) struct JsBytes(Box<[u8]>);

impl AsRef<[u8]> for JsBytes {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl JsBytes {
  pub(crate) fn from_object(env: Env, value: Object) -> Result<Self> {
    if value.is_buffer()? {
      let buffer = unsafe { BufferSlice::from_napi_value(env.raw(), value.raw()) }?;
      return Ok(Self(Box::from(&*buffer)));
    }

    if value.is_typedarray()? {
      let array = unsafe { Uint8ArraySlice::from_napi_value(env.raw(), value.raw()) }?;
      return Ok(Self(Box::from(&*array)));
    }

    if value.is_arraybuffer()? {
      let buffer = unsafe { ArrayBuffer::from_napi_value(env.raw(), value.raw()) }?;
      return Ok(Self(Box::from(&*buffer)));
    }

    Err(Error::from_reason(
      "Expected Buffer, ArrayBuffer, or Uint8Array".to_owned(),
    ))
  }
}

pub(crate) fn parse_font_input(env: Env, font: Object) -> Result<(FontOptions, JsBytes)> {
  if let Ok(buffer) = JsBytes::from_object(env, font) {
    Ok((FontOptions::default(), buffer))
  } else {
    let buffer = font
      .get_named_property("data")
      .and_then(|buffer| JsBytes::from_object(env, buffer))?;
    let font: FontOptions = deserialize_with_tracing(font).map_err(map_error)?;

    Ok((font, buffer))
  }
}

pub(crate) fn deserialize_with_tracing<T: DeserializeOwned>(value: Object) -> Result<T> {
  let mut de = De::new(&value);
  T::deserialize(&mut de).map_err(|e| Error::from_reason(e.to_string()))
}

/// Reads a node tree from a JS object, warning about the declarations its style objects drop.
pub(crate) fn deserialize_node(env: &Env, value: Object) -> Result<Node> {
  let (node, invalid) = InvalidDeclarations::collect(|| deserialize_with_tracing(value));

  warn_invalid_declarations(env, &invalid)?;

  node
}

/// Reports each dropped declaration through `console.warn`, as a browser console reports one.
pub(crate) fn warn_invalid_declarations(env: &Env, invalid: &InvalidDeclarations) -> Result<()> {
  if invalid.is_empty() {
    return Ok(());
  }

  let console: Object = env.get_global()?.get_named_property("console")?;
  let warn: Function<String, Unknown> = console.get_named_property("warn")?;

  for message in invalid.messages() {
    warn.apply(console, format!("takumi: ignoring {message}"))?;
  }

  Ok(())
}

/// A node tree from JS: JSON text, parsed on the worker thread, or a tree read from an object.
pub(crate) enum NodeSource {
  Json(String),
  Tree(Box<Node>),
}

impl NodeSource {
  /// Reads `source`, leaving JSON text for [`NodeSource::into_node`] to parse.
  pub(crate) fn from_js(env: &Env, source: Either<String, Object>) -> Result<Self> {
    match source {
      Either::A(json) => Ok(Self::Json(json)),
      Either::B(object) => deserialize_node(env, object).map(|node| Self::Tree(Box::new(node))),
    }
  }

  /// The node tree and the declarations JSON text dropped, failing with the message the object path reports for the same input.
  pub(crate) fn into_node(self) -> Result<(Node, InvalidDeclarations)> {
    let json = match self {
      Self::Json(json) => json,
      Self::Tree(node) => return Ok((*node, InvalidDeclarations::default())),
    };
    let (node, invalid) = InvalidDeclarations::collect(|| serde_json::from_str(&json));

    node
      .map(|node| (node, invalid))
      .map_err(|error: serde_json::Error| {
        let message = error.to_string();
        let position = format!(" at line {} column {}", error.line(), error.column());
        let message = message.strip_suffix(&position).unwrap_or(&message);

        Error::from_reason(format!("{}, {message}", Status::InvalidArg))
      })
  }
}

pub(crate) fn map_error<E: Display>(err: E) -> napi::Error {
  napi::Error::from_reason(err.to_string())
}
