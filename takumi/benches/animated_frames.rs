use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use takumi::prelude::{ImageScalingAlgorithm, ImageSource};

mod common;

const SIZE: u32 = 200;
const FRAME_COUNTS: [usize; 3] = [30, 120, 300];

fn frame_color(index: usize) -> [u8; 4] {
  [(index * 7 % 256) as u8, 40, 200, 255]
}

fn encoded_gif(frames: usize) -> Vec<u8> {
  use image::{Delay, Frame, Rgba, RgbaImage, codecs::gif::GifEncoder};

  let mut bytes = Vec::new();
  let mut encoder = GifEncoder::new(&mut bytes);
  encoder
    .encode_frames((0..frames).map(|index| {
      Frame::from_parts(
        RgbaImage::from_pixel(SIZE, SIZE, Rgba(frame_color(index))),
        0,
        0,
        Delay::from_numer_denom_ms(100, 1),
      )
    }))
    .unwrap();
  drop(encoder);

  bytes
}

fn encoded_apng(frames: usize) -> Vec<u8> {
  encoded_apng_frames(frames, None)
}

/// Every frame a half-canvas subframe blended onto the one before it, so no
/// frame stands alone and each sample replays the whole timeline.
fn encoded_dependent_apng(frames: usize) -> Vec<u8> {
  encoded_apng_frames(frames, Some(SIZE / 2))
}

/// `subframe` set makes every frame past the first a blended subframe of that
/// size; otherwise each frame fills and replaces the canvas.
fn encoded_apng_frames(frames: usize, subframe: Option<u32>) -> Vec<u8> {
  use png::{BitDepth, BlendOp, ColorType, Encoder};

  let mut bytes = Vec::new();
  let mut encoder = Encoder::new(&mut bytes, SIZE, SIZE);
  encoder.set_color(ColorType::Rgba);
  encoder.set_depth(BitDepth::Eight);
  encoder.set_animated(frames as u32, 0).unwrap();

  let mut writer = encoder.write_header().unwrap();
  for index in 0..frames {
    let size = subframe.filter(|_| index > 0).unwrap_or(SIZE);
    writer.set_frame_delay(100, 1000).unwrap();
    if size != SIZE {
      writer.set_frame_dimension(size, size).unwrap();
      writer.set_frame_position(0, 0).unwrap();
      writer.set_blend_op(BlendOp::Over).unwrap();
    }
    writer
      .write_image_data(&frame_color(index).repeat((size * size) as usize))
      .unwrap();
  }
  writer.finish().unwrap();

  bytes
}

/// A `VP8X` canvas with the animation flag, then one `ANMF` per frame wrapping
/// a lossless still.
fn encoded_animated_webp(frames: usize) -> Vec<u8> {
  use image_webp::{ColorType, WebPEncoder};

  fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
      bytes.push(0);
    }
    bytes
  }

  fn still(color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    WebPEncoder::new(&mut bytes)
      .encode(
        &color.repeat((SIZE * SIZE) as usize),
        SIZE,
        SIZE,
        ColorType::Rgba8,
      )
      .unwrap();
    bytes[12..].to_vec()
  }

  let dimension = (SIZE - 1).to_le_bytes();
  let mut body = chunk(
    b"VP8X",
    &[
      0b0000_0010,
      0,
      0,
      0,
      dimension[0],
      dimension[1],
      dimension[2],
      dimension[0],
      dimension[1],
      dimension[2],
    ],
  );
  body.extend_from_slice(&chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]));

  for index in 0..frames {
    let mut payload = vec![0, 0, 0, 0, 0, 0];
    payload.extend_from_slice(&dimension[..3]);
    payload.extend_from_slice(&dimension[..3]);
    payload.extend_from_slice(&100_u32.to_le_bytes()[..3]);
    // Bit 1 set: replace the canvas rect instead of blending onto it.
    payload.push(0b0000_0010);
    payload.extend_from_slice(&still(frame_color(index)));
    body.extend_from_slice(&chunk(b"ANMF", &payload));
  }

  let mut bytes = b"RIFF".to_vec();
  bytes.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
  bytes.extend_from_slice(b"WEBP");
  bytes.extend_from_slice(&body);

  bytes
}

/// Samples the last frame of an animation, the worst case for a decoder that
/// replays from the start.
///
/// The `standalone` sources are full-canvas frames that replace what is under
/// them, so each is decoded on its own. The `dependent` source blends
/// half-canvas subframes, which no shortcut can serve, so it is the replay
/// path's own cost.
fn bench_last_frame(c: &mut Criterion) {
  let mut group = c.benchmark_group("animated_frames");

  for (format, encode) in [
    ("standalone/gif", encoded_gif as fn(usize) -> Vec<u8>),
    ("standalone/apng", encoded_apng),
    ("standalone/webp", encoded_animated_webp),
    ("dependent/apng", encoded_dependent_apng),
  ] {
    for frames in FRAME_COUNTS {
      let Ok(ImageSource::Animated(animated)) = ImageSource::from_bytes(&encode(frames)) else {
        panic!("{format} did not decode as an animated source");
      };
      let last_frame_ms = (frames as u64 - 1) * 100;

      group.bench_function(BenchmarkId::new(format, frames), |b| {
        b.iter(|| {
          black_box(animated.frame_at_time_covering(
            last_frame_ms,
            SIZE,
            SIZE,
            ImageScalingAlgorithm::Auto,
          ))
        })
      });
    }
  }

  group.finish();
}

/// Walks the whole timeline, one sample per frame, the way rendering an
/// animation does.
fn bench_whole_timeline(c: &mut Criterion) {
  let mut group = c.benchmark_group("animated_timeline");

  for (format, encode) in [
    ("standalone/apng", encoded_apng as fn(usize) -> Vec<u8>),
    ("dependent/apng", encoded_dependent_apng),
  ] {
    for frames in FRAME_COUNTS {
      let Ok(ImageSource::Animated(animated)) = ImageSource::from_bytes(&encode(frames)) else {
        panic!("{format} did not decode as an animated source");
      };

      group.bench_function(BenchmarkId::new(format, frames), |b| {
        b.iter(|| {
          for frame in 0..frames {
            black_box(animated.frame_at_time_covering(
              frame as u64 * 100,
              SIZE,
              SIZE,
              ImageScalingAlgorithm::Auto,
            ));
          }
        })
      });
    }
  }

  group.finish();
}

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_last_frame, bench_whole_timeline
}
criterion_main!(benches);
