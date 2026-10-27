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

use std::{fmt::Display, mem::take};

use napi::{De, Env, Error, bindgen_prelude::*};
use napi_derive::napi;
pub use renderer::Renderer;
use serde::de::DeserializeOwned;
use takumi_bindings_common::input::FontOptions;
use takumi_core::{layout::node::Node, resources::glyph_cache};
use takumi_html::{FromHtmlOptions, ParsedHtml, parse_html};

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

/// A node tree from JS: JSON text from a `{ json }` object, parsed on the worker thread, a tree
/// read from an object, or HTML from a string, parsed with the CSS of its `<style>` elements.
pub(crate) enum NodeSource {
  Json(String),
  Tree(Box<Node>),
  Html(Box<ParsedHtml>),
}

impl NodeSource {
  /// Reads `source`, leaving JSON text for [`NodeSource::into_node`] to parse.
  pub(crate) fn from_js(source: Either<String, Object>) -> Result<Self> {
    let object = match source {
      Either::A(html) => {
        return parse_html(&html, FromHtmlOptions::default())
          .map(|parsed| Self::Html(Box::new(parsed)))
          .map_err(map_error);
      }
      Either::B(object) => object,
    };

    if let Some(json) = object.get::<String>("json")? {
      return Ok(Self::Json(json));
    }

    deserialize_with_tracing(object).map(|node| Self::Tree(Box::new(node)))
  }

  /// The CSS of an HTML source's `<style>` elements, leaving none behind.
  pub(crate) fn take_css(&mut self) -> Vec<String> {
    match self {
      Self::Html(parsed) => take(&mut parsed.css),
      _ => Vec::new(),
    }
  }

  /// The node tree, failing with the message the object path reports for the same input.
  pub(crate) fn into_node(self) -> Result<Node> {
    let json = match self {
      Self::Json(json) => json,
      Self::Tree(node) => return Ok(*node),
      Self::Html(parsed) => return Ok(parsed.node),
    };

    serde_json::from_str(&json).map_err(|error| {
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
