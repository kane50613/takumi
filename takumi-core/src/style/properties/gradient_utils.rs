use std::fmt;

use color::{ColorSpaceTag, HueDirection};
use cssparser::{Parser, Token};
use smallvec::SmallVec;
use tiny_skia::PremultipliedColorU8;

use crate::{
  geometry::Point,
  style::{
    Color, ColorInput, ColorInterpolationMethod, FromCss, GradientStop, ParseResult,
    ResolvedGradientStop, SizingContext, StopPosition, ToCss, math::fast_div_255,
  },
};

const MIN_GRADIENT_LUT_SIZE: usize = 2;
const MAX_GRADIENT_LUT_SIZE: usize = 8193;

/// 8x8 Bayer thresholds in 8.8 fixed point, applied to gradient samples before
/// quantization. Blink dithers every gradient (`gradient.cc`, "Legacy behavior:
/// gradients are always dithered"); an integer-exact sample rounds back to
/// itself, so flat regions and hard stops stay byte-identical.
#[rustfmt::skip]
const DITHER_NOISE_88: [[i16; 8]; 8] = [
  [-128,    0,  -96,   32, -120,    8,  -88,   40],
  [  64,  -64,   96,  -32,   72,  -56,  104,  -24],
  [ -80,   48, -112,   16,  -72,   56, -104,   24],
  [ 112,  -16,   80,  -48,  120,   -8,   88,  -40],
  [-116,   12,  -84,   44, -124,    4,  -92,   36],
  [  76,  -52,  108,  -20,   68,  -60,  100,  -28],
  [ -68,   60, -100,   28,  -76,   52, -108,   20],
  [ 124,   -4,   92,  -36,  116,  -12,   84,  -44],
];

/// A gradient LUT entry in premultiplied 8.8 fixed point.
pub(crate) type GradientLutHiEntry = [u16; 4];

#[cfg(test)]
pub(crate) fn red_blue_stops(
  red_hint: Option<StopPosition>,
  blue_hint: Option<StopPosition>,
) -> [GradientStop; 2] {
  [
    GradientStop::ColorHint {
      color: Color([255, 0, 0, 255]).into(),
      hint: red_hint,
    },
    GradientStop::ColorHint {
      color: Color([0, 0, 255, 255]).into(),
      hint: blue_hint,
    },
  ]
}

/// Emits the field-backed `GradientOverlayTile` accessors shared by every tile type.
macro_rules! gradient_tile_accessors {
  () => {
    #[inline(always)]
    fn width(&self) -> u32 {
      self.width
    }

    #[inline(always)]
    fn height(&self) -> u32 {
      self.height
    }

    #[inline(always)]
    fn lut_len(&self) -> usize {
      self.lut.len()
    }

    #[inline(always)]
    fn sample_at(&self, lut_idx: usize) -> PremultipliedColorU8 {
      self.lut.sample(lut_idx)
    }

    #[inline(always)]
    fn sample_dithered_at(&self, lut_idx: usize, x: u32, y: u32) -> PremultipliedColorU8 {
      self.lut.sample_dithered(lut_idx, x, y)
    }

    #[inline(always)]
    fn dither_active(&self) -> bool {
      self.lut.dither_active()
    }

    #[inline(always)]
    fn fully_opaque(&self) -> bool {
      self.fully_opaque
    }
  };
}

pub(crate) use gradient_tile_accessors;

/// Color functions whose presence flips an unspecified gradient interpolation space from sRGB to
/// Oklab; every other stop syntax is a legacy sRGB form.
const MODERN_COLOR_FUNCTIONS: [&str; 6] = ["lab", "lch", "oklab", "oklch", "color", "color-mix"];

fn peeks_modern_color_function(input: &mut Parser<'_, '_>) -> bool {
  let state = input.state();
  let modern = match input.next() {
    Ok(Token::Function(name)) => {
      MODERN_COLOR_FUNCTIONS
        .iter()
        .any(|function| name.eq_ignore_ascii_case(function))
        // Relative syntax (`rgb(from ...)`) is modern whatever the function name.
        || input
          .parse_nested_block(|arguments| {
            let relative =
              matches!(arguments.next(), Ok(Token::Ident(ident)) if ident.eq_ignore_ascii_case("from"));
            while arguments.next().is_ok() {}
            ParseResult::Ok(relative)
          })
          .unwrap_or(false)
    }
    _ => false,
  };

  input.reset(&state);
  modern
}

