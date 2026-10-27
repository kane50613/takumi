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

use std::{fmt::Display, sync::Arc};

use napi::{De, Env, Error, bindgen_prelude::*};
use napi_derive::napi;
pub use renderer::Renderer;
use serde::de::DeserializeOwned;
use takumi_bindings_common::{append_css, input::FontOptions, stylesheet};
use takumi_core::{
  layout::node::Node,
  resources::{glyph_cache, image::ResourceCache},
  style::{CssSource, KeyframesRule, StyleSheet},
};
use takumi_html::{FromHtmlOptions, parse_html};

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

/// A node tree from JS: JSON text from a `{ json }` object or HTML from a string, both parsed on
/// the worker thread, or a tree read from an object.
pub(crate) enum NodeSource {
  Json(String),
  Tree(Box<Node>),
  Html(String),
}

impl NodeSource {
  /// Reads `source`, leaving JSON text and HTML for [`NodeSource::into_parts`] to parse.
  pub(crate) fn from_js(source: Either<String, Object>) -> Result<Self> {
    let object = match source {
      Either::A(html) => return Ok(Self::Html(html)),
      Either::B(object) => object,
    };

    if let Some(json) = object.get::<String>("json")? {
      return Ok(Self::Json(json));
    }

    deserialize_with_tracing(object).map(|node| Self::Tree(Box::new(node)))
  }

  /// The node tree and the CSS of an HTML source's `<style>` elements. JSON fails with the
  /// message the object path reports for the same input.
  pub(crate) fn into_parts(self) -> Result<(Node, Vec<String>)> {
    let json = match self {
      Self::Json(json) => json,
      Self::Tree(node) => return Ok((*node, Vec::new())),
      Self::Html(html) => {
        let parsed = parse_html(&html, FromHtmlOptions::default()).map_err(map_error)?;

        return Ok((parsed.node, parsed.css));
      }
    };

    let node = serde_json::from_str(&json).map_err(|error| {
      let message = error.to_string();
      let position = format!(" at line {} column {}", error.line(), error.column());
      let message = message.strip_suffix(&position).unwrap_or(&message);

      Error::from_reason(format!("{}, {message}", Status::InvalidArg))
    })?;

    Ok((node, Vec::new()))
  }
}

/// A render's source and the CSS that styles it, read on the main thread and parsed on the worker.
pub(crate) struct RenderInput {
  pub(crate) source: NodeSource,
  pub(crate) css: Option<Vec<CssSource>>,
  pub(crate) keyframes: Vec<KeyframesRule>,
}

impl RenderInput {
  /// The node tree and the stylesheet the render uses, `<style>` elements of HTML included.
  pub(crate) fn resolve(self, cache: &ResourceCache) -> Result<(Node, Arc<StyleSheet>)> {
    let (node, document_css) = self.source.into_parts()?;
    let stylesheet =
      stylesheet(cache, append_css(self.css, document_css), self.keyframes).map_err(map_error)?;

    Ok((node, stylesheet))
  }
}

pub(crate) fn map_error<E: Display>(err: E) -> napi::Error {
  napi::Error::from_reason(err.to_string())
}
