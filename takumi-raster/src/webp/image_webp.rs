use std::{borrow::Cow, io::Write};

use image::RgbaImage;
use image_webp::{ColorType, EncoderParams, WebPEncoder};

use crate::{
  Result,
  error::{Error, WebPError},
  webp::{
    EncodedFrame, U24_MAX, UniqueFrame, has_any_alpha_pixel, strip_alpha_channel,
    write_riff_container,
  },
  write::{AnimatedWebpOptions, AnimationFrame, Bitmap},
};

fn predictor_encoder<W: Write>(destination: W) -> WebPEncoder<W> {
  let mut encoder = WebPEncoder::new(destination);
  let mut params = EncoderParams::default();
  params.use_predictor_transform = true;
  encoder.set_params(params);
  encoder
}

pub(crate) fn write_webp_lossless(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
) -> Result<()> {
  let encoder = predictor_encoder(destination);
  let width = image.width();
  let height = image.height();
  let has_alpha = has_any_alpha_pixel(&image);

  let image_data = if has_alpha {
    Cow::Borrowed(image.as_raw())
  } else {
    Cow::Owned(strip_alpha_channel(image))
  };

  encoder
    .encode(
      &image_data,
      width,
      height,
      if has_alpha {
        ColorType::Rgba8
      } else {
        ColorType::Rgb8
      },
    )
    .map_err(Error::encode)?;

  Ok(())
}

fn validate_u24_dimension(name: &'static str, value: u32) -> Result<()> {
  if (1..=U24_MAX + 1).contains(&value) {
    return Ok(());
  }

  Err(
    WebPError::InvalidDimension {
      name,
      value,
      max: U24_MAX + 1,
    }
    .into(),
  )
}

/// Encode a sequence of RGBA frames into an animated WebP and write to `destination`.
pub fn write_animated_webp<W: Write>(
  frames: Cow<'_, [AnimationFrame]>,
  destination: &mut W,
  options: AnimatedWebpOptions,
) -> Result<()> {
  encode_animated_webp(
    frames.into_owned().into_iter().map(Ok),
    destination,
    options,
  )
}

/// Streams frames into an animated WebP, encoding each as it arrives so only one
/// raw frame is held at a time. Only the compact encoded frames are retained,
/// since the RIFF container needs every frame's size before the first byte is
/// written.
pub(crate) fn encode_animated_webp<W, I>(
  mut frames: I,
  destination: &mut W,
  options: AnimatedWebpOptions,
) -> Result<()>
where
  W: Write,
  I: Iterator<Item = Result<AnimationFrame>>,
{
  let Some(first) = frames.next().transpose()? else {
    return Err(WebPError::EmptyAnimation.into());
  };

  let canvas_width = first.image.width();
  let canvas_height = first.image.height();
  validate_u24_dimension("WebP canvas width", canvas_width)?;
  validate_u24_dimension("WebP canvas height", canvas_height)?;

  let validate_frame = |image: &Bitmap, index: usize| -> Result<()> {
    let width = image.width();
    let height = image.height();
    validate_u24_dimension("WebP frame width", width)?;
    validate_u24_dimension("WebP frame height", height)?;
    if width > canvas_width || height > canvas_height {
      return Err(
        WebPError::FrameExceedsCanvas {
          index,
          frame_width: width,
          frame_height: height,
          canvas_width,
          canvas_height,
        }
        .into(),
      );
    }
    Ok(())
  };

  let encode_frame = |frame: UniqueFrame<Bitmap>| -> Result<EncodedFrame<Vec<u8>>> {
    let region = frame.placement.region;
    let cropped;
    let source = if region.covers(frame.image.as_rgba()) {
      frame.image.as_rgba()
    } else {
      cropped = region.crop(frame.image.as_rgba());
      &cropped
    };

    let mut buf = Vec::new();
    predictor_encoder(&mut buf)
      .encode(
        source.as_raw(),
        region.width,
        region.height,
        ColorType::Rgba8,
      )
      .map_err(|_| WebPError::EncodeFailed)?;

    EncodedFrame::new(buf, frame.placement, frame.duration_ms)
      .ok_or_else(|| WebPError::InvalidEncodedData.into())
  };

  // Merge runs of identical frames, matching the native encoder, so a static
  // stretch encodes and stores once.
  validate_frame(&first.image, 0)?;
  let mut encoded = Vec::new();
  let mut pending = UniqueFrame::first(first.image, first.duration_ms, &options);

  for (offset, frame) in frames.enumerate() {
    let frame = frame?;
    validate_frame(&frame.image, offset + 1)?;

    if let Some(finished) = pending.push(
      frame.image,
      frame.duration_ms,
      canvas_width,
      canvas_height,
      &options,
    ) {
      encoded.push(encode_frame(finished)?);
    }
  }
  encoded.push(encode_frame(pending)?);

  write_riff_container(&encoded, canvas_width, canvas_height, destination, &options)?;
  destination.flush()?;

  Ok(())
}
