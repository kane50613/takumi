#![deny(clippy::unwrap_used, clippy::expect_used)]
#![allow(
  clippy::module_name_repetitions,
  clippy::missing_errors_doc,
  clippy::missing_panics_doc,
  clippy::must_use_candidate
)]

mod load_font_task;
mod measure_task;
mod put_persistent_image_task;
mod render_animation_task;
mod render_task;
mod renderer;

use std::{fmt::Display, ops::Deref};

use napi::{De, Env, Error, JsValue, bindgen_prelude::*};
use napi_derive::napi;
pub use renderer::Renderer;
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use takumi::{
  layout::node::{Node, NodeKind},
  parley::FontStyle,
  resources::task::FetchTaskCollection,
};

#[derive(Deserialize, Default)]
pub(crate) struct FontInput {
  pub name: Option<String>,
  pub weight: Option<f64>,
  pub style: Option<FontStyleInput>,
}

#[derive(Clone, Copy)]
pub struct FontStyleInput(pub FontStyle);

impl<'de> Deserialize<'de> for FontStyleInput {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    Ok(FontStyleInput(FontStyle::parse(&s).unwrap_or_default()))
  }
}

fn buffer_from_object(env: Env, value: Object) -> Result<Buffer> {
  if let Ok(buffer) = unsafe { ArrayBuffer::from_napi_value(env.raw(), value.raw()) } {
    return Ok((*buffer).into());
  }

  unsafe { Buffer::from_napi_value(env.raw(), value.raw()) }
}

pub(crate) enum BufferOrSlice<'env> {
  ArrayBuffer(ArrayBuffer<'env>),
  Slice(BufferSlice<'env>),
}

impl<'env> Deref for BufferOrSlice<'env> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    match self {
      BufferOrSlice::ArrayBuffer(buffer) => buffer,
      BufferOrSlice::Slice(buffer) => buffer,
    }
  }
}

pub(crate) fn buffer_slice_from_object<'env>(
  env: Env,
  value: Object,
) -> Result<BufferOrSlice<'env>> {
  if let Ok(buffer) = unsafe { ArrayBuffer::from_napi_value(env.raw(), value.raw()) } {
    return Ok(BufferOrSlice::ArrayBuffer(buffer));
  }

  unsafe { BufferSlice::from_napi_value(env.raw(), value.raw()).map(BufferOrSlice::Slice) }
}

pub(crate) fn deserialize_with_tracing<T: DeserializeOwned>(value: Object) -> Result<T> {
  let mut de = De::new(&value);
  T::deserialize(&mut de).map_err(|e| Error::from_reason(e.to_string()))
}

pub(crate) fn map_error<E: Display>(err: E) -> napi::Error {
  napi::Error::from_reason(err.to_string())
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

/// Collects the fetch task urls from the node.
#[napi(ts_args_type = "node: AnyNode")]
pub fn extract_resource_urls(node: Object) -> Result<Vec<String>> {
  let node: NodeKind = deserialize_with_tracing(node)?;

  let mut collection = FetchTaskCollection::default();

  node.collect_fetch_tasks(&mut collection);
  node.collect_style_fetch_tasks(&mut collection);

  Ok(
    collection
      .into_inner()
      .iter()
      .map(|task| task.to_string())
      .collect(),
  )
}
