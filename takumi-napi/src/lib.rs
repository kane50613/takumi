//! Node.js N-API bindings for Takumi.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]

#[cfg(target_env = "musl")]
mod allocator;
mod load_font_file_task;
mod load_font_task;
mod measure_task;
mod needs_task;
mod pool;
mod render_animation_task;
mod render_job;
mod render_task;
pub(crate) mod renderer;
mod svg_render_task;

use std::{fmt::Display, sync::Arc};

use napi::{De, Env, Error, bindgen_prelude::*};
use napi_derive::napi;
pub use renderer::Renderer;
use renderer::RendererState;
use serde::de::DeserializeOwned;
use takumi_bindings_common::{
  append_css,
  input::{FontOptions, node_from_json},
  source_cache::{ParsedSource, SourceCache, SourceKind},
  stylesheet,
};
use takumi_core::html::{FromHtmlOptions, parse_html};
use takumi_core::{
  layout::node::Node,
  resources::{
    font::{FontResource, Fonts},
    glyph_cache,
  },
  style::{CssSource, KeyframesRule, StyleSheet},
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

/// Bytes copied out of a JS buffer, so a worker thread never reads memory JS can rewrite or detach.
#[derive(Default)]
pub(crate) struct JsBytes(Box<[u8]>);

impl AsRef<[u8]> for JsBytes {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl JsBytes {
  pub(crate) fn into_vec(self) -> Vec<u8> {
    self.0.into_vec()
  }

  pub(crate) fn from_object(env: Env, value: Object) -> Result<Self> {
    read_bytes(env, value, |bytes| Self(Box::from(bytes)))
  }
}

/// Calls `read` with the bytes of a JS buffer, borrowed for the call alone.
fn read_bytes<R>(env: Env, value: Object, read: impl FnOnce(&[u8]) -> R) -> Result<R> {
  if value.is_buffer()? {
    // SAFETY: `value` is a live Buffer in `env`, and the slice is dropped before this call returns.
    let buffer = unsafe { BufferSlice::from_napi_value(env.raw(), value.raw()) }?;
    return Ok(read(&buffer));
  }

  if value.is_typedarray()? {
    // SAFETY: `value` is a live typed array in `env`, whose element type `from_napi_value` checks,
    // and the slice is dropped before this call returns.
    let array = unsafe { Uint8ArraySlice::from_napi_value(env.raw(), value.raw()) }?;
    return Ok(read(&array));
  }

  if value.is_arraybuffer()? {
    // SAFETY: `value` is a live ArrayBuffer in `env`, and the view is dropped before this call
    // returns.
    let buffer = unsafe { ArrayBuffer::from_napi_value(env.raw(), value.raw()) }?;
    return Ok(read(&buffer));
  }

  Err(Error::from_reason(
    "Expected Buffer, ArrayBuffer, or Uint8Array".to_owned(),
  ))
}

/// The font a `Font` input describes, copied unless [`Fonts::resolved`] finds its bytes.
pub(crate) fn read_font(env: Env, font: Object, fonts: &Fonts) -> Result<FontResource<'static>> {
  let resolve = |info: FontOptions, data: Object| {
    read_bytes(env, data, |bytes| {
      let resource = info.resource(bytes).map_err(map_error)?;

      Ok(
        fonts
          .resolved(&resource)
          .unwrap_or_else(|| resource.into_owned()),
      )
    })?
  };

  if font.is_buffer()? || font.is_typedarray()? || font.is_arraybuffer()? {
    return resolve(FontOptions::default(), font);
  }

  resolve(
    deserialize_with_tracing(font)?,
    font.get_named_property("data")?,
  )
}

pub(crate) fn deserialize_with_tracing<T: DeserializeOwned>(value: Object) -> Result<T> {
  let mut de = De::new(&value);
  T::deserialize(&mut de).map_err(|e| Error::from_reason(e.to_string()))
}

/// Options read from `options`, their defaults when it is absent.
pub(crate) fn read_options<T: DeserializeOwned + Default>(options: Option<Object>) -> Result<T> {
  Ok(
    options
      .map(deserialize_with_tracing)
      .transpose()?
      .unwrap_or_default(),
  )
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
  /// HTML and JSON come from `sources` when they are the text it parsed last.
  pub(crate) fn into_parts(self, sources: &SourceCache) -> Result<ParsedSource> {
    match self {
      Self::Tree(node) => Ok((*node, Vec::new())),
      Self::Html(html) => sources.parse(SourceKind::Html, &html, |html| {
        let parsed = parse_html(html, FromHtmlOptions::default()).map_err(map_error)?;

        Ok((parsed.node, parsed.css))
      }),
      Self::Json(json) => sources.parse(SourceKind::Json, &json, |json| {
        let node = node_from_json(json)
          .map_err(|message| Error::from_reason(format!("{}, {message}", Status::InvalidArg)))?;

        Ok((node, Vec::new()))
      }),
    }
  }
}

/// A render's source and the CSS that styles it, read on the main thread and parsed on the worker.
pub(crate) struct RenderInput {
  source: NodeSource,
  css: Option<Vec<CssSource>>,
  keyframes: Vec<KeyframesRule>,
}

impl RenderInput {
  pub(crate) fn new(
    source: NodeSource,
    css: Option<Vec<CssSource>>,
    keyframes: Vec<KeyframesRule>,
  ) -> Self {
    Self {
      source,
      css,
      keyframes,
    }
  }

  /// The node tree and the stylesheet the render uses, `<style>` elements of HTML included.
  pub(crate) fn resolve(self, state: &RendererState) -> Result<(Node, Arc<StyleSheet>)> {
    let (node, document_css) = self.source.into_parts(&state.sources)?;
    let stylesheet = stylesheet(
      &state.resource_cache,
      append_css(self.css, document_css),
      self.keyframes,
    )
    .map_err(map_error)?;

    Ok((node, stylesheet))
  }
}

pub(crate) fn map_error<E: Display>(err: E) -> napi::Error {
  napi::Error::from_reason(err.to_string())
}
