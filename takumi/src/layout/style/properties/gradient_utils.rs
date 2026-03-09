use color::{AlphaColor, ColorSpaceTag, DynamicColor, HueDirection, Srgb};
use image::{Rgba, RgbaImage};
use smallvec::SmallVec;
use taffy::Point;
use wide::f32x4;

use super::{Color, GradientStop, ResolvedGradientStop};
use crate::rendering::{RenderContext, blend_pixel};

const MIN_GRADIENT_LUT_SIZE: usize = 1024;
const MAX_GRADIENT_LUT_SIZE: usize = 8193;

/// Interpolates between two colors in RGBA space, if t is 0.0 or 1.0, returns the first or second color.
/// Uses SIMD to process all 4 color channels in parallel.
pub(crate) fn interpolate_rgba(c1: Color, c2: Color, t: f32) -> Color {
  let result_f32 = interpolate_rgba_impl(c1, c2, t);
  let result = result_f32.to_array();
  Color([
    result[0].round() as u8,
    result[1].round() as u8,
    result[2].round() as u8,
    result[3].round() as u8,
  ])
}

/// Interpolates between two colors in RGBA space, if t is 0.0 or 1.0, returns the first or second color as f32x4.
fn interpolate_rgba_impl(c1: Color, c2: Color, t: f32) -> f32x4 {
  let c1_f32 = f32x4::from([
    c1.0[0] as f32,
    c1.0[1] as f32,
    c1.0[2] as f32,
    c1.0[3] as f32,
  ]);

  if t <= f32::EPSILON {
    return c1_f32;
  }

  let c2_f32 = f32x4::from([
    c2.0[0] as f32,
    c2.0[1] as f32,
    c2.0[2] as f32,
    c2.0[3] as f32,
  ]);

  if t >= 1.0 - f32::EPSILON {
    return c2_f32;
  }

  c1_f32 * (1.0 - t) + c2_f32 * t
}

pub(crate) fn interpolate_with_color_space(
  c1: Color,
  c2: Color,
  t: f32,
  color_space: ColorSpaceTag,
  hue_direction: HueDirection,
) -> f32x4 {
  if color_space == ColorSpaceTag::Srgb && hue_direction == HueDirection::Shorter {
    return interpolate_rgba_impl(c1, c2, t);
  }

  if t <= f32::EPSILON {
    return f32x4::from([
      c1.0[0] as f32,
      c1.0[1] as f32,
      c1.0[2] as f32,
      c1.0[3] as f32,
    ]);
  }

  if t >= 1.0 - f32::EPSILON {
    return f32x4::from([
      c2.0[0] as f32,
      c2.0[1] as f32,
      c2.0[2] as f32,
      c2.0[3] as f32,
    ]);
  }

  let dynamic_1 =
    DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from(color::Rgba8::from_u8_array(c1.0)));
  let dynamic_2 =
    DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from(color::Rgba8::from_u8_array(c2.0)));

  let mixed = dynamic_1
    .interpolate(dynamic_2, color_space, hue_direction)
    .eval(t);
  let rgba = mixed.to_alpha_color::<Srgb>().to_rgba8().to_u8_array();

  f32x4::from([
    rgba[0] as f32,
    rgba[1] as f32,
    rgba[2] as f32,
    rgba[3] as f32,
  ])
}

impl From<Color> for [f32; 4] {
  fn from(color: Color) -> Self {
    [
      color.0[0] as f32,
      color.0[1] as f32,
      color.0[2] as f32,
      color.0[3] as f32,
    ]
  }
}

pub(crate) trait GradientOverlayTile {
  type RowState;

  fn width(&self) -> u32;
  fn height(&self) -> u32;
  fn lut_samples(&self) -> &[[f32; 4]];
  fn begin_row(&self, src_x_start: u32, src_y: u32, lut_len: usize) -> Self::RowState;
  /// Returns an index in `0..lut_len` where `lut_len` is the value passed to `begin_row`.
  fn next_lut_index(&self, row_state: &mut Self::RowState) -> usize;
}

