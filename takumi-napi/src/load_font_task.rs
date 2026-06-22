use std::sync::Arc;

use napi::bindgen_prelude::*;

use crate::{FontInput, RegisteredFamily, renderer::RendererState, resolve_font_resource};

pub struct LoadFontTask {
  pub(crate) state: Arc<RendererState>,
  pub(crate) buffer: Buffer,
  pub(crate) info: FontInput,
}

impl Task for LoadFontTask {
  type Output = Vec<RegisteredFamily>;
  type JsValue = Vec<RegisteredFamily>;

  fn compute(&mut self) -> Result<Self::Output> {
    let resource = resolve_font_resource(&self.info, &self.buffer)?;

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
