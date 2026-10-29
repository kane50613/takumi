use std::{mem::take, sync::Arc};

use napi::bindgen_prelude::*;

use takumi_bindings_common::input::FontOptions;
use takumi_core::resources::font::FontResource;

use crate::{JsBytes, RegisteredFamily, map_error, renderer::RendererState};

pub struct LoadFontTask {
  pub(crate) state: Arc<RendererState>,
  pub(crate) buffer: JsBytes,
  pub(crate) info: FontOptions,
  pub(crate) key: Option<String>,
  pub(crate) pinned: bool,
}

impl Task for LoadFontTask {
  type Output = FontResource<'static>;
  type JsValue = Vec<RegisteredFamily>;

  /// Decompresses the font off the JS thread, the costly part of loading it.
  fn compute(&mut self) -> Result<Self::Output> {
    self
      .info
      .resource(take(&mut self.buffer).into_vec())
      .map_err(map_error)?
      .into_resolved()
      .map_err(map_error)
  }

  /// Registers the font on the JS thread, where every font change runs, then releases fonts past
  /// the budget there too, so nothing is released between a render checking its fonts and
  /// starting.
  fn resolve(&mut self, _env: Env, resource: Self::Output) -> Result<Self::JsValue> {
    let bytes = resource.byte_len() as u64;
    let registered = self.state.update_fonts(|fonts, cache| {
      let registered = fonts
        .register(resource)
        .map_err(|e| Error::from_reason(format!("Failed to register font: {e}")))?;

      if self.pinned {
        cache.record(self.key.as_deref(), &registered, bytes, true);
      } else {
        cache
          .admit(fonts, self.key.as_deref(), &registered, bytes, false)
          .map_err(map_error)?;
      }

      Ok(registered)
    })?;

    Ok(registered.into_iter().map(Into::into).collect())
  }
}
