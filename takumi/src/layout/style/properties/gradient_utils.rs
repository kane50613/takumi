use color::{AlphaColor, ColorSpaceTag, DynamicColor, HueDirection, Srgb};
use smallvec::SmallVec;
use wide::f32x4;

use super::{Color, GradientStop, ResolvedGradientStop};
use crate::rendering::RenderContext;

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

fn interpolate_with_color_space(
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

pub(crate) fn color_from_stops_with_interpolation(
  position: f32,
  resolved_stops: &[ResolvedGradientStop],
  color_space: ColorSpaceTag,
  hue_direction: HueDirection,
) -> f32x4 {
  // Find the two stops that bracket the current position.
  // We want the last stop with position <= current position.
  let left_index = resolved_stops
    .iter()
    .rposition(|stop| stop.position <= position)
    .unwrap_or(0);

  let right_index = resolved_stops
    .iter()
    .enumerate()
    .position(|(i, stop)| i > left_index && stop.position >= position)
    .unwrap_or(resolved_stops.len() - 1);

  if left_index == right_index {
    // if the left and right indices are the same, we should return a hard stop
    let color = resolved_stops[left_index].color;
    f32x4::from([
      color.0[0] as f32,
      color.0[1] as f32,
      color.0[2] as f32,
      color.0[3] as f32,
    ])
  } else {
    let left_stop = &resolved_stops[left_index];
    let right_stop = &resolved_stops[right_index];

    let denom = right_stop.position - left_stop.position;
    let interpolation_position = if denom.abs() < f32::EPSILON {
      0.0
    } else {
      ((position - left_stop.position) / denom).clamp(0.0, 1.0)
    };

    interpolate_with_color_space(
      left_stop.color,
      right_stop.color,
      interpolation_position,
      color_space,
      hue_direction,
    )
  }
}

pub(crate) const BAYER_MATRIX_8X8: [[f32; 8]; 8] = [
  [
    -0.5, 0.0, -0.375, 0.125, -0.46875, 0.03125, -0.34375, 0.15625,
  ],
  [
    0.25, -0.25, 0.375, -0.125, 0.28125, -0.21875, 0.40625, -0.09375,
  ],
  [
    -0.3125, 0.1875, -0.4375, 0.0625, -0.28125, 0.21875, -0.40625, 0.09375,
  ],
  [
    0.4375, -0.0625, 0.3125, -0.1875, 0.46875, -0.03125, 0.34375, -0.15625,
  ],
  [
    -0.453125, 0.046875, -0.328125, 0.171875, -0.484375, 0.015625, -0.359375, 0.140625,
  ],
  [
    0.296875, -0.203125, 0.421875, -0.078125, 0.265625, -0.234375, 0.390625, -0.109375,
  ],
  [
    -0.265625, 0.234375, -0.390625, 0.109375, -0.296875, 0.203125, -0.421875, 0.078125,
  ],
  [
    0.484375, -0.015625, 0.359375, -0.140625, 0.453125, -0.046875, 0.328125, -0.171875,
  ],
];

/// Applies Bayer matrix dithering to a high-precision color and returns an 8-bit RGBA color.
#[inline(always)]
pub(crate) fn apply_dither(color: &[f32], x: u32, y: u32) -> [u8; 4] {
  let dither = BAYER_MATRIX_8X8[(y % 8) as usize][(x % 8) as usize];
  [
    (color[0] + dither).clamp(0.0, 255.0).round() as u8,
    (color[1] + dither).clamp(0.0, 255.0).round() as u8,
    (color[2] + dither).clamp(0.0, 255.0).round() as u8,
    (color[3] + dither).clamp(0.0, 255.0).round() as u8,
  ]
}

/// Builds a pre-computed high-precision color lookup table for a gradient.
/// This allows O(1) color sampling instead of O(n) search + interpolation per pixel.
pub(crate) fn build_color_lut_with_interpolation(
  resolved_stops: &[ResolvedGradientStop],
  axis_length: f32,
  lut_size: usize,
  buffer_pool: &mut crate::rendering::BufferPool,
  color_space: ColorSpaceTag,
  hue_direction: HueDirection,
) -> Vec<u8> {
  // Fast path: if only one color, fill just 16 bytes
  if resolved_stops.len() <= 1 {
    let color = resolved_stops
      .first()
      .map(|s| s.color)
      .unwrap_or(crate::layout::style::Color::transparent());

    let mut lut = buffer_pool.acquire_dirty(16);
    let c = [
      color.0[0] as f32,
      color.0[1] as f32,
      color.0[2] as f32,
      color.0[3] as f32,
    ];
    let f32_lut = bytemuck::cast_slice_mut::<u8, [f32; 4]>(&mut lut);
    f32_lut[0] = c;
    return lut;
  }

  let mut lut = buffer_pool.acquire_dirty(lut_size * 16);
  let f32_lut = bytemuck::cast_slice_mut::<u8, [f32; 4]>(&mut lut);
  for (i, chunk) in f32_lut.iter_mut().enumerate() {
    let t = i as f32 / (lut_size - 1) as f32;
    let position_px = t * axis_length;
    let color =
      color_from_stops_with_interpolation(position_px, resolved_stops, color_space, hue_direction);
    *chunk = color.to_array();
  }

  lut
}

/// Calculates an adaptive LUT size based on the gradient axis length.
pub(crate) fn adaptive_lut_size(axis_length: f32) -> usize {
  let size = (axis_length.ceil() as usize).next_power_of_two().max(1024);
  (size + 1).min(8193)
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
  use crate::{
    GlobalContext,
    layout::style::{Length, StopPosition},
  };

  use super::*;

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
}