/// Parses a comma-separated gradient stop list; `color a b` expands to two stops.
pub(crate) fn parse_gradient_stops<'i>(
  input: &mut Parser<'i, '_>,
  parse_position: fn(&mut Parser<'i, '_>) -> ParseResult<'i, StopPosition>,
) -> ParseResult<'i, (Vec<GradientStop>, bool)> {
  let mut stops = Vec::new();
  let mut modern = false;
  loop {
    if let Ok(hint) = input.try_parse(parse_position) {
      stops.push(GradientStop::Hint(hint));
    } else {
      modern |= peeks_modern_color_function(input);
      let color = ColorInput::from_css(input)?;
      let first_position = input.try_parse(parse_position).ok();
      let second_position = if first_position.is_some() {
        input.try_parse(parse_position).ok()
      } else {
        None
      };

      match (first_position, second_position) {
        (Some(first_position), Some(second_position)) => {
          stops.push(GradientStop::ColorHint {
            color,
            hint: Some(first_position),
          });
          stops.push(GradientStop::ColorHint {
            color,
            hint: Some(second_position),
          });
        }
        (first_position, _) => {
          stops.push(GradientStop::ColorHint {
            color,
            hint: first_position,
          });
        }
      }
    }

    if input.try_parse(Parser::expect_comma).is_err() {
      break;
    }
  }

  Ok((stops, modern))
}

/// Serializes a gradient as `name(<params>, <interpolation>, <stops>)`.
pub(crate) fn write_gradient_css<W: fmt::Write>(
  dest: &mut W,
  name: &str,
  params: &str,
  interpolation: &ColorInterpolationMethod,
  stops: &[GradientStop],
) -> fmt::Result {
  dest.write_str(name)?;
  dest.write_char('(')?;

  // Direction and color-interpolation-method share the leading clause,
  // space-separated (e.g. `to right in srgb`); a comma only precedes the stops.
  let mut has_prelude = false;
  if !params.is_empty() {
    dest.write_str(params)?;
    has_prelude = true;
  }

  // Stops always serialize in legacy forms, so a reparse infers sRGB; only a
  // space that differs from that inference needs writing.
  if *interpolation != ColorInterpolationMethod::LEGACY {
    if has_prelude {
      dest.write_char(' ')?;
    }
    interpolation.to_css(dest)?;
    has_prelude = true;
  }

  for (index, stop) in stops.iter().enumerate() {
    if has_prelude || index > 0 {
      dest.write_str(", ")?;
    }
    stop.to_css(dest)?;
  }

  dest.write_char(')')
}

/// Interpolates two premultiplied colors directly in premultiplied RGBA space.
pub(crate) fn interpolate_rgba_premultiplied(
  c1: PremultipliedColorU8,
  c2: PremultipliedColorU8,
  t: f32,
) -> PremultipliedColorU8 {
  if t <= f32::EPSILON {
    return c1;
  }

  if t >= 1.0 - f32::EPSILON {
    return c2;
  }

  let mut result = [0u8; 4];
  let c1_rgba = [c1.red(), c1.green(), c1.blue(), c1.alpha()];
  let c2_rgba = [c2.red(), c2.green(), c2.blue(), c2.alpha()];

  for i in 0..4 {
    result[i] = (c1_rgba[i] as f32 * (1.0 - t) + c2_rgba[i] as f32 * t)
      .round()
      .clamp(0.0, 255.0) as u8;
  }

  PremultipliedColorU8::from_rgba(
    result[0].min(result[3]),
    result[1].min(result[3]),
    result[2].min(result[3]),
    result[3],
  )
  .unwrap_or(PremultipliedColorU8::TRANSPARENT)
}

/// A precomputed gradient that overlays its samples onto a destination buffer.
pub trait GradientOverlayTile {
  /// Per-row state carried across a horizontal scan.
  type RowState;

  /// Tile width in pixels.
  fn width(&self) -> u32;
  /// Tile height in pixels.
  fn height(&self) -> u32;
  /// Number of LUT entries.
  fn lut_len(&self) -> usize;
  /// LUT entry at `lut_idx`.
  fn sample_at(&self, lut_idx: usize) -> PremultipliedColorU8;
  /// Dithered color for LUT entry `lut_idx` painted at `(x, y)`.
  fn sample_dithered_at(&self, lut_idx: usize, _x: u32, _y: u32) -> PremultipliedColorU8 {
    self.sample_at(lut_idx)
  }
  /// Whether any LUT entry carries a fraction dithering can move.
  fn dither_active(&self) -> bool {
    false
  }
  /// Color at pixel `(x, y)`.
  fn sample_pixel(&self, x: u32, y: u32) -> PremultipliedColorU8;
  /// Color at pixel `(x, y)` with gradient dithering applied.
  fn sample_pixel_dithered(&self, x: u32, y: u32) -> PremultipliedColorU8 {
    self.sample_pixel(x, y)
  }
  /// Initializes row state at the given row start.
  fn begin_row(&self, src_x_start: u32, src_y: u32, lut_len: usize) -> Self::RowState;
  /// Returns an index in `0..lut_len` where `lut_len` is the value passed to `begin_row`.
  fn next_lut_index(&self, row_state: &mut Self::RowState) -> usize;
  /// Returns true when every LUT entry has alpha = 255; allows the overlay loop to skip blending.
  fn fully_opaque(&self) -> bool {
    false
  }

