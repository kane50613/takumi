use std::{mem::take, sync::Arc};

use napi::bindgen_prelude::*;

use takumi_core::resources::font::FontSource;

use crate::{JsBytes, map_error, renderer::RendererState};

/// Loads an `@font-face` file a render's `needs` reported.
pub struct LoadFontFileTask {
  pub(crate) state: Arc<RendererState>,
  pub(crate) url: String,
  pub(crate) buffer: JsBytes,
  /// The leases of the render that loads it, which hold the file.
  pub(crate) lease: Option<u32>,
}

impl Task for LoadFontFileTask {
  type Output = (FontSource<'static>, usize);
  type JsValue = ();

  /// Decompresses the file off the JS thread, the costly part of loading it.
  fn compute(&mut self) -> Result<Self::Output> {
    FontSource::from(take(&mut self.buffer).into_vec())
      .into_decoded()
      .map_err(map_error)
  }

  /// Hands the file over on the JS thread, where every font change runs, then unregisters the
  /// fonts the cache evicted to make room.
  fn resolve(&mut self, _env: Env, (source, bytes): Self::Output) -> Result<Self::JsValue> {
    let held = self.state.update_fonts(|fonts, registrations| {
      registrations
        .admit_file(fonts, &self.state.cache, &self.url, source, bytes as u64)
        .map_err(map_error)
    })?;

    self.state.leases.hold(self.lease, held);
    self.state.release_evicted();

    Ok(())
  }
}
