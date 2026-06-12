use std::sync::{Arc, RwLock};

use napi::bindgen_prelude::*;
use rayon::prelude::*;

use crate::{FontInput, renderer::RendererState, resolve_font_resource};

pub struct LoadFontTask {
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub(crate) buffers: Vec<(FontInput, Buffer)>,
}

impl Task for LoadFontTask {
  type Output = usize;
  type JsValue = u32;

  fn compute(&mut self) -> Result<Self::Output> {
    if self.buffers.is_empty() {
      return Ok(0);
    }

    let mut loaded_count = 0;

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

    for resource in resources.into_iter() {
      if state.global.font_context.load_and_store(resource).is_ok() {
        loaded_count += 1;
      }
    }

    Ok(loaded_count)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output as u32)
  }
}
