use std::sync::{Arc, RwLock};

use napi::bindgen_prelude::*;
use takumi::resources::image::ImageSource as LoadedImageSource;
use xxhash_rust::xxh3::xxh3_64;

use crate::{
  map_error,
  renderer::{ImageCacheKey, RendererState},
};

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

    let cache_key = ImageCacheKey {
      src: src.as_str().into(),
      data_hash: xxh3_64(&self.buffer),
    };

    let mut state = self
      .state
      .write()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;
    if state.persistent_image_cache.contains(&cache_key) {
      return Ok(());
    }
    state.persistent_image_cache.insert(cache_key);
    let image = LoadedImageSource::from_bytes(&self.buffer).map_err(map_error)?;
    state.global.persistent_image_store_mut().insert(src, image);

    Ok(())
  }

  fn resolve(&mut self, _env: napi::Env, _output: Self::Output) -> napi::Result<Self::JsValue> {
    Ok(())
  }
}
