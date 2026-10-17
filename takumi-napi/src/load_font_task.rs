use std::sync::Arc;

use napi::bindgen_prelude::*;

use takumi_bindings_common::input::FontOptions;

use crate::{JsBytes, RegisteredFamily, map_error, renderer::RendererState};

pub struct LoadFontTask {
  pub(crate) state: Arc<RendererState>,
  pub(crate) buffer: JsBytes,
  pub(crate) info: FontOptions,
  pub(crate) key: Option<String>,
  pub(crate) pinned: bool,
}

impl Task for LoadFontTask {
  type Output = Vec<RegisteredFamily>;
  type JsValue = Vec<RegisteredFamily>;

  fn compute(&mut self) -> Result<Self::Output> {
    let resource = self
      .info
      .resource(self.buffer.as_ref())
      .map_err(map_error)?
      .into_resolved()
      .map_err(map_error)?;

    let bytes = resource.byte_len() as u64;
    let registered = self.state.update_fonts(|fonts, cache| {
      let registered = fonts
        .register(resource)
        .map_err(|e| Error::from_reason(format!("Failed to register font: {e}")))?;

      cache.admit(fonts, self.key.as_deref(), &registered, bytes, self.pinned);

      Ok(registered)
    })?;

    Ok(registered.into_iter().map(Into::into).collect())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}
