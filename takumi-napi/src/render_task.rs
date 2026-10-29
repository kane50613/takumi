use napi::bindgen_prelude::*;
use takumi_raster::{render, write_image};

use crate::{map_error, render_job::RenderJob};

pub struct RenderTask(pub(crate) RenderJob);

impl Task for RenderTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let options = &self.0.options;
    let format = options
      .format
      .unwrap_or_default()
      .raster(options.quality, options.lossless);
    let image = render(self.0.raster()?).map_err(map_error)?;
    let Some(format) = format else {
      return Ok(image.into_raw());
    };
    let mut buffer = Vec::new();

    write_image(&image, &mut buffer, format).map_err(map_error)?;

    Ok(buffer)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
