use std::{
  borrow::Cow,
  sync::{Arc, RwLock},
};

use napi::bindgen_prelude::*;
use rayon::prelude::*;
use takumi::{
  layout::{Viewport, node::NodeKind},
  rendering::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame,
    RenderOptionsBuilder, encode_animated_gif, encode_animated_png, encode_animated_webp, render,
  },
};

use crate::{
  ExternalMemoryAccountable, map_error,
  renderer::{AnimationOutputFormat, RendererState},
};

pub struct RenderAnimationTask {
  pub nodes: Option<Vec<(NodeKind, u32)>>,
  pub(crate) state: Arc<RwLock<RendererState>>,
  pub viewport: Viewport,
  pub format: AnimationOutputFormat,
  pub quality: Option<u8>,
  pub draw_debug_border: bool,
}

impl Task for RenderAnimationTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    const ENCODED_BYTES_PER_PIXEL_ESTIMATE: usize = 1;
    const FRAME_OVERHEAD_BYTES: usize = 128;
    const MAX_PREALLOC: usize = 4 * 1024 * 1024;

    let Some(nodes) = self.nodes.take() else {
      unreachable!()
    };
    let state = self
      .state
      .read()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let viewport = self.viewport;
    let draw_debug_border = self.draw_debug_border;
    let frames = nodes
      .into_par_iter()
      .map(|(node, duration_ms)| {
        Ok(AnimationFrame::new(
          render(
            RenderOptionsBuilder::default()
              .viewport(viewport)
              .node(node)
              .global(&state.global)
              .draw_debug_border(draw_debug_border)
              .build()
              .map_err(map_error)?,
          )
          .map_err(map_error)?,
          duration_ms,
        ))
      })
      .collect::<Result<Vec<_>, _>>()?;

    // Pre-size conservatively to avoid excessive transient allocations.
    let estimated_capacity = if let Some(first) = frames.first() {
      let w = first.image.width() as usize;
      let h = first.image.height() as usize;
      let per_frame_estimate = w
        .saturating_mul(h)
        .saturating_mul(ENCODED_BYTES_PER_PIXEL_ESTIMATE)
        .saturating_add(FRAME_OVERHEAD_BYTES);
      per_frame_estimate
        .saturating_mul(frames.len())
        .saturating_add(44)
        .min(MAX_PREALLOC)
    } else {
      0
    };
    let mut buffer = Vec::with_capacity(estimated_capacity);

    if let Some(quality) = self.quality
      && quality > 100
    {
      return Err(Error::from_reason(format!(
        "Invalid WebP quality {quality}; expected a value in 0..=100"
      )));
    }

    match self.format {
      AnimationOutputFormat::webp => {
        let mut options = AnimatedWebpOptions::default();
        if let Some(quality) = self.quality {
          options.quality = quality;
        }

        encode_animated_webp(Cow::Owned(frames), &mut buffer, options)
          .map_err(|e| napi::Error::from_reason(e.to_string()))?;
      }
      AnimationOutputFormat::apng => {
        encode_animated_png(&frames, &mut buffer, AnimatedPngOptions::default())
          .map_err(|e| napi::Error::from_reason(e.to_string()))?;
      }
      AnimationOutputFormat::gif => {
        encode_animated_gif(
          Cow::Owned(frames),
          &mut buffer,
          AnimatedGifOptions::default(),
        )
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
      }
    }

    Ok(buffer)
  }

  fn resolve(&mut self, mut env: Env, output: Self::Output) -> Result<Self::JsValue> {
    // Account external memory to V8's garbage collector
    // This enables V8 to collect memory based on actual memory pressure
    output.account_external_memory(&mut env)?;
    Ok(output.into())
  }
}
