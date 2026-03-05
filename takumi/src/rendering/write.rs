use std::{borrow::Cow, io::Write};

use gif::{Encoder as GifEncoder, Frame as GifFrame, Repeat};
use image::{ExtendedColorType, ImageEncoder, ImageFormat, RgbaImage, codecs::jpeg::JpegEncoder};
use png::{ColorType, Compression, Filter};
use serde::Deserialize;

/// Encode a sequence of RGBA frames into an animated WebP and write to `destination`.
pub use super::webp::encode_animated_webp;
use super::webp::{has_any_alpha_pixel, strip_alpha_channel, write_webp};

use crate::{Result, error::TakumiError};

/// Output format for rendered images.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageOutputFormat {
  /// WebP image format, provides good compression and supports animation.
  /// It is useful for images in web contents.
  WebP,

  /// PNG image format, lossless and widely supported, and its the fastest format to encode.
  Png,

  /// JPEG image format, lossy and does not support transparency.
  Jpeg,
}

impl ImageOutputFormat {
  /// Returns the MIME type for the image output format.
  pub fn content_type(&self) -> &'static str {
    match self {
      ImageOutputFormat::WebP => "image/webp",
      ImageOutputFormat::Png => "image/png",
      ImageOutputFormat::Jpeg => "image/jpeg",
    }
  }
}

impl From<ImageOutputFormat> for ImageFormat {
  fn from(format: ImageOutputFormat) -> Self {
    match format {
      ImageOutputFormat::WebP => Self::WebP,
      ImageOutputFormat::Png => Self::Png,
      ImageOutputFormat::Jpeg => Self::Jpeg,
    }
  }
}

/// Represents a single frame of an animated image.
#[derive(Debug, Clone)]
pub struct AnimationFrame {
  /// The image data for the frame.
  pub image: RgbaImage,
  /// The duration of the frame in milliseconds.
  /// Maximum value is 0xffffff (24-bit), overflow will be clamped.
  pub duration_ms: u32,
}

impl AnimationFrame {
  /// Creates a new animation frame.
  pub fn new(image: RgbaImage, duration_ms: u32) -> Self {
    Self { image, duration_ms }
  }
}

/// Encoding options for animated WebP output.
#[derive(Debug, Clone, Copy)]
pub struct AnimatedWebpOptions {
  /// Whether frames should be alpha-blended with previous content.
  pub blend: bool,
  /// Whether frame disposal clears to background before the next frame.
  pub dispose: bool,
  /// Number of times to loop; `None` means infinite loop.
  pub loop_count: Option<u16>,
  /// Quality in range `0..=100`; `100` is treated as lossless by native backend.
  pub quality: u8,
  /// Encoding speed in range `0..=6`; `0` is fastest (lowest compression), `6` is
  /// slowest (best compression). `None` uses the default speed of `1`.
  ///
  /// Only effective on native targets (libwebp). Ignored on WASM.
  pub speed: Option<u8>,
}

impl Default for AnimatedWebpOptions {
  fn default() -> Self {
    Self {
      blend: true,
      dispose: false,
      loop_count: None,
      quality: 100,
      speed: None,
    }
  }
}

/// Encoding options for animated PNG output.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnimatedPngOptions {
  /// Number of times to loop; `None` means infinite loop.
  pub loop_count: Option<u16>,
}

/// Encoding options for animated GIF output.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnimatedGifOptions {
  /// Number of times to loop; `None` means infinite loop.
  pub loop_count: Option<u16>,
}

fn duration_ms_to_gif_delay(duration_ms: u32) -> u16 {
  if duration_ms == 0 {
    0
  } else {
    duration_ms.div_ceil(10).min(u16::MAX as u32) as u16
  }
}

