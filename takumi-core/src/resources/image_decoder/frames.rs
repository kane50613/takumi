//! Animation frames: per-frame metadata, what the canvas keeps between frames, and the size a
//! decoded frame resamples down to.

use image::ImageResult;

use super::{MAX_IMAGE_DIMENSION, invalid_buffer_error};
use crate::{
  resources::{image_buffer::ImageBuffer, image_resampler::resample_premultiplied},
  style::ImageScalingAlgorithm,
};

/// What a container says about one frame, read without decoding pixels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameInfo {
  /// Frame rectangle within the canvas, as `(x, y, width, height)`.
  pub(crate) rect: (u32, u32, u32, u32),
  pub(crate) duration_ms: u32,
  /// Composites onto what is under it rather than replacing it.
  pub(crate) blends: bool,
  pub(crate) dispose: Dispose,
}

/// What the canvas holds once a frame has been shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dispose {
  /// Leave the frame in place.
  Keep,
  /// Clear the frame rectangle.
  Background,
  /// Restore what was there before the frame.
  Previous,
}

impl FrameInfo {
  /// A single frame standing in for a stream whose metadata could not be read.
  pub(crate) fn still() -> Self {
    Self {
      rect: (0, 0, u32::MAX, u32::MAX),
      duration_ms: 1,
      blends: false,
      dispose: Dispose::Keep,
    }
  }

  fn covers(&self, canvas: (u32, u32)) -> bool {
    covers_canvas(self.rect, canvas)
  }
}

/// Whether frame `index` needs an earlier frame drawn first.
///
/// Follows `ImageDecoder::FindRequiredPreviousFrame` in Blink.
pub(crate) fn needs_previous_frame(frames: &[FrameInfo], index: usize, canvas: (u32, u32)) -> bool {
  let Some(frame) = frames.get(index) else {
    return false;
  };
  if index == 0 || (!frame.blends && frame.covers(canvas)) {
    return false;
  }

  // A frame restoring what came before it leaves the canvas as it found it, so
  // it is not the starting state for anything after it.
  let mut previous = index - 1;
  while frames[previous].dispose == Dispose::Previous {
    let Some(earlier) = previous.checked_sub(1) else {
      return false;
    };
    previous = earlier;
  }

  match frames[previous].dispose {
    Dispose::Keep => true,
    Dispose::Background => {
      !frames[previous].covers(canvas) && needs_previous_frame(frames, previous, canvas)
    }
    Dispose::Previous => false,
  }
}

/// Total pixels across all frames of an animation (frames are canvas-sized).
pub(crate) const MAX_ANIMATION_TOTAL_PIXELS: u64 =
  4 * MAX_IMAGE_DIMENSION as u64 * MAX_IMAGE_DIMENSION as u64;

/// Frames past this point are dropped from a timeline.
pub(crate) const MAX_ANIMATION_FRAMES: usize = 1024;

/// Whether a frame rectangle spans the entire canvas.
pub(crate) fn covers_canvas(rect: (u32, u32, u32, u32), canvas: (u32, u32)) -> bool {
  let (x, y, width, height) = rect;
  x == 0 && y == 0 && width == canvas.0 && height == canvas.1
}

/// The size decoded frames resample down to, and how.
#[derive(Clone, Copy)]
pub(crate) struct DecodeTarget {
  pub(crate) width: u32,
  pub(crate) height: u32,
  pub(crate) algorithm: ImageScalingAlgorithm,
}

impl DecodeTarget {
  /// Whether a `width` by `height` canvas is larger than the target on either axis.
  pub(super) fn shrinks(self, width: u32, height: u32) -> bool {
    self.width < width || self.height < height
  }

  /// Resamples a premultiplied `source`-sized canvas to the target.
  pub(super) fn resample(self, data: &[u8], source: (u32, u32)) -> Option<ImageBuffer> {
    resample_premultiplied(data, source, (self.width, self.height), self.algorithm)
  }
}

/// Resamples a full-canvas buffer down to `target`, or hands it back untouched.
pub(crate) fn fit_to_target(
  buffer: ImageBuffer,
  target: Option<DecodeTarget>,
) -> ImageResult<ImageBuffer> {
  let Some(target) = target.filter(|target| target.shrinks(buffer.width(), buffer.height())) else {
    return Ok(buffer);
  };

  target
    .resample(buffer.data(), (buffer.width(), buffer.height()))
    .ok_or_else(invalid_buffer_error)
}
