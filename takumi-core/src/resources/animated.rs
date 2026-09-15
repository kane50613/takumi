//! An animated image kept as its encoded bytes plus frame timing: a GIF, APNG or animated WebP
//! decoded one frame at a time, at the size it is drawn.

use std::sync::{Arc, OnceLock};

#[cfg(feature = "webp")]
use super::image_decoder::{
  animated_webp_dimensions, decode_webp_frame_alone, decode_webp_frames, is_animated_webp,
  webp_frame_infos,
};
#[cfg(feature = "png")]
use super::image_decoder::{
  apng_dimensions, apng_frame_infos, decode_apng_frame_alone, decode_apng_frames, is_apng,
};
#[cfg(feature = "gif")]
use super::image_decoder::{
  decode_gif_frame_alone, decode_gif_frames, gif_dimensions, gif_frame_infos, is_gif,
};
use super::{
  image::ImageError,
  image_buffer::ImageBuffer,
  image_decoder::{DecodeTarget, FrameInfo, MAX_ANIMATION_FRAMES, required_previous_frame},
};
use crate::{resources::image::cover_target, style::ImageScalingAlgorithm};

/// A lazily decoded animated GIF. Only the first frame and the (pixel-free)
/// per-frame timing are retained; every later frame is decoded on demand at the
/// size it is drawn and dropped afterwards, so the whole timeline never sits in
/// memory at once. No cache holds decoded frames — retention stays a single
/// frame, and the byte budget can account for it exactly.
#[derive(Debug, Clone)]
pub struct AnimatedSource {
  inner: Arc<AnimatedInner>,
}

/// Animation container an [`AnimatedSource`] decodes its frames from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnimatedFormat {
  #[cfg(feature = "gif")]
  Gif,
  #[cfg(feature = "png")]
  Apng,
  #[cfg(feature = "webp")]
  WebP,
}

impl AnimatedFormat {
  /// The animation container these bytes carry, if they carry one.
  pub(crate) fn detect(bytes: &[u8]) -> Option<Self> {
    #[cfg(feature = "gif")]
    if is_gif(bytes) {
      return Some(Self::Gif);
    }

    #[cfg(feature = "png")]
    if is_apng(bytes) {
      return Some(Self::Apng);
    }

    #[cfg(feature = "webp")]
    if is_animated_webp(bytes) {
      return Some(Self::WebP);
    }

    None
  }

  fn dimensions(self, bytes: &[u8]) -> Result<(u32, u32), image::ImageError> {
    match self {
      #[cfg(feature = "gif")]
      Self::Gif => gif_dimensions(bytes),
      #[cfg(feature = "png")]
      Self::Apng => apng_dimensions(bytes),
      #[cfg(feature = "webp")]
      Self::WebP => animated_webp_dimensions(bytes),
    }
  }

  pub(crate) fn decode_frames(
    self,
    bytes: &[u8],
    skip: usize,
    limit: Option<usize>,
    target: Option<DecodeTarget>,
    push: impl FnMut(Arc<ImageBuffer>),
  ) -> Result<bool, image::ImageError> {
    match self {
      #[cfg(feature = "gif")]
      Self::Gif => decode_gif_frames(bytes, skip, limit, target, push),
      #[cfg(feature = "png")]
      Self::Apng => decode_apng_frames(bytes, skip, limit, target, push),
      #[cfg(feature = "webp")]
      Self::WebP => decode_webp_frames(bytes, skip, limit, target, push),
    }
  }

  /// Decodes one frame without replaying the frames before it. `None` when the
  /// container shows the frame depends on them.
  pub(crate) fn decode_frame_alone(
    self,
    bytes: &[u8],
    index: usize,
    target: DecodeTarget,
  ) -> Option<ImageBuffer> {
    match self {
      #[cfg(feature = "gif")]
      Self::Gif => decode_gif_frame_alone(bytes, index, Some(target)),
      #[cfg(feature = "png")]
      Self::Apng => decode_apng_frame_alone(bytes, index, Some(target)),
      #[cfg(feature = "webp")]
      Self::WebP => decode_webp_frame_alone(bytes, index, Some(target)),
    }
  }

  /// Per-frame metadata in stream order, read without decoding pixels.
  fn frame_infos(self, bytes: &[u8]) -> Result<Box<[FrameInfo]>, image::ImageError> {
    match self {
      #[cfg(feature = "gif")]
      Self::Gif => gif_frame_infos(bytes),
      #[cfg(feature = "png")]
      Self::Apng => apng_frame_infos(bytes),
      #[cfg(feature = "webp")]
      Self::WebP => webp_frame_infos(bytes),
    }
  }
}

#[derive(Debug)]
struct AnimatedInner {
  format: AnimatedFormat,
  bytes: Box<[u8]>,
  width: u32,
  height: u32,
  timing: OnceLock<AnimationTiming>,
}