/// Writes a single rendered image to `destination` using `format`.
pub fn write_image<'a, T: Write>(
  image: Cow<'a, RgbaImage>,
  destination: &mut T,
  format: ImageOutputFormat,
  quality: Option<u8>,
) -> Result<()> {
  match format {
    ImageOutputFormat::Jpeg => {
      let width = image.width();
      let height = image.height();
      let rgb = strip_alpha_channel(image);

      let encoder = JpegEncoder::new_with_quality(destination, quality.unwrap_or(75));
      encoder.write_image(&rgb, width, height, ExtendedColorType::Rgb8)?;
    }
    ImageOutputFormat::Png => {
      let mut encoder = png::Encoder::new(destination, image.width(), image.height());

      let has_alpha = has_any_alpha_pixel(&image);

      let image_data = if has_alpha {
        Cow::Borrowed(image.as_raw())
      } else {
        Cow::Owned(strip_alpha_channel(image))
      };

      encoder.set_color(if has_alpha {
        ColorType::Rgba
      } else {
        ColorType::Rgb
      });

      // Use quality settings to determine compression level.
      // Higher quality settings map to better compression ratio (slower).
      // If quality is not specified or < 90, we favor speed.
      let quality = quality.unwrap_or(75);
      if quality >= 90 {
        encoder.set_compression(Compression::Balanced);
      } else {
        encoder.set_compression(Compression::Fast);
      }

      // Fast subtraction filter handles smooth gradients well with minimal overhead.
      encoder.set_filter(Filter::Sub);

      let mut writer = encoder.write_header()?;
      writer.write_image_data(&image_data)?;
      writer.finish()?;
    }
    ImageOutputFormat::WebP => {
      write_webp(image, destination, quality)?;
    }
  }

  Ok(())
}

/// Encode a sequence of RGBA frames into an animated GIF and write to `destination`.
pub fn encode_animated_gif<W: Write>(
  frames: Cow<'_, [AnimationFrame]>,
  destination: &mut W,
  options: AnimatedGifOptions,
) -> Result<()> {
  if frames.is_empty() {
    return Err(TakumiError::EmptyAnimationFrames { format: "GIF" });
  }

  let width = frames[0].image.width();
  let height = frames[0].image.height();

  if width > u16::MAX as u32 || height > u16::MAX as u32 {
    return Err(TakumiError::GifFrameDimensionsTooLarge {
      width,
      height,
      max: u16::MAX,
    });
  }

  for frame in frames.iter() {
    if frame.image.width() != width || frame.image.height() != height {
      return Err(TakumiError::MixedAnimationFrameDimensions { format: "GIF" });
    }
  }

  let width = width as u16;
  let height = height as u16;
  let mut encoder = GifEncoder::new(destination, width, height, &[])?;
  encoder.set_repeat(options.loop_count.map_or(Repeat::Infinite, Repeat::Finite))?;

  for frame in frames.into_owned().into_iter() {
    let mut pixels = frame.image.into_raw();
    let mut gif_frame = GifFrame::from_rgba_speed(width, height, &mut pixels, 28);
    gif_frame.delay = duration_ms_to_gif_delay(frame.duration_ms);
    encoder.write_frame(&gif_frame)?;
  }

  Ok(())
}

