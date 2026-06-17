use std::sync::{Arc, RwLock};

use napi::bindgen_prelude::*;
use rayon::prelude::*;

use crate::{FontInput, RegisteredFamily, renderer::RendererState, resolve_font_resource};

pub struct LoadFontTask {
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub(crate) buffers: Vec<(FontInput, Buffer)>,
}

impl Task for LoadFontTask {
  type Output = Vec<Vec<RegisteredFamily>>;
  type JsValue = Vec<Vec<RegisteredFamily>>;

  fn compute(&mut self) -> Result<Self::Output> {
    if self.buffers.is_empty() {
      return Ok(Vec::new());
    }

    let resources = crate::pool::install(|| {
      self
        .buffers
        .par_iter()
        .with_min_len(2)
        .map(|(font, buffer): &(FontInput, Buffer)| resolve_font_resource(font, buffer.as_ref()))
        .collect::<Result<Vec<_>>>()
    })?;

    let mut state = self
      .state
      .write()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let registered = resources
      .into_iter()
      .map(|resource| {
        state
          .fonts
          .register(resource)
          .map(|families| families.into_iter().map(Into::into).collect())
          .unwrap_or_default()
      })
      .collect();

    Ok(registered)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}
