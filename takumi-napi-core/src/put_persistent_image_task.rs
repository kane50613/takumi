use std::sync::{Arc, RwLock};

use napi::bindgen_prelude::*;
use takumi_core::resources::image::ImageSource as LoadedImageSource;

use crate::{map_error, renderer::RendererState};

pub struct PutPersistentImageTask {
  pub src: Option<String>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub buffer: Buffer,
}

impl Task for PutPersistentImageTask {
  type Output = ();
  type JsValue = ();

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(src) = self.src.take() else {
      unreachable!()
    };

    let state = self
      .state
      .write()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let image = LoadedImageSource::from_bytes(&self.buffer).map_err(map_error)?;
    state.global.persistent_image_store.insert(src, image);

    Ok(())
  }

  fn resolve(&mut self, _env: napi::Env, _output: Self::Output) -> napi::Result<Self::JsValue> {
    Ok(())
  }
}