/// Encode a sequence of RGBA frames into an animated PNG and write to `destination`.
pub fn encode_animated_png<W: Write>(
  frames: &[AnimationFrame],
  destination: &mut W,
  options: AnimatedPngOptions,
) -> Result<()> {
  if frames.is_empty() {
    return Err(TakumiError::EmptyAnimationFrames { format: "APNG" });
  }

  let width = frames[0].image.width();
  let height = frames[0].image.height();
  for frame in frames.iter() {
    if frame.image.width() != width || frame.image.height() != height {
      return Err(TakumiError::MixedAnimationFrameDimensions { format: "APNG" });
    }
  }

  let mut encoder = png::Encoder::new(destination, width, height);

  encoder.set_color(ColorType::Rgba);
  encoder.set_compression(png::Compression::Fastest);
  encoder.set_animated(frames.len() as u32, options.loop_count.unwrap_or(0) as u32)?;

  // Since APNG doesn't support variable frame duration, we use the minimum duration of all frames.
  let min_duration_ms = frames
    .iter()
    .map(|frame| frame.duration_ms)
    .min()
    .unwrap_or(0);

  encoder.set_frame_delay(min_duration_ms.clamp(0, u16::MAX as u32) as u16, 1000)?;

  let mut writer = encoder.write_header()?;

  for frame in frames {
    writer.write_image_data(frame.image.as_raw())?;
  }

  writer.finish()?;

  Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
  use std::{borrow::Cow, io::Cursor, mem::MaybeUninit, slice::from_raw_parts};

  use gif::{ColorOutput, DecodeOptions};
  use image::RgbaImage;
  use libwebp_sys::WEBP_CSP_MODE::MODE_RGBA;
  use libwebp_sys::*;

  use super::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame,
    encode_animated_gif, encode_animated_png, encode_animated_webp,
  };

  #[test]
  fn encode_animated_gif_writes_valid_animation_and_delays() {
    let frame_a = AnimationFrame::new(
      RgbaImage::from_fn(2, 2, |x, y| {
        if x == 0 && y == 0 {
          image::Rgba([255, 0, 0, 255])
        } else {
          image::Rgba([0, 0, 0, 0])
        }
      }),
      45,
    );
    let frame_b = AnimationFrame::new(
      RgbaImage::from_fn(2, 2, |x, y| {
        if x == 1 && y == 1 {
          image::Rgba([0, 255, 0, 255])
        } else {
          image::Rgba([0, 0, 0, 0])
        }
      }),
      10,
    );

    let mut bytes = Vec::new();
    let encode_result = encode_animated_gif(
      Cow::Owned(vec![frame_a, frame_b]),
      &mut bytes,
      AnimatedGifOptions {
        loop_count: Some(7),
      },
    );
    assert!(encode_result.is_ok(), "failed to encode animated gif");

    let mut decoder_options = DecodeOptions::new();
    decoder_options.set_color_output(ColorOutput::RGBA);
    let decode_result = decoder_options.read_info(Cursor::new(&bytes));
    assert!(decode_result.is_ok(), "failed to decode animated gif");

    let mut decoder = decode_result.unwrap_or_else(|_| unreachable!());
    let frame_one = decoder.read_next_frame();
    assert!(frame_one.is_ok(), "missing first decoded gif frame");
    let frame_one = frame_one.unwrap_or_else(|_| unreachable!());
    assert!(frame_one.is_some(), "missing first decoded gif frame");
    let frame_one = frame_one.unwrap_or_else(|| unreachable!());
    assert_eq!(frame_one.delay, 5);

    let frame_two = decoder.read_next_frame();
    assert!(frame_two.is_ok(), "missing second decoded gif frame");
    let frame_two = frame_two.unwrap_or_else(|_| unreachable!());
    assert!(frame_two.is_some(), "missing second decoded gif frame");
    let frame_two = frame_two.unwrap_or_else(|| unreachable!());
    assert_eq!(frame_two.delay, 1);

    let frame_three = decoder.read_next_frame();
    assert!(frame_three.is_ok(), "unexpected decoder error");
    assert!(
      frame_three.unwrap_or_else(|_| unreachable!()).is_none(),
      "only two frames should be encoded"
    );

    assert!(
      bytes
        .windows(b"NETSCAPE2.0".len())
        .any(|chunk| chunk == b"NETSCAPE2.0"),
      "encoded gif should contain application extension for loop count"
    );
    assert!(
      bytes
        .windows(5)
        .any(|chunk| chunk == [0x03, 0x01, 0x07, 0x00, 0x00]),
      "encoded gif should store loop count = 7"
    );
  }

  #[test]
  fn encode_animated_gif_rejects_mismatched_frame_dimensions() {
    let frame_a = AnimationFrame::new(
      RgbaImage::from_fn(2, 2, |_, _| image::Rgba([255, 0, 0, 255])),
      10,
    );
    let frame_b = AnimationFrame::new(
      RgbaImage::from_fn(3, 2, |_, _| image::Rgba([0, 255, 0, 255])),
      10,
    );

    let mut bytes = Vec::new();
    let encode_result = encode_animated_gif(
      Cow::Owned(vec![frame_a, frame_b]),
      &mut bytes,
      AnimatedGifOptions::default(),
    );
    assert!(encode_result.is_err(), "mismatched frames should error");
    assert!(
      bytes.is_empty(),
      "encoder should not write bytes before validating frame dimensions"
    );
  }

  #[test]
  fn encode_animated_gif_rejects_empty_frames() {
    let mut bytes = Vec::new();
    let result = encode_animated_gif(
      Cow::Owned(Vec::new()),
      &mut bytes,
      AnimatedGifOptions::default(),
    );
    let err = result.err();
    assert!(err.is_some(), "empty frame list should be rejected");
    let err = err.unwrap_or_else(|| unreachable!());
    assert_eq!(
      err.to_string(),
      "GIF animation must contain at least one frame",
      "unexpected error message: {err}"
    );
  }

  #[test]
  fn encode_animated_png_rejects_empty_frames() {
    let mut bytes = Vec::new();
    let result = encode_animated_png(&[], &mut bytes, AnimatedPngOptions::default());
    let err = result.err();
    assert!(err.is_some(), "empty frame list should be rejected");
    let err = err.unwrap_or_else(|| unreachable!());
    assert_eq!(
      err.to_string(),
      "APNG animation must contain at least one frame",
      "unexpected error message: {err}"
    );
  }

  #[test]
  fn encode_animated_png_rejects_mismatched_frame_dimensions() {
    let frames = vec![
      AnimationFrame::new(
        RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])),
        100,
      ),
      AnimationFrame::new(
        RgbaImage::from_pixel(3, 2, image::Rgba([0, 255, 0, 255])),
        100,
      ),
    ];

    let mut bytes = Vec::new();
    let result = encode_animated_png(&frames, &mut bytes, AnimatedPngOptions::default());
    let err = result.err();
    assert!(err.is_some(), "mismatched frame sizes should be rejected");
    let err = err.unwrap_or_else(|| unreachable!());
    assert_eq!(
      err.to_string(),
      "all APNG animation frames must share the same dimensions",
      "unexpected error message: {err}"
    );
  }

  #[test]
  fn encode_animated_webp_respects_blend_dispose_and_loop_count() {
    let frame_a = AnimationFrame::new(
      RgbaImage::from_fn(2, 2, |x, y| {
        if x == 0 && y == 0 {
          image::Rgba([255, 0, 0, 255])
        } else {
          image::Rgba([0, 0, 0, 0])
        }
      }),
      120,
    );
    let frame_b = AnimationFrame::new(
      RgbaImage::from_fn(2, 2, |x, y| {
        if x == 1 && y == 1 {
          image::Rgba([0, 255, 0, 255])
        } else {
          image::Rgba([0, 0, 0, 0])
        }
      }),
      240,
    );

    let mut bytes = Vec::new();
    let encode_result = encode_animated_webp(
      Cow::Owned(vec![frame_a, frame_b]),
      &mut bytes,
      AnimatedWebpOptions {
        blend: true,
        dispose: true,
        loop_count: Some(7),
        quality: 100,
        speed: None,
      },
    );
    assert!(encode_result.is_ok(), "failed to encode animated webp");

    let webp_data = WebPData {
      bytes: bytes.as_ptr(),
      size: bytes.len(),
    };
    let mut state = WebPDemuxState::WEBP_DEMUX_PARSING_HEADER;
    let demux =
      unsafe { WebPDemuxInternal(&webp_data, 1, &mut state, WEBP_DEMUX_ABI_VERSION as i32) };
    assert!(!demux.is_null(), "demux should parse encoded animation");

    let loop_count = unsafe { WebPDemuxGetI(demux, WebPFormatFeature::WEBP_FF_LOOP_COUNT) };
    assert_eq!(loop_count, 7);

    let mut iter = MaybeUninit::<WebPIterator>::zeroed();
    let has_frame = unsafe { WebPDemuxGetFrame(demux, 1, iter.as_mut_ptr()) };
    assert_eq!(has_frame, 1, "first frame should be available");

    let mut iter = unsafe { iter.assume_init() };
    assert_eq!(
      iter.dispose_method,
      WebPMuxAnimDispose::WEBP_MUX_DISPOSE_BACKGROUND
    );
    assert_eq!(iter.blend_method, WebPMuxAnimBlend::WEBP_MUX_BLEND);

    unsafe {
      WebPDemuxReleaseIterator(&mut iter);
      WebPDemuxDelete(demux);
    }
  }

  #[test]
  fn encode_animated_webp_lossy_produces_valid_animation() {
    // With allow_mixed=1 libwebp may choose VP8L even at quality<100 when it
    // produces a smaller file (trivial 2×2 solid-colour images always compress
    // better losslessly). We verify the output is a parseable animated WebP.
    let frame = AnimationFrame::new(
      RgbaImage::from_fn(2, 2, |_, _| image::Rgba([20, 80, 220, 255])),
      100,
    );

    let mut bytes = Vec::new();
    let encode_result = encode_animated_webp(
      Cow::Owned(vec![frame]),
      &mut bytes,
      AnimatedWebpOptions {
        quality: 70,
        ..Default::default()
      },
    );
    assert!(
      encode_result.is_ok(),
      "failed to encode lossy animated webp"
    );

    assert!(
      bytes
        .windows(4)
        .any(|chunk| chunk == b"VP8 " || chunk == b"VP8L"),
      "animation should contain a VP8 or VP8L bitstream chunk"
    );

    // Verify it parses as a valid animated WebP
    let webp_data = WebPData {
      bytes: bytes.as_ptr(),
      size: bytes.len(),
    };
    let mut state = WebPDemuxState::WEBP_DEMUX_PARSING_HEADER;
    let demux =
      unsafe { WebPDemuxInternal(&webp_data, 1, &mut state, WEBP_DEMUX_ABI_VERSION as i32) };
    assert!(!demux.is_null(), "lossy animation should be parseable");
    unsafe { WebPDemuxDelete(demux) };
  }

  #[test]
  fn encode_animated_webp_merges_consecutive_identical_frames() {
    let image_a = RgbaImage::from_fn(2, 2, |_, _| image::Rgba([120, 30, 10, 255]));
    let image_b = RgbaImage::from_fn(2, 2, |_, _| image::Rgba([5, 200, 20, 255]));
    let frame_a = AnimationFrame::new(image_a.clone(), 50);
    let frame_b = AnimationFrame::new(image_a, 70);
    let frame_c = AnimationFrame::new(image_b, 30);

    let mut bytes = Vec::new();
    let encode_result = encode_animated_webp(
      Cow::Owned(vec![frame_a, frame_b, frame_c]),
      &mut bytes,
      AnimatedWebpOptions {
        quality: 100,
        ..Default::default()
      },
    );
    assert!(
      encode_result.is_ok(),
      "failed to encode animated webp with repeated frames"
    );

    let webp_data = WebPData {
      bytes: bytes.as_ptr(),
      size: bytes.len(),
    };
    let mut state = WebPDemuxState::WEBP_DEMUX_PARSING_HEADER;
    let demux =
      unsafe { WebPDemuxInternal(&webp_data, 1, &mut state, WEBP_DEMUX_ABI_VERSION as i32) };
    assert!(!demux.is_null(), "demux should parse encoded animation");

    let frame_count = unsafe { WebPDemuxGetI(demux, WebPFormatFeature::WEBP_FF_FRAME_COUNT) };
    assert_eq!(
      frame_count, 2,
      "identical consecutive frames should be merged"
    );

    let mut iter = MaybeUninit::<WebPIterator>::zeroed();
    let has_frame = unsafe { WebPDemuxGetFrame(demux, 1, iter.as_mut_ptr()) };
    assert_eq!(has_frame, 1, "first frame should be available");
    let mut iter = unsafe { iter.assume_init() };
    assert_eq!(
      iter.duration, 120,
      "merged frame should keep total duration"
    );

    unsafe {
      WebPDemuxReleaseIterator(&mut iter);
      WebPDemuxDelete(demux);
    }
  }

  #[test]
  fn encode_animated_webp_rejects_zero_sized_frames() {
    let invalid = AnimationFrame::new(RgbaImage::new(0, 1), 10);

    let mut bytes = Vec::new();
    let result = encode_animated_webp(
      Cow::Owned(vec![invalid]),
      &mut bytes,
      AnimatedWebpOptions::default(),
    );
    let err = result.err();
    assert!(err.is_some(), "zero-sized frame should be rejected");
    let err = err.unwrap_or_else(|| unreachable!());
    assert!(
      err
        .to_string()
        .contains("WebP animation frame dimensions must be in 1..=16777216"),
      "unexpected error message: {err}"
    );
  }

  #[test]
  fn encode_animated_webp_preserves_parallel_frame_order() {
    let frames = vec![
      AnimationFrame::new(
        RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])),
        10,
      ),
      AnimationFrame::new(
        RgbaImage::from_pixel(2, 2, image::Rgba([0, 255, 0, 255])),
        20,
      ),
      AnimationFrame::new(
        RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 255, 255])),
        30,
      ),
      AnimationFrame::new(
        RgbaImage::from_pixel(2, 2, image::Rgba([255, 255, 0, 255])),
        40,
      ),
    ];

    let mut bytes = Vec::new();
    let encode_result = encode_animated_webp(
      Cow::Owned(frames),
      &mut bytes,
      AnimatedWebpOptions {
        quality: 100,
        ..Default::default()
      },
    );
    assert!(
      encode_result.is_ok(),
      "failed to encode animated webp in parallel"
    );

    let webp_data = WebPData {
      bytes: bytes.as_ptr(),
      size: bytes.len(),
    };
    let mut state = WebPDemuxState::WEBP_DEMUX_PARSING_HEADER;
    let demux =
      unsafe { WebPDemuxInternal(&webp_data, 1, &mut state, WEBP_DEMUX_ABI_VERSION as i32) };
    assert!(!demux.is_null(), "demux should parse encoded animation");

    let mut decoder_config = unsafe { MaybeUninit::<WebPDecoderConfig>::zeroed().assume_init() };
    let init_ok = unsafe { WebPInitDecoderConfig(&raw mut decoder_config) };
    assert!(init_ok, "decoder config should initialize");
    decoder_config.output.colorspace = MODE_RGBA;

    let expected_dominant_channels = [
      [true, false, false],
      [false, true, false],
      [false, false, true],
      [true, true, false],
    ];
    let expected_durations = [10, 20, 30, 40];

    let mut iter = MaybeUninit::<WebPIterator>::zeroed();
    let has_frame = unsafe { WebPDemuxGetFrame(demux, 1, iter.as_mut_ptr()) };
    assert_eq!(has_frame, 1, "first frame should be available");
    let mut iter = unsafe { iter.assume_init() };

    for (expected_dominant_channels, expected_duration) in
      expected_dominant_channels.iter().zip(expected_durations)
    {
      let decode_status = unsafe {
        WebPDecode(
          iter.fragment.bytes,
          iter.fragment.size,
          &raw mut decoder_config,
        )
      };
      assert_eq!(
        decode_status,
        VP8StatusCode::VP8_STATUS_OK,
        "frame payload should decode"
      );

      let rgba = unsafe {
        from_raw_parts(
          decoder_config.output.u.RGBA.rgba,
          decoder_config.output.u.RGBA.size,
        )
      };
      let channel_flags = [rgba[0] >= 250, rgba[1] >= 250, rgba[2] >= 250];
      assert_eq!(channel_flags, *expected_dominant_channels);
      assert!(rgba[3] >= 250, "decoded frame should remain opaque");
      assert_eq!(iter.duration, expected_duration);

      unsafe { WebPFreeDecBuffer(&raw mut decoder_config.output) };
      if expected_duration != expected_durations[expected_durations.len() - 1] {
        let has_next = unsafe { WebPDemuxNextFrame(&mut iter) };
        assert_eq!(has_next, 1, "next frame should be available");
      }
    }

    unsafe {
      WebPDemuxReleaseIterator(&mut iter);
      WebPDemuxDelete(demux);
    }
  }
}