  /// Overlays the tile onto `data` with source-over blending, no clip.
  fn overlay_unconstrained(
    &self,
    data: &mut [u8],
    bottom_width: u32,
    bottom_height: u32,
    offset: Point<f32>,
  ) {
    let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
      compute_overlay_bounds_raw(
        bottom_width,
        bottom_height,
        offset,
        self.width(),
        self.height(),
      )
    else {
      return;
    };

    let lut_len = self.lut_len();
    if lut_len == 0 {
      return;
    }

    let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(data);
    let row_pixels = bottom_width as usize;
    let dest_x_min_usize = dest_x_min as usize;
    let dest_x_max_usize = dest_x_max as usize;
    let fully_opaque = self.fully_opaque();
    let dither = self.dither_active();
    let sample = |lut_idx: usize, src_x: u32, src_y: u32| {
      if dither {
        self.sample_dithered_at(lut_idx, src_x, src_y)
      } else {
        self.sample_at(lut_idx)
      }
    };

    for dest_y in dest_y_min..dest_y_max {
      let src_y = (dest_y - offset_y) as u32;
      let src_x_start = (dest_x_min - offset_x) as u32;
      let mut row_state = self.begin_row(src_x_start, src_y, lut_len);
      let row_start = dest_y as usize * row_pixels;
      let row = &mut pixels[row_start + dest_x_min_usize..row_start + dest_x_max_usize];

      if fully_opaque {
        for (i, dst) in row.iter_mut().enumerate() {
          let lut_idx = self.next_lut_index(&mut row_state);
          debug_assert!(lut_idx < lut_len);
          let pixel = sample(lut_idx, src_x_start + i as u32, src_y);
          *dst = [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()];
        }
      } else {
        const CHUNK: usize = 256;
        let mut buf = [[0u8; 4]; CHUNK];
        let mut remaining = row;
        let mut src_x = src_x_start;
        while !remaining.is_empty() {
          let n = remaining.len().min(CHUNK);
          let (chunk, rest) = remaining.split_at_mut(n);
          remaining = rest;
          for slot in buf.iter_mut().take(n) {
            let lut_idx = self.next_lut_index(&mut row_state);
            debug_assert!(lut_idx < lut_len);
            let pixel = sample(lut_idx, src_x, src_y);
            src_x += 1;
            *slot = [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()];
          }
          for (dst, &src) in chunk.iter_mut().zip(buf[..n].iter()) {
            let src_a = src[3];
            let inv_src_a = (u8::MAX - src_a) as u32;
            dst[0] = src[0].saturating_add(fast_div_255(dst[0] as u32 * inv_src_a));
            dst[1] = src[1].saturating_add(fast_div_255(dst[1] as u32 * inv_src_a));
            dst[2] = src[2].saturating_add(fast_div_255(dst[2] as u32 * inv_src_a));
            dst[3] = src_a.saturating_add(fast_div_255(dst[3] as u32 * inv_src_a));
          }
        }
      }
    }
  }
}

#[inline(always)]
fn compute_overlay_bounds_raw(
  bottom_width: u32,
  bottom_height: u32,
  offset: Point<f32>,
  width: u32,
  height: u32,
) -> Option<(i32, i32, i32, i32, i32, i32)> {
  if width == 0 || height == 0 {
    return None;
  }

  let offset_x = offset.x.trunc() as i32;
  let offset_y = offset.y.trunc() as i32;
  let bottom_width = bottom_width as i32;
  let bottom_height = bottom_height as i32;
  let dest_y_min = offset_y.max(0);
  let dest_y_max = (offset_y + height as i32).min(bottom_height);
  if dest_y_min >= dest_y_max {
    return None;
  }

  let dest_x_min = offset_x.max(0);
  let dest_x_max = (offset_x + width as i32).min(bottom_width);
  if dest_x_min >= dest_x_max {
    return None;
  }

  Some((
    offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max,
  ))
}

#[inline(always)]
fn position_to_sample_index(position: f32, axis_length: f32, lut_size: usize) -> usize {
  if lut_size <= 1 || axis_length.abs() <= f32::EPSILON {
    return 0;
  }

  let max_index = lut_size - 1;
  ((position.clamp(0.0, axis_length) * max_index as f32 / axis_length).round() as usize)
    .min(max_index)
}

