use std::sync::{Arc, RwLock};

use napi::bindgen_prelude::*;

use crate::{FontInput, RegisteredFamily, renderer::RendererState, resolve_font_resource};

pub struct LoadFontTask {
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub(crate) buffer: Buffer,
  pub(crate) info: FontInput,
}

impl Task for LoadFontTask {
  type Output = Vec<RegisteredFamily>;
  type JsValue = Vec<RegisteredFamily>;

  fn compute(&mut self) -> Result<Self::Output> {
    let resource = resolve_font_resource(&self.info, &self.buffer)?;

    let mut state = self
      .state
      .write()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let registered = state
      .fonts
      .register(resource)
      .map_err(|e| Error::from_reason(format!("Failed to register font: {e}")))?;

    Ok(registered.into_iter().map(Into::into).collect())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}
