use std::sync::Arc;

use napi::bindgen_prelude::*;

use takumi_bindings_common::input::RegisteredFont;
use takumi_core::resources::font::FontResource;

use crate::{RegisteredFamily, map_error, renderer::RendererState};

pub struct LoadFontTask {
  pub(crate) state: Arc<RendererState>,
  pub(crate) resource: Option<FontResource<'static>>,
  pub(crate) key: Option<String>,
  pub(crate) pinned: bool,
  /// The leases of the render that loads it, which hold its file.
  pub(crate) lease: Option<u32>,
}

impl Task for LoadFontTask {
  type Output = FontResource<'static>;
  type JsValue = Vec<RegisteredFamily>;

  /// Decompresses the font off the JS thread, the costly part of loading it.
  fn compute(&mut self) -> Result<Self::Output> {
    self
      .resource
      .take()
      .ok_or_else(|| Error::from_reason("The font was already loaded"))?
      .into_resolved()
      .map_err(map_error)
  }

  /// Registers the font on the JS thread, where every font change runs, then unregisters the
  /// fonts the cache evicted to make room, so nothing is released between a render checking its
  /// fonts and starting.
  fn resolve(&mut self, _env: Env, resource: Self::Output) -> Result<Self::JsValue> {
    let (families, held) = self.state.update_fonts(|fonts, registrations| {
      let registered = RegisteredFont::register(fonts, resource)
        .map_err(|e| Error::from_reason(format!("Failed to register font: {e}")))?;

      Ok(registrations.admit(
        fonts,
        &self.state.cache,
        self.key.as_deref(),
        registered,
        self.pinned,
      ))
    })?;

    self.state.leases.hold(self.lease, held);
    self.state.release_evicted();

    Ok(families.into_iter().map(Into::into).collect())
  }
}
