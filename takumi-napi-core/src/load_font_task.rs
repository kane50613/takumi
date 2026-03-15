use std::borrow::Cow;
use std::sync::{Arc, RwLock};

use napi::bindgen_prelude::*;
use takumi::parley::{FontWeight, fontique::FontInfoOverride};

use crate::{FontInput, renderer::RendererState};

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
    let mut state = self
      .state
      .write()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    for (font, buffer) in &self.buffers {
      if state
        .global
        .font_context_mut()
        .load_and_store(
          Cow::Borrowed(buffer),
          Some(FontInfoOverride {
            family_name: font.name.as_deref(),
            width: None,
            style: font.style.map(|style| style.0),
            weight: font.weight.map(|weight| FontWeight::new(weight as f32)),
            axes: None,
          }),
          None,
        )
        .is_ok()
      {
        loaded_count += 1;
      }
    }

    Ok(loaded_count)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output as u32)
  }
}