fn assign_stop_sample_indices(
  resolved_stops: &[ResolvedGradientStop],
  axis_length: f32,
  lut_size: usize,
) -> Vec<usize> {
  if resolved_stops.is_empty() || lut_size == 0 {
    return Vec::new();
  }

  let stop_count = resolved_stops.len();
  let max_index = lut_size - 1;
  let mut indices = vec![0usize; stop_count];
  let mut i = 0usize;

  while i < stop_count {
    let position = resolved_stops[i].position;
    let preferred = position_to_sample_index(position, axis_length, lut_size);
    let mut run_end = i + 1;
    while run_end < stop_count
      && (resolved_stops[run_end].position - position).abs() <= f32::EPSILON
    {
      run_end += 1;
    }

    let run_len = run_end - i;
    let run_start_index = preferred.saturating_sub(run_len.saturating_sub(1));
    for (offset, slot) in indices[i..run_end].iter_mut().enumerate() {
      let logical_index = run_start_index.saturating_add(offset).min(max_index);
      let stop_index = i + offset;
      let lower_bound = stop_index.min(max_index);
      let upper_bound = max_index.saturating_sub(stop_count - 1 - stop_index);
      *slot = if lower_bound <= upper_bound {
        logical_index.clamp(lower_bound, upper_bound)
      } else {
        // More stops than LUT slots: uniqueness is impossible; stay in range.
        logical_index.min(max_index)
      };
    }

    i = run_end;
  }

  for i in 1..stop_count {
    indices[i] = indices[i]
      .max(indices[i - 1].saturating_add(1))
      .min(max_index);
  }

  for i in (0..stop_count.saturating_sub(1)).rev() {
    indices[i] = indices[i].min(indices[i + 1].saturating_sub(1));
  }

  indices
}

#[inline(always)]
fn interpolation_position(left_position: f32, right_position: f32, sample_position: f32) -> f32 {
  let denominator = right_position - left_position;
  if denominator.abs() < f32::EPSILON {
    return 0.0;
  }

  ((sample_position - left_position) / denominator).clamp(0.0, 1.0)
}

/// Builds a gradient LUT in `T`, sampling stops along the axis and snapping the stop positions onto
/// exact entries.
fn build_lut<T: Copy>(
  resolved_stops: &[ResolvedGradientStop],
  axis_length: f32,
  lut_size: usize,
  interpolation: ColorInterpolationMethod,
  from_color: impl Fn(Color) -> T,
  interpolate_srgb: impl Fn(T, T, f32) -> T,
) -> Vec<T> {
  let color_space = interpolation.color_space;
  let hue_direction = interpolation.hue_direction;
  if lut_size == 0 {
    return Vec::new();
  }

  if resolved_stops.len() <= 1 {
    let color = resolved_stops
      .first()
      .map(|s| s.color)
      .unwrap_or(Color::transparent());

    return vec![from_color(color)];
  }

  let mut left_index = 0usize;
  let mut right_index = 1usize;
  let sample_step = if lut_size <= 1 {
    0.0
  } else {
    axis_length / (lut_size - 1) as f32
  };

  let mut write_sample = |sample_index: usize| -> T {
    let position_px = sample_index as f32 * sample_step;

    while right_index < resolved_stops.len() && resolved_stops[right_index].position <= position_px
    {
      left_index = right_index;
      right_index += 1;
    }

    if right_index >= resolved_stops.len() {
      return from_color(resolved_stops[left_index].color);
    }

    let left_stop = &resolved_stops[left_index];
    let right_stop = &resolved_stops[right_index];
    if left_stop.color == right_stop.color {
      return from_color(left_stop.color);
    }

    let t = interpolation_position(left_stop.position, right_stop.position, position_px);
    if color_space == ColorSpaceTag::Srgb && hue_direction == HueDirection::Shorter {
      return interpolate_srgb(from_color(left_stop.color), from_color(right_stop.color), t);
    }

    from_color(
      left_stop
        .color
        .interpolate(right_stop.color, t, color_space, hue_direction),
    )
  };

  let mut lut: Vec<T> = (0..lut_size).map(&mut write_sample).collect();
  let stop_indices = assign_stop_sample_indices(resolved_stops, axis_length, lut.len());
  for (stop, &sample_index) in resolved_stops.iter().zip(&stop_indices) {
    lut[sample_index] = from_color(stop.color);
  }

  lut
}

fn color_to_premultiplied_hi(color: Color) -> GradientLutHiEntry {
  let [r, g, b, a] = color.0;
  let scale = a as f32 / 255.0 * 256.0;

  [
    (r as f32 * scale).round() as u16,
    (g as f32 * scale).round() as u16,
    (b as f32 * scale).round() as u16,
    (a as u16) << 8,
  ]
}

