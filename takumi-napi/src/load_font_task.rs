use std::sync::Arc;

use napi::bindgen_prelude::*;

use takumi_bindings_common::input::FontOptions;

use crate::{JsBytes, RegisteredFamily, map_error, renderer::RendererState};

pub struct LoadFontTask {
  pub(crate) state: Arc<RendererState>,
  pub(crate) buffer: JsBytes,
  pub(crate) info: FontOptions,
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

    // Serialize registrations; readers stay wait-free on the old snapshot meanwhile.
    let _write = self
      .state
      .font_write
      .lock()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let mut fonts = self.state.fonts.load_full().as_ref().clone();

    let registered = fonts
      .register(resource)
      .map_err(|e| Error::from_reason(format!("Failed to register font: {e}")))?;

    self.state.fonts.store(Arc::new(fonts));

    Ok(registered.into_iter().map(Into::into).collect())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}