/// Per-frame metadata for the whole animation, read once without pixels.
#[derive(Debug)]
struct AnimationTiming {
  /// Every frame in stream order, first frame included.
  frames: Box<[FrameInfo]>,
  /// Duration of the whole loop.
  total_ms: u64,
}

impl AnimatedSource {
  pub(crate) fn from_bytes(format: AnimatedFormat, bytes: &[u8]) -> Result<Self, ImageError> {
    let (width, height) = format.dimensions(bytes).map_err(ImageError::decode)?;

    // Decoded only to reject a stream carrying no frame at all; the pixels go.
    let mut decodable = false;
    format
      .decode_frames(bytes, 0, Some(1), None, |_| decodable = true)
      .map_err(ImageError::decode)?;
    if !decodable {
      return Err(ImageError::InvalidAnimation);
    }

    Ok(Self {
      inner: Arc::new(AnimatedInner {
        format,
        bytes: bytes.into(),
        width,
        height,
        timing: OnceLock::new(),
      }),
    })
  }

  /// The animation canvas dimensions in pixels.
  pub fn dimensions(&self) -> (u32, u32) {
    (self.inner.width, self.inner.height)
  }

  /// Per-frame timing, read once (pixel-free) and memoized. Falls back to a
  /// single-frame loop if the stream can't be re-read.
  fn timing(&self) -> &AnimationTiming {
    self.inner.timing.get_or_init(|| {
      let frames = self
        .inner
        .format
        .frame_infos(&self.inner.bytes)
        .ok()
        .filter(|frames| !frames.is_empty())
        .unwrap_or_else(|| Box::from([FrameInfo::still()]));
      let total_ms = frames.iter().map(|frame| frame.duration_ms as u64).sum();

      AnimationTiming { frames, total_ms }
    })
  }

  /// Whether the frame can be drawn without the frames before it.
  pub(crate) fn stands_alone(&self, index: usize) -> bool {
    index > 0
      && index < MAX_ANIMATION_FRAMES
      && required_previous_frame(
        &self.timing().frames,
        index,
        (self.inner.width, self.inner.height),
      )
      .is_none()
  }

  /// Stream index of the frame shown at the given playback time, looping over
  /// the total duration.
  fn frame_index_at(&self, timing: &AnimationTiming, time_ms: u64) -> usize {
    if timing.total_ms == 0 || timing.frames.len() <= 1 {
      return 0;
    }

    let target_time = time_ms % timing.total_ms;
    let mut elapsed_ms = 0_u64;
    for (index, frame) in timing.frames.iter().enumerate() {
      elapsed_ms += frame.duration_ms as u64;
      if target_time < elapsed_ms {
        return index;
      }
    }

    timing.frames.len() - 1
  }

  /// Frame shown at the given playback time, looping over total duration.
  #[cfg(test)]
  #[cfg(test)]
  pub(crate) fn frame_at_time(&self, time_ms: u64) -> Arc<ImageBuffer> {
    self.frame_at_time_covering(
      time_ms,
      self.inner.width,
      self.inner.height,
      ImageScalingAlgorithm::Auto,
    )
  }

  /// Frame shown at the given playback time, looping over total duration,
  /// decoded to cover a `width` x `height` draw box (never upscaled).
  pub fn frame_at_time_covering(
    &self,
    time_ms: u64,
    width: u32,
    height: u32,
    algorithm: ImageScalingAlgorithm,
  ) -> Arc<ImageBuffer> {
    let timing = self.timing();
    let index = self.frame_index_at(timing, time_ms);
    let (width, height) = cover_target((self.inner.width, self.inner.height), (width, height));
    let target = DecodeTarget {
      width,
      height,
      algorithm,
    };

    self
      .decode_frame(index, target)
      .or_else(|| self.decode_frame(0, target))
      .unwrap_or_else(|| {
        log::warn!("Failed to decode any frame of an animated image, drawing nothing.");
        Arc::new(ImageBuffer::transparent_pixel())
      })
  }

  /// Decodes a single frame by stream index, resampled to `target`. A frame
  /// that depends on earlier ones is reached by replaying them, since disposal
  /// is stateful.
  fn decode_frame(&self, index: usize, target: DecodeTarget) -> Option<Arc<ImageBuffer>> {
    if self.stands_alone(index)
      && let Some(frame) = self
        .inner
        .format
        .decode_frame_alone(&self.inner.bytes, index, target)
    {
      return Some(Arc::new(frame));
    }

    let mut frame = None;
    if let Err(error) =
      self
        .inner
        .format
        .decode_frames(&self.inner.bytes, index, Some(1), Some(target), |decoded| {
          frame = Some(decoded)
        })
    {
      log::warn!("Failed to decode frame {index} of an animated image: {error}");
    }

    frame
  }

  /// Bytes retained for cache budgeting: decoded frames are never held.
  pub(crate) fn decoded_bytes(&self) -> usize {
    self.inner.bytes.len()
  }
}
