use napi::bindgen_prelude::*;
use takumi_raster::measure;

use crate::{map_error, render_job::RenderJob, renderer::MeasuredNode};

pub struct MeasureTask(pub(crate) RenderJob);

impl Task for MeasureTask {
  type Output = takumi_raster::MeasuredNode;
  type JsValue = MeasuredNode;

  fn compute(&mut self) -> Result<Self::Output> {
    measure(self.0.raster()?).map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