fn interpolate_hi(
  left: GradientLutHiEntry,
  right: GradientLutHiEntry,
  t: f32,
) -> GradientLutHiEntry {
  let mut entry = [0u16; 4];
  for (slot, (l, r)) in entry.iter_mut().zip(left.iter().zip(right.iter())) {
    *slot = (*l as f32 * (1.0 - t) + *r as f32 * t).round() as u16;
  }
  entry
}

/// Precomputed gradient samples: 8-bit for plain reads and, when dithering, premultiplied 8.8 for
/// dithered ones.
#[derive(Debug, Clone, Default)]
pub struct ColorLut {
  colors: Vec<PremultipliedColorU8>,
  hi: Vec<GradientLutHiEntry>,
  dither_active: bool,
}

impl ColorLut {
  /// Samples `stops` along the axis into `size` entries, snapping stop positions onto exact
  /// entries.
  pub fn new(
    stops: &[ResolvedGradientStop],
    axis_length: f32,
    size: usize,
    interpolation: ColorInterpolationMethod,
    dither: bool,
  ) -> Self {
    let colors = build_lut(
      stops,
      axis_length,
      size,
      interpolation,
      Color::premultiplied,
      interpolate_rgba_premultiplied,
    );
    let hi = if dither {
      build_lut(
        stops,
        axis_length,
        size,
        interpolation,
        color_to_premultiplied_hi,
        interpolate_hi,
      )
    } else {
      Vec::new()
    };
    let dither_active = stops.len() > 1
      && hi
        .iter()
        .any(|entry| entry.iter().any(|channel| channel & 0xFF != 0));

    Self {
      colors,
      hi,
      dither_active,
    }
  }

  /// The 8-bit entries.
  pub fn colors(&self) -> &[PremultipliedColorU8] {
    &self.colors
  }

  /// Number of entries.
  pub fn len(&self) -> usize {
    self.colors.len()
  }

  /// Whether the table has no entries.
  pub fn is_empty(&self) -> bool {
    self.colors.is_empty()
  }

  /// Whether any entry carries a fraction dithering can move.
  pub fn dither_active(&self) -> bool {
    self.dither_active
  }

  /// The entry at `index`.
  #[inline(always)]
  pub fn sample(&self, index: usize) -> PremultipliedColorU8 {
    self.colors[index]
  }

  /// The entry at `index` quantized with the Bayer noise for pixel `(x, y)`.
  #[inline(always)]
  pub(crate) fn sample_dithered(&self, index: usize, x: u32, y: u32) -> PremultipliedColorU8 {
    let entry = self.hi[index];
    let bias = (128 + DITHER_NOISE_88[(y & 7) as usize][(x & 7) as usize] as i32) as u32;
    let channel = |value: u16| ((value as u32 + bias) >> 8) as u8;

    PremultipliedColorU8::from_rgba(
      channel(entry[0]),
      channel(entry[1]),
      channel(entry[2]),
      channel(entry[3]),
    )
    .unwrap_or(PremultipliedColorU8::TRANSPARENT)
  }
}

/// The stop run a LUT samples.
pub(crate) struct LutAxis {
  pub(crate) repeating: bool,
  /// First resolved stop position, the repeating origin.
  pub(crate) repeat_start: f32,
  pub(crate) repeat_period: f32,
  /// The length the LUT covers.
  pub(crate) length: f32,
  pub(crate) stops: SmallVec<[ResolvedGradientStop; 4]>,
}

impl LutAxis {
  pub(crate) fn new(
    repeating: bool,
    stops: SmallVec<[ResolvedGradientStop; 4]>,
    axis_length: f32,
  ) -> Self {
    if repeating && let (Some(first), Some(last)) = (stops.first(), stops.last()) {
      let repeat_start = first.position;
      let repeat_period = (last.position - first.position).max(0.0);
      if repeat_period > 1e-6 {
        let shifted = stops
          .iter()
          .map(|stop| ResolvedGradientStop {
            color: stop.color,
            position: stop.position - repeat_start,
          })
          .collect();
        return Self {
          repeating: true,
          repeat_start,
          repeat_period,
          length: repeat_period,
          stops: shifted,
        };
      }
    }

    Self {
      repeating: false,
      repeat_start: 0.0,
      repeat_period: 0.0,
      length: axis_length,
      stops,
    }
  }

  /// A LUT size with one entry per pixel of the axis.
  pub(crate) fn lut_size(&self) -> usize {
    self.lut_size_covering(
      (self.length.ceil() as usize)
        .saturating_add(1)
        .max(MIN_GRADIENT_LUT_SIZE),
    )
  }