#[inline(always)]
/// Computes destination/source bounds using floored pixel offsets from `offset`.
pub(crate) fn compute_overlay_bounds(
  bottom: &RgbaImage,
  offset: Point<f32>,
  width: u32,
  height: u32,
) -> Option<(i32, i32, i32, i32, i32, i32)> {
  if width == 0 || height == 0 {
    return None;
  }

  let offset_x = offset.x.trunc() as i32;
  let offset_y = offset.y.trunc() as i32;
  let bottom_width = bottom.width() as i32;
  let bottom_height = bottom.height() as i32;
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

pub(crate) fn overlay_gradient_tile_fast_normal_unconstrained<T: GradientOverlayTile>(
  bottom: &mut RgbaImage,
  tile: &T,
  offset: Point<f32>,
) {
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds(bottom, offset, tile.width(), tile.height())
  else {
    return;
  };

  let lut_samples = tile.lut_samples();
  if lut_samples.is_empty() {
    return;
  }
  let lut_len = lut_samples.len();

  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as u32;
    let src_x_start = (dest_x_min - offset_x) as u32;
    let mut row_state = tile.begin_row(src_x_start, src_y, lut_len);
    for dest_x in dest_x_min..dest_x_max {
      let lut_idx = tile.next_lut_index(&mut row_state);
      debug_assert!(lut_idx < lut_len);
      let pixel: Rgba<u8> = Color::from(lut_samples[lut_idx]).into();
      if pixel.0[3] != 0 {
        let current = bottom.get_pixel_mut(dest_x as u32, dest_y as u32);
        blend_pixel(current, pixel, super::BlendMode::Normal);
      }
    }
  }
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
      *slot = logical_index.clamp(lower_bound, upper_bound);
    }

    i = run_end;
  }

  for i in 1..stop_count {
    indices[i] = indices[i].max(indices[i - 1].saturating_add(1));
  }

  for i in (0..stop_count.saturating_sub(1)).rev() {
    indices[i] = indices[i].min(indices[i + 1].saturating_sub(1));
  }

  indices
}

fn snap_stop_samples(
  typed_lut: &mut [[f32; 4]],
  resolved_stops: &[ResolvedGradientStop],
  axis_length: f32,
) {
  if typed_lut.is_empty() || resolved_stops.is_empty() {
    return;
  }

  let stop_indices = assign_stop_sample_indices(resolved_stops, axis_length, typed_lut.len());
  for (stop, &sample_index) in resolved_stops.iter().zip(&stop_indices) {
    typed_lut[sample_index] = stop.color.into();
  }
}

/// Builds a pre-computed high-precision color lookup table for a gradient.
/// This allows O(1) color sampling instead of O(n) search + interpolation per pixel.
pub(crate) fn build_color_lut_with_interpolation(
  resolved_stops: &[ResolvedGradientStop],
  axis_length: f32,
  lut_size: usize,
  color_space: ColorSpaceTag,
  hue_direction: HueDirection,
) -> Vec<[f32; 4]> {
  if lut_size == 0 {
    return Vec::new();
  }

  // Fast path: if only one color, fill just 16 bytes
  if resolved_stops.len() <= 1 {
    let color = resolved_stops
      .first()
      .map(|s| s.color)
      .unwrap_or(crate::layout::style::Color::transparent());

    return vec![color.into()];
  }

  let mut left_index = 0usize;
  let mut right_index = 1usize;
  let sample_step = if lut_size <= 1 {
    0.0
  } else {
    axis_length / (lut_size - 1) as f32
  };

  let mut write_sample = |sample_index: usize| -> [f32; 4] {
    let position_px = sample_index as f32 * sample_step;

    while right_index < resolved_stops.len() && resolved_stops[right_index].position <= position_px
    {
      left_index = right_index;
      right_index += 1;
    }

    let color = if right_index >= resolved_stops.len() {
      f32x4::from(<[f32; 4]>::from(resolved_stops[left_index].color))
    } else {
      let left_stop = &resolved_stops[left_index];
      let right_stop = &resolved_stops[right_index];
      let denominator = right_stop.position - left_stop.position;
      let interpolation_position = if denominator.abs() < f32::EPSILON {
        0.0
      } else {
        ((position_px - left_stop.position) / denominator).clamp(0.0, 1.0)
      };

      interpolate_with_color_space(
        left_stop.color,
        right_stop.color,
        interpolation_position,
        color_space,
        hue_direction,
      )
    };

    color.to_array()
  };

  let mut typed_lut = vec![[0.0; 4]; lut_size];
  for (sample_index, chunk) in typed_lut.iter_mut().enumerate() {
    *chunk = write_sample(sample_index);
  }
  snap_stop_samples(&mut typed_lut, resolved_stops, axis_length);
  typed_lut
}