  /// A LUT size that covers `visible_samples` and the tightest stop interval.
  pub(crate) fn lut_size_covering(&self, visible_samples: usize) -> usize {
    let visible_samples = visible_samples.max(MIN_GRADIENT_LUT_SIZE);

    let min_interval = self
      .stops
      .windows(2)
      .map(|stops| stops[1].position - stops[0].position)
      .filter(|interval| *interval > f32::EPSILON)
      .fold(f32::INFINITY, f32::min);

    let segment_aware_size = if min_interval.is_finite() {
      ((self.length / min_interval).ceil() as usize)
        .saturating_add(self.stops.len())
        .saturating_add(1)
        .max(MIN_GRADIENT_LUT_SIZE)
    } else {
      self
        .stops
        .len()
        .saturating_add(1)
        .max(MIN_GRADIENT_LUT_SIZE)
    };

    let size = visible_samples
      .max(segment_aware_size)
      .max(self.stops.len().saturating_mul(2))
      .max(MIN_GRADIENT_LUT_SIZE);
    size.min(MAX_GRADIENT_LUT_SIZE)
  }

  pub(crate) fn lut(
    &self,
    size: usize,
    interpolation: ColorInterpolationMethod,
    dither: bool,
  ) -> ColorLut {
    ColorLut::new(&self.stops, self.length, size, interpolation, dither)
  }
}

impl ResolvedGradientStop {
  /// Resolves stop positions to pixels along the axis, filling unspecified ones.
  pub fn resolve(
    stops: &[GradientStop],
    axis_size_px: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) -> SmallVec<[Self; 4]> {
    let mut resolved: SmallVec<[ResolvedGradientStop; 4]> = SmallVec::new();
    let mut last_position = 0.0;

    for (i, step) in stops.iter().enumerate() {
      match step {
        GradientStop::ColorHint {
          color,
          hint: Some(hint),
        } => {
          let position = hint.0.to_px(sizing, axis_size_px).max(last_position);

          last_position = position;

          resolved.push(ResolvedGradientStop {
            color: color.resolve(current_color),
            position,
          });
        }
        GradientStop::ColorHint { color, hint: None } => {
          resolved.push(ResolvedGradientStop {
            color: color.resolve(current_color),
            position: UNDEFINED_POSITION,
          });
        }
        GradientStop::Hint(hint) => {
          let Some(before) = resolved.last() else {
            continue;
          };

          let Some(after_color) = stops.get(i + 1).and_then(|stop| match stop {
            GradientStop::ColorHint { color, hint: _ } => Some(color.resolve(current_color)),
            GradientStop::Hint(_) => None,
          }) else {
            continue;
          };

          let interpolated_color = before.color.lerp_premultiplied(after_color, 0.5);

          let position = hint.0.to_px(sizing, axis_size_px).max(last_position);

          resolved.push(ResolvedGradientStop {
            color: interpolated_color,
            position,
          });

          last_position = position;
        }
      }
    }

    // If there are no color stops, return an empty vector
    if resolved.is_empty() {
      return resolved;
    }

    // if there is only one stop, treat it as pure color image
    if resolved.len() == 1 {
      if let Some(first_stop) = resolved.first_mut() {
        first_stop.position = axis_size_px;
      }

      return resolved;
    }

    if let Some(first_stop) = resolved.first_mut()
      && first_stop.position == UNDEFINED_POSITION
    {
      first_stop.position = 0.0;
    }

    if let Some(last_stop) = resolved.last_mut()
      && last_stop.position == UNDEFINED_POSITION
    {
      last_stop.position = axis_size_px;
    }

    // Distribute unspecified or non-increasing positions in pixel domain
    let mut i = 1usize;
    while i < resolved.len() - 1 {
      // if the position is defined and valid, skip it
      if resolved[i].position != UNDEFINED_POSITION {
        i += 1;
        continue;
      }

      let last_defined_position = resolved.get(i - 1).map(|s| s.position).unwrap_or(0.0);

      // try to find next defined position
      let next_index = resolved
        .iter()
        .skip(i + 1)
        .position(|s| s.position != UNDEFINED_POSITION)
        .map(|idx| i + 1 + idx)
        .unwrap_or(resolved.len() - 1);

      let next_position = resolved[next_index].position;

      // number of segments between last defined and next position
      let segments_count = (next_index - i + 1) as f32;
      let step_for_each_segment = (next_position - last_defined_position) / segments_count;

      // distribute the step evenly between the stops
      for j in i..next_index {
        let offset = (j - i + 1) as f32;
        resolved[j].position = last_defined_position + step_for_each_segment * offset;
      }

      i = next_index + 1;
    }

    resolved
  }
}

const UNDEFINED_POSITION: f32 = -1.0;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    style::{Color, Length, StopPosition},
    viewport::Viewport,
  };

  #[test]
  fn test_resolve_stops_along_axis() {
    let stops = vec![
      GradientStop::ColorHint {
        color: Color([255, 0, 0, 255]).into(),
        hint: Some(StopPosition(Length::Px(10.0))),
      },
      GradientStop::ColorHint {
        color: Color([0, 255, 0, 255]).into(),
        hint: Some(StopPosition(Length::Px(20.0))),
      },
      GradientStop::ColorHint {
        color: Color([0, 0, 255, 255]).into(),
        hint: Some(StopPosition(Length::Percentage(30.0))),
      },
    ];

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((40, 40)))
      .build();

    let width = sizing.viewport.size.width;

    assert!(width.is_some());

    let resolved = ResolvedGradientStop::resolve(
      &stops,
      width.unwrap_or_default() as f32,
      &sizing,
      Color::black(),
    );

    assert_eq!(
      resolved[0],
      ResolvedGradientStop {
        color: Color([255, 0, 0, 255]),
        position: 10.0,
      },
    );

    assert_eq!(
      resolved[1],
      ResolvedGradientStop {
        color: Color([0, 255, 0, 255]),
        position: 20.0,
      },
    );

    assert_eq!(
      resolved[2],
      ResolvedGradientStop {
        color: Color([0, 0, 255, 255]),
        position: 20.0,
      },
    );
  }

  #[test]
  fn test_distribute_evenly_between_positions() {
    let stops = vec![
      GradientStop::ColorHint {
        color: Color([255, 0, 0, 255]).into(),
        hint: None,
      },
      GradientStop::ColorHint {
        color: Color([0, 255, 0, 255]).into(),
        hint: None,
      },
      GradientStop::ColorHint {
        color: Color([0, 0, 255, 255]).into(),
        hint: None,
      },
    ];

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((40, 40)))
      .build();

    let resolved = ResolvedGradientStop::resolve(
      &stops,
      sizing.viewport.size.width.unwrap_or_default() as f32,
      &sizing,
      Color::black(),
    );

    assert_eq!(
      resolved.as_slice(),
      &[
        ResolvedGradientStop {
          color: Color([255, 0, 0, 255]),
          position: 0.0,
        },
        ResolvedGradientStop {
          color: Color([0, 255, 0, 255]),
          position: sizing.viewport.size.width.unwrap_or_default() as f32 / 2.0,
        },
        ResolvedGradientStop {
          color: Color([0, 0, 255, 255]),
          position: sizing.viewport.size.width.unwrap_or_default() as f32,
        },
      ]
    );
  }

  #[test]
  fn test_hint_only() {
    let stops = vec![
      GradientStop::ColorHint {
        color: Color([255, 0, 0, 255]).into(),
        hint: None,
      },
      GradientStop::Hint(StopPosition(Length::Percentage(10.0))),
      GradientStop::ColorHint {
        color: Color([0, 0, 255, 255]).into(),
        hint: None,
      },
    ];

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((40, 40)))
      .build();

    let resolved = ResolvedGradientStop::resolve(
      &stops,
      sizing.viewport.size.width.unwrap_or_default() as f32,
      &sizing,
      Color::black(),
    );

    assert_eq!(
      resolved[0],
      ResolvedGradientStop {
        color: Color([255, 0, 0, 255]),
        position: 0.0,
      },
    );

    // the mid color between red and blue should be at 10%
    assert_eq!(
      resolved[1],
      ResolvedGradientStop {
        color: Color([255, 0, 0, 255]).lerp_premultiplied(Color([0, 0, 255, 255]), 0.5),
        position: sizing.viewport.size.width.unwrap_or_default() as f32 * 0.1,
      },
    );

    assert_eq!(
      resolved[2],
      ResolvedGradientStop {
        color: Color([0, 0, 255, 255]),
        position: sizing.viewport.size.width.unwrap_or_default() as f32,
      },
    );
  }

  #[test]
  fn test_adaptive_lut_size_grows_for_tight_stop_clusters() {
    let resolved = [
      ResolvedGradientStop {
        color: Color([255, 0, 0, 255]),
        position: 0.0,
      },
      ResolvedGradientStop {
        color: Color([0, 255, 0, 255]),
        position: 0.25,
      },
      ResolvedGradientStop {
        color: Color([0, 0, 255, 255]),
        position: 256.0,
      },
    ];

    let size = LutAxis::new(false, resolved.iter().cloned().collect(), 256.0).lut_size();

    assert!(size > 1025);
    assert!(size <= MAX_GRADIENT_LUT_SIZE);
  }

  #[test]
  fn test_build_color_lut_preserves_hard_stop_transition() {
    let resolved = [
      ResolvedGradientStop {
        color: Color([255, 0, 0, 255]),
        position: 0.0,
      },
      ResolvedGradientStop {
        color: Color([255, 0, 0, 255]),
        position: 8.0,
      },
      ResolvedGradientStop {
        color: Color([0, 0, 255, 255]),
        position: 8.0,
      },
      ResolvedGradientStop {
        color: Color([0, 0, 255, 255]),
        position: 16.0,
      },
    ];

    let lut = ColorLut::new(
      &resolved,
      16.0,
      17,
      ColorInterpolationMethod {
        color_space: ColorSpaceTag::Srgb,
        hue_direction: HueDirection::Shorter,
      },
      false,
    )
    .colors()
    .to_vec();

    assert_eq!(lut[7], Color::premultiplied(Color([255, 0, 0, 255])));
    assert_eq!(lut[8], Color::premultiplied(Color([0, 0, 255, 255])));
  }

  #[test]
  fn test_build_color_lut_gives_distinct_samples_to_narrow_interval() {
    let resolved = [
      ResolvedGradientStop {
        color: Color([255, 0, 0, 255]),
        position: 0.0,
      },
      ResolvedGradientStop {
        color: Color([0, 255, 0, 255]),
        position: 0.05,
      },
      ResolvedGradientStop {
        color: Color([0, 0, 255, 255]),
        position: 32.0,
      },
    ];

    let lut_size = LutAxis::new(false, resolved.iter().cloned().collect(), 32.0).lut_size();
    let lut = ColorLut::new(
      &resolved,
      32.0,
      lut_size,
      ColorInterpolationMethod {
        color_space: ColorSpaceTag::Srgb,
        hue_direction: HueDirection::Shorter,
      },
      false,
    )
    .colors()
    .to_vec();
    let stop_indices = assign_stop_sample_indices(&resolved, 32.0, lut.len());

    assert!(stop_indices[0] < stop_indices[1]);
    assert_eq!(
      lut[stop_indices[0]],
      Color::premultiplied(resolved[0].color)
    );
    assert_eq!(
      lut[stop_indices[1]],
      Color::premultiplied(resolved[1].color)
    );
  }

  #[test]
  fn test_build_color_lut_remains_monotonic_for_even_spacing() {
    let resolved = [
      ResolvedGradientStop {
        color: Color([0, 0, 0, 255]),
        position: 0.0,
      },
      ResolvedGradientStop {
        color: Color([255, 255, 255, 255]),
        position: 10.0,
      },
    ];

    let lut = ColorLut::new(
      &resolved,
      10.0,
      33,
      ColorInterpolationMethod {
        color_space: ColorSpaceTag::Srgb,
        hue_direction: HueDirection::Shorter,
      },
      false,
    )
    .colors()
    .to_vec();

    for pair in lut.windows(2) {
      assert!(pair[0].red() <= pair[1].red());
      assert!(pair[0].green() <= pair[1].green());
      assert!(pair[0].blue() <= pair[1].blue());
      assert_eq!(pair[0].alpha(), 255);
      assert_eq!(pair[1].alpha(), 255);
    }
  }

  #[test]
  fn test_interpolate_rgba_uses_premultiplied_alpha() {
    let mixed = Color([255, 255, 255, 255]).lerp_premultiplied(Color([0, 0, 0, 0]), 0.5);
    assert_eq!(mixed, Color([255, 255, 255, 128]));
  }

  fn evenly_spaced_stops(count: usize, axis_length: f32) -> Vec<ResolvedGradientStop> {
    (0..count)
      .map(|i| ResolvedGradientStop {
        color: Color([255, 0, 0, 255]),
        position: axis_length * i as f32 / (count - 1) as f32,
      })
      .collect()
  }

  #[test]
  fn assign_stop_sample_indices_survives_more_stops_than_lut() {
    let stops = evenly_spaced_stops(9000, 256.0);
    let indices = assign_stop_sample_indices(&stops, 256.0, 8193);

    assert_eq!(indices.len(), 9000);
    assert!(indices.iter().all(|&idx| idx < 8193));
  }

  #[test]
  fn stop_snapping_survives_more_stops_than_lut() {
    let stops = evenly_spaced_stops(9000, 256.0);
    let lut = ColorLut::new(
      &stops,
      256.0,
      8193,
      ColorInterpolationMethod::default(),
      false,
    )
    .colors()
    .to_vec();

    assert_eq!(lut.len(), 8193);
  }

  #[test]
  fn assign_stop_sample_indices_in_range_is_strictly_increasing() {
    let stops = evenly_spaced_stops(5, 512.0);
    let indices = assign_stop_sample_indices(&stops, 512.0, 512);

    assert!(indices.iter().all(|&idx| idx < 512));
    for pair in indices.windows(2) {
      assert!(pair[0] < pair[1]);
    }
  }
}