/// Calculates an adaptive LUT size based on the gradient axis length.
pub(crate) fn adaptive_lut_size(
  axis_length: f32,
  resolved_stops: &[ResolvedGradientStop],
) -> usize {
  let base_size = (axis_length.ceil() as usize)
    .max(1)
    .next_power_of_two()
    .max(MIN_GRADIENT_LUT_SIZE);

  let min_interval = resolved_stops
    .windows(2)
    .map(|stops| stops[1].position - stops[0].position)
    .filter(|interval| *interval > f32::EPSILON)
    .fold(f32::INFINITY, f32::min);

  let segment_aware_size = if min_interval.is_finite() {
    let target_samples = ((axis_length / min_interval).ceil() as usize)
      .saturating_add(resolved_stops.len())
      .max(2);
    target_samples.next_power_of_two()
  } else {
    resolved_stops
      .len()
      .saturating_mul(2)
      .max(2)
      .next_power_of_two()
  };

  let size = base_size
    .max(segment_aware_size)
    .max(resolved_stops.len().saturating_mul(2))
    .max(2);
  (size + 1).min(MAX_GRADIENT_LUT_SIZE)
}

const UNDEFINED_POSITION: f32 = -1.0;

pub(crate) fn resolve_stops_along_axis(
  stops: &[GradientStop],
  axis_size_px: f32,
  context: &RenderContext,
) -> SmallVec<[ResolvedGradientStop; 4]> {
  let mut resolved: SmallVec<[ResolvedGradientStop; 4]> = SmallVec::new();
  let mut last_position = 0.0;

  for (i, step) in stops.iter().enumerate() {
    match step {
      GradientStop::ColorHint {
        color,
        hint: Some(hint),
      } => {
        let position = hint
          .0
          .to_px(&context.sizing, axis_size_px)
          .max(last_position);

        last_position = position;

        resolved.push(ResolvedGradientStop {
          color: color.resolve(context.current_color),
          position,
        });
      }
      GradientStop::ColorHint { color, hint: None } => {
        resolved.push(ResolvedGradientStop {
          color: color.resolve(context.current_color),
          position: UNDEFINED_POSITION,
        });
      }
      GradientStop::Hint(hint) => {
        let Some(before) = resolved.last() else {
          continue;
        };

        let Some(after_color) = stops.get(i + 1).and_then(|stop| match stop {
          GradientStop::ColorHint { color, hint: _ } => Some(color.resolve(context.current_color)),
          GradientStop::Hint(_) => None,
        }) else {
          continue;
        };

        let interpolated_color = interpolate_rgba(before.color, after_color, 0.5);

        let position = hint
          .0
          .to_px(&context.sizing, axis_size_px)
          .max(last_position);

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

#[cfg(test)]
mod tests {
  use image::{Rgba, RgbaImage};
  use taffy::Point;

  use crate::rendering::blend_pixel;
  use crate::{
    GlobalContext,
    layout::style::{BlendMode, Length, StopPosition},
  };

  use super::*;

  #[derive(Debug, Clone, Copy)]
  struct MockTile {
    width: u32,
    height: u32,
  }

  #[derive(Debug, Clone, Copy)]
  struct MockRowState {
    value: usize,
    lut_len: usize,
  }

  impl GradientOverlayTile for MockTile {
    type RowState = MockRowState;

    fn width(&self) -> u32 {
      self.width
    }

    fn height(&self) -> u32 {
      self.height
    }

    fn lut_samples(&self) -> &[[f32; 4]] {
      static LUT: [[f32; 4]; 2] = [[255.0, 0.0, 0.0, 255.0], [0.0, 0.0, 255.0, 255.0]];
      &LUT
    }

    fn begin_row(&self, src_x_start: u32, src_y: u32, lut_len: usize) -> Self::RowState {
      MockRowState {
        value: ((src_x_start + src_y) as usize) % lut_len.max(1),
        lut_len,
      }
    }

    fn next_lut_index(&self, row_state: &mut Self::RowState) -> usize {
      let value = row_state.value;
      row_state.value = (row_state.value + 1) % row_state.lut_len.max(1);
      value
    }
  }

  fn overlay_reference(bottom: &mut RgbaImage, tile: &MockTile, offset: Point<f32>) {
    let offset_x = offset.x as i32;
    let offset_y = offset.y as i32;
    let dest_x_min = offset_x.max(0);
    let dest_x_max = (offset_x + tile.width as i32).min(bottom.width() as i32);
    let dest_y_min = offset_y.max(0);
    let dest_y_max = (offset_y + tile.height as i32).min(bottom.height() as i32);
    let lut_samples = tile.lut_samples();

    for dest_y in dest_y_min..dest_y_max {
      let src_y = (dest_y - offset_y) as u32;
      let src_x_start = (dest_x_min - offset_x) as u32;
      let mut row_state = tile.begin_row(src_x_start, src_y, lut_samples.len());

      for dest_x in dest_x_min..dest_x_max {
        let lut_idx = tile.next_lut_index(&mut row_state);
        let pixel: Rgba<u8> = Color::from(lut_samples[lut_idx]).into();
        let current = bottom.get_pixel_mut(dest_x as u32, dest_y as u32);
        blend_pixel(current, pixel, BlendMode::Normal);
      }
    }
  }

  #[test]
  fn test_overlay_gradient_tile_fast_matches_reference() {
    let tile = MockTile {
      width: 4,
      height: 3,
    };
    let offset = Point { x: 2.0, y: 1.0 };
    let mut actual = RgbaImage::from_pixel(10, 7, Rgba([20, 30, 40, 255]));
    let mut expected = actual.clone();

    overlay_gradient_tile_fast_normal_unconstrained(&mut actual, &tile, offset);
    overlay_reference(&mut expected, &tile, offset);

    assert_eq!(actual, expected);
  }

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

    let context = GlobalContext::default();
    let render_context = RenderContext::new_test(&context, (40, 40).into());

    let width = render_context.sizing.viewport.width;

    assert!(width.is_some());

    let resolved =
      resolve_stops_along_axis(&stops, width.unwrap_or_default() as f32, &render_context);

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
        position: 20.0, // since 30% (12px) is smaller than the last
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

    let context = GlobalContext::default();
    let render_context = RenderContext::new_test(&context, (40, 40).into());

    let resolved = resolve_stops_along_axis(
      &stops,
      render_context.sizing.viewport.width.unwrap_or_default() as f32,
      &render_context,
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
          position: render_context.sizing.viewport.width.unwrap_or_default() as f32 / 2.0,
        },
        ResolvedGradientStop {
          color: Color([0, 0, 255, 255]),
          position: render_context.sizing.viewport.width.unwrap_or_default() as f32,
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

    let context = GlobalContext::default();
    let render_context = RenderContext::new_test(&context, (40, 40).into());

    let resolved = resolve_stops_along_axis(
      &stops,
      render_context.sizing.viewport.width.unwrap_or_default() as f32,
      &render_context,
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
        color: interpolate_rgba(Color([255, 0, 0, 255]), Color([0, 0, 255, 255]), 0.5),
        position: render_context.sizing.viewport.width.unwrap_or_default() as f32 * 0.1,
      },
    );

    assert_eq!(
      resolved[2],
      ResolvedGradientStop {
        color: Color([0, 0, 255, 255]),
        position: render_context.sizing.viewport.width.unwrap_or_default() as f32,
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

    let size = adaptive_lut_size(256.0, &resolved);

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

    let lut = build_color_lut_with_interpolation(
      &resolved,
      16.0,
      17,
      ColorSpaceTag::Srgb,
      HueDirection::Shorter,
    );

    assert_eq!(Color::from(lut[7]), Color([255, 0, 0, 255]));
    assert_eq!(Color::from(lut[8]), Color([0, 0, 255, 255]));
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

    let lut_size = adaptive_lut_size(32.0, &resolved);
    let lut = build_color_lut_with_interpolation(
      &resolved,
      32.0,
      lut_size,
      ColorSpaceTag::Srgb,
      HueDirection::Shorter,
    );
    let stop_indices = assign_stop_sample_indices(&resolved, 32.0, lut.len());

    assert!(stop_indices[0] < stop_indices[1]);
    assert_eq!(Color::from(lut[stop_indices[0]]), resolved[0].color);
    assert_eq!(Color::from(lut[stop_indices[1]]), resolved[1].color);
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

    let lut = build_color_lut_with_interpolation(
      &resolved,
      10.0,
      33,
      ColorSpaceTag::Srgb,
      HueDirection::Shorter,
    );

    for pair in lut.windows(2) {
      assert!(pair[0][0] <= pair[1][0]);
      assert!(pair[0][1] <= pair[1][1]);
      assert!(pair[0][2] <= pair[1][2]);
      assert_eq!(pair[0][3], 255.0);
      assert_eq!(pair[1][3], 255.0);
    }
  }
}
