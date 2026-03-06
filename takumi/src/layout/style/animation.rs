#[cfg(feature = "css_stylesheet_parsing")]
use std::cmp::Ordering;

#[cfg(feature = "css_stylesheet_parsing")]
use crate::{
  layout::style::{
    Angle, AnimationDirection, AnimationFillMode, AnimationIterationCount, AnimationPlayState,
    AnimationTime, BackgroundPosition, BorderRadius, Color, ColorInput, Length, PercentageNumber,
    PositionComponent, ResolvedStyle, Sides, SpacePair, Style, apply_timing_function, direction_at,
    fill_mode_at, iteration_count_at, play_state_at,
    selector::{KeyframeRule, KeyframesRule, StyleSheet},
    time_at, timing_function_at,
  },
  rendering::{RenderContext, Sizing},
};

#[cfg(feature = "css_stylesheet_parsing")]
pub(crate) fn apply_stylesheet_animations(
  base_style: &ResolvedStyle,
  context: &RenderContext<'_>,
) -> ResolvedStyle {
  if base_style.animation_name.0.is_empty() {
    return base_style.clone();
  }

  let mut animated = base_style.clone();

  for (animation_index, animation_name) in base_style.animation_name.0.iter().enumerate() {
    let Some(keyframes) = find_keyframes(&context.stylesheets, animation_name) else {
      continue;
    };

    if play_state_at(&base_style.animation_play_state, animation_index)
      == AnimationPlayState::Paused
    {
      continue;
    }

    let duration = time_at(
      &base_style.animation_duration,
      animation_index,
      AnimationTime::from_milliseconds(0.0),
    );
    let delay = time_at(
      &base_style.animation_delay,
      animation_index,
      AnimationTime::from_milliseconds(0.0),
    );
    let iteration_count =
      iteration_count_at(&base_style.animation_iteration_count, animation_index);
    let direction = direction_at(&base_style.animation_direction, animation_index);
    let fill_mode = fill_mode_at(&base_style.animation_fill_mode, animation_index);
    let timing_function =
      timing_function_at(&base_style.animation_timing_function, animation_index);

    let Some(progress) = sample_animation_progress(
      context.time.time_ms as f32,
      duration.milliseconds,
      delay.milliseconds,
      iteration_count,
      direction,
      fill_mode,
    ) else {
      continue;
    };

    let Some((from_style, to_style, segment_progress)) =
      sample_keyframe_segment(keyframes, base_style, progress)
    else {
      continue;
    };

    let eased_progress = apply_timing_function(&timing_function, segment_progress);
    interpolate_styles(
      &mut animated,
      &from_style,
      &to_style,
      eased_progress,
      &context.sizing,
      context.current_color,
    );
  }

  animated
}

#[cfg(not(feature = "css_stylesheet_parsing"))]
pub(crate) fn apply_stylesheet_animations(
  base_style: &ResolvedStyle,
  _context: &crate::rendering::RenderContext<'_>,
) -> ResolvedStyle {
  base_style.clone()
}

#[cfg(feature = "css_stylesheet_parsing")]
fn find_keyframes<'a>(stylesheets: &'a [StyleSheet], name: &str) -> Option<&'a KeyframesRule> {
  stylesheets.iter().rev().find_map(|sheet| {
    sheet
      .keyframes
      .iter()
      .rev()
      .find(|rule| rule.name.eq_ignore_ascii_case(name))
  })
}

#[cfg(feature = "css_stylesheet_parsing")]
fn sample_animation_progress(
  time_ms: f32,
  duration_ms: f32,
  delay_ms: f32,
  iteration_count: AnimationIterationCount,
  direction: AnimationDirection,
  fill_mode: AnimationFillMode,
) -> Option<f32> {
  let active_time = time_ms - delay_ms;

  if duration_ms <= 0.0 {
    if active_time < 0.0 {
      return match fill_mode {
        AnimationFillMode::Backwards | AnimationFillMode::Both => Some(start_progress(direction)),
        _ => None,
      };
    }

    return Some(end_progress(direction, 0));
  }

  let total_active_duration = match iteration_count {
    AnimationIterationCount::Infinite => f32::INFINITY,
    AnimationIterationCount::Number(count) => duration_ms * count.max(0.0),
  };

  if active_time < 0.0 {
    return match fill_mode {
      AnimationFillMode::Backwards | AnimationFillMode::Both => Some(start_progress(direction)),
      _ => None,
    };
  }

  if active_time >= total_active_duration {
    return match fill_mode {
      AnimationFillMode::Forwards | AnimationFillMode::Both => {
        let iteration_index = match iteration_count {
          AnimationIterationCount::Infinite => 0,
          AnimationIterationCount::Number(count) => count.max(1.0).ceil() as usize - 1,
        };
        Some(end_progress(direction, iteration_index))
      }
      _ => None,
    };
  }

  let iteration_index = (active_time / duration_ms).floor() as usize;
  let mut progress = (active_time / duration_ms).fract();
  if progress == 0.0 && active_time > 0.0 {
    progress = 1.0;
  }

  Some(apply_direction(progress, direction, iteration_index))
}

#[cfg(feature = "css_stylesheet_parsing")]
fn start_progress(direction: AnimationDirection) -> f32 {
  apply_direction(0.0, direction, 0)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn end_progress(direction: AnimationDirection, iteration_index: usize) -> f32 {
  apply_direction(1.0, direction, iteration_index)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn apply_direction(progress: f32, direction: AnimationDirection, iteration_index: usize) -> f32 {
  match direction {
    AnimationDirection::Normal => progress,
    AnimationDirection::Reverse => 1.0 - progress,
    AnimationDirection::Alternate => {
      if iteration_index.is_multiple_of(2) {
        progress
      } else {
        1.0 - progress
      }
    }
    AnimationDirection::AlternateReverse => {
      if iteration_index.is_multiple_of(2) {
        1.0 - progress
      } else {
        progress
      }
    }
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
fn sample_keyframe_segment(
  keyframes: &KeyframesRule,
  base_style: &ResolvedStyle,
  progress: f32,
) -> Option<(ResolvedStyle, ResolvedStyle, f32)> {
  let resolved_frames = resolve_keyframes(keyframes, base_style);
  let first = resolved_frames.first()?;

  if progress <= first.0 {
    let segment_progress = if first.0 <= 0.0 {
      1.0
    } else {
      progress / first.0
    };
    return Some((
      base_style.clone(),
      first.1.clone(),
      segment_progress.clamp(0.0, 1.0),
    ));
  }

  for window in resolved_frames.windows(2) {
    let [(start_offset, start_style), (end_offset, end_style)] = window else {
      continue;
    };
    if progress <= *end_offset {
      let width = end_offset - start_offset;
      let segment_progress = if width <= f32::EPSILON {
        1.0
      } else {
        (progress - start_offset) / width
      };
      return Some((
        start_style.clone(),
        end_style.clone(),
        segment_progress.clamp(0.0, 1.0),
      ));
    }
  }

  let last = resolved_frames.last()?;
  let segment_progress = if last.0 >= 1.0 {
    1.0
  } else {
    (progress - last.0) / (1.0 - last.0)
  };
  Some((
    last.1.clone(),
    base_style.clone(),
    segment_progress.clamp(0.0, 1.0),
  ))
}

#[cfg(feature = "css_stylesheet_parsing")]
fn resolve_keyframes(
  keyframes: &KeyframesRule,
  base_style: &ResolvedStyle,
) -> Vec<(f32, ResolvedStyle)> {
  let mut frames = keyframes
    .keyframes
    .iter()
    .flat_map(|keyframe| {
      keyframe
        .offsets
        .iter()
        .copied()
        .map(|offset| (offset, resolve_keyframe_style(keyframe, base_style)))
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();

  frames.sort_by(|lhs, rhs| lhs.0.partial_cmp(&rhs.0).unwrap_or(Ordering::Equal));
  frames
}

#[cfg(feature = "css_stylesheet_parsing")]
fn resolve_keyframe_style(keyframe: &KeyframeRule, base_style: &ResolvedStyle) -> ResolvedStyle {
  let mut style = Style::default();
  for declaration in &keyframe.declarations {
    declaration.merge_into(&mut style);
  }
  style.inherit(base_style)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_styles(
  target: &mut ResolvedStyle,
  from: &ResolvedStyle,
  to: &ResolvedStyle,
  progress: f32,
  sizing: &Sizing,
  current_color: Color,
) {
  let discrete = progress >= 0.5;

  target.opacity = interpolate_percentage(from.opacity, to.opacity, progress);

  target.width = interpolate_length(from.width, to.width, progress).unwrap_or(if discrete {
    to.width
  } else {
    from.width
  });
  target.height = interpolate_length(from.height, to.height, progress).unwrap_or(if discrete {
    to.height
  } else {
    from.height
  });
  target.max_width =
    interpolate_length(from.max_width, to.max_width, progress).unwrap_or(if discrete {
      to.max_width
    } else {
      from.max_width
    });
  target.max_height =
    interpolate_length(from.max_height, to.max_height, progress).unwrap_or(if discrete {
      to.max_height
    } else {
      from.max_height
    });
  target.min_width =
    interpolate_length(from.min_width, to.min_width, progress).unwrap_or(if discrete {
      to.min_width
    } else {
      from.min_width
    });
  target.min_height =
    interpolate_length(from.min_height, to.min_height, progress).unwrap_or(if discrete {
      to.min_height
    } else {
      from.min_height
    });

  target.padding = interpolate_sides_length(from.padding, to.padding, progress);
  target.margin = interpolate_sides_length(from.margin, to.margin, progress);
  target.inset = interpolate_sides_length(from.inset, to.inset, progress);
  target.gap = interpolate_gap(from.gap, to.gap, progress);
  *target.border_radius =
    interpolate_border_radius(*from.border_radius, *to.border_radius, progress);

  target.padding_inline =
    interpolate_option_space_pair_length(from.padding_inline, to.padding_inline, progress);
  target.padding_block =
    interpolate_option_space_pair_length(from.padding_block, to.padding_block, progress);
  target.padding_top = interpolate_option_length(from.padding_top, to.padding_top, progress);
  target.padding_right = interpolate_option_length(from.padding_right, to.padding_right, progress);
  target.padding_bottom =
    interpolate_option_length(from.padding_bottom, to.padding_bottom, progress);
  target.padding_left = interpolate_option_length(from.padding_left, to.padding_left, progress);
  target.margin_inline =
    interpolate_option_space_pair_length(from.margin_inline, to.margin_inline, progress);
  target.margin_block =
    interpolate_option_space_pair_length(from.margin_block, to.margin_block, progress);
  target.margin_top = interpolate_option_length(from.margin_top, to.margin_top, progress);
  target.margin_right = interpolate_option_length(from.margin_right, to.margin_right, progress);
  target.margin_bottom = interpolate_option_length(from.margin_bottom, to.margin_bottom, progress);
  target.margin_left = interpolate_option_length(from.margin_left, to.margin_left, progress);
  target.inset_inline =
    interpolate_option_space_pair_auto_length(from.inset_inline, to.inset_inline, progress);
  target.inset_block =
    interpolate_option_space_pair_auto_length(from.inset_block, to.inset_block, progress);
  target.top = interpolate_option_length(from.top, to.top, progress);
  target.right = interpolate_option_length(from.right, to.right, progress);
  target.bottom = interpolate_option_length(from.bottom, to.bottom, progress);
  target.left = interpolate_option_length(from.left, to.left, progress);
  target.column_gap = interpolate_option_length(from.column_gap, to.column_gap, progress);
  target.row_gap = interpolate_option_length(from.row_gap, to.row_gap, progress);

  target.border_width =
    interpolate_option_sides_auto_length(from.border_width, to.border_width, progress);
  target.border_inline_width = interpolate_option_space_pair_auto_length(
    from.border_inline_width,
    to.border_inline_width,
    progress,
  );
  target.border_block_width = interpolate_option_space_pair_auto_length(
    from.border_block_width,
    to.border_block_width,
    progress,
  );
  target.border_top_width =
    interpolate_option_length(from.border_top_width, to.border_top_width, progress);
  target.border_right_width =
    interpolate_option_length(from.border_right_width, to.border_right_width, progress);
  target.border_bottom_width =
    interpolate_option_length(from.border_bottom_width, to.border_bottom_width, progress);
  target.border_left_width =
    interpolate_option_length(from.border_left_width, to.border_left_width, progress);
  target.outline_width = interpolate_option_length(from.outline_width, to.outline_width, progress);
  target.outline_offset =
    interpolate_option_length(from.outline_offset, to.outline_offset, progress);

  target.border_top_left_radius = interpolate_option_space_pair_length(
    from.border_top_left_radius,
    to.border_top_left_radius,
    progress,
  );
  target.border_top_right_radius = interpolate_option_space_pair_length(
    from.border_top_right_radius,
    to.border_top_right_radius,
    progress,
  );
  target.border_bottom_right_radius = interpolate_option_space_pair_length(
    from.border_bottom_right_radius,
    to.border_bottom_right_radius,
    progress,
  );
  target.border_bottom_left_radius = interpolate_option_space_pair_length(
    from.border_bottom_left_radius,
    to.border_bottom_left_radius,
    progress,
  );

  target.translate =
    interpolate_option_space_pair_auto_length(from.translate, to.translate, progress);
  target.translate_x = interpolate_option_length(from.translate_x, to.translate_x, progress);
  target.translate_y = interpolate_option_length(from.translate_y, to.translate_y, progress);
  target.rotate = interpolate_option_angle(from.rotate, to.rotate, progress);
  target.scale = interpolate_option_scale_pair(from.scale, to.scale, progress);
  target.scale_x = interpolate_option_percentage(from.scale_x, to.scale_x, progress);
  target.scale_y = interpolate_option_percentage(from.scale_y, to.scale_y, progress);
  target.transform_origin =
    interpolate_option_background_position(from.transform_origin, to.transform_origin, progress);

  target.color = interpolate_color_input(from.color, to.color, progress, current_color);
  target.background_color = interpolate_option_color_input(
    from.background_color,
    to.background_color,
    progress,
    current_color,
  );
  target.border_color =
    interpolate_option_color_input(from.border_color, to.border_color, progress, current_color);
  target.outline_color = interpolate_option_color_input(
    from.outline_color,
    to.outline_color,
    progress,
    current_color,
  );
  target.text_decoration_color = interpolate_option_color_input(
    from.text_decoration_color,
    to.text_decoration_color,
    progress,
    current_color,
  );
  target.webkit_text_stroke_color = interpolate_option_color_input(
    from.webkit_text_stroke_color,
    to.webkit_text_stroke_color,
    progress,
    current_color,
  );
  target.webkit_text_fill_color = interpolate_option_color_input(
    from.webkit_text_fill_color,
    to.webkit_text_fill_color,
    progress,
    current_color,
  );

  target.font_size = interpolate_option_length(from.font_size, to.font_size, progress);
  target.letter_spacing =
    interpolate_option_length(from.letter_spacing, to.letter_spacing, progress);
  target.word_spacing = interpolate_option_length(from.word_spacing, to.word_spacing, progress);

  let _ = sizing;
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_percentage(
  from: PercentageNumber,
  to: PercentageNumber,
  progress: f32,
) -> PercentageNumber {
  PercentageNumber(from.0 + (to.0 - from.0) * progress)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_percentage(
  from: Option<PercentageNumber>,
  to: Option<PercentageNumber>,
  progress: f32,
) -> Option<PercentageNumber> {
  interpolate_option(from, to, progress, interpolate_percentage)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_angle(from: Angle, to: Angle, progress: f32) -> Angle {
  Angle::new(*from + (*to - *from) * progress)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_angle(
  from: Option<Angle>,
  to: Option<Angle>,
  progress: f32,
) -> Option<Angle> {
  interpolate_option(from, to, progress, interpolate_angle)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_length<const DEFAULT_AUTO: bool>(
  from: Length<DEFAULT_AUTO>,
  to: Length<DEFAULT_AUTO>,
  progress: f32,
) -> Option<Length<DEFAULT_AUTO>> {
  match (from, to) {
    (Length::Percentage(lhs), Length::Percentage(rhs)) => {
      Some(Length::Percentage(lerp(lhs, rhs, progress)))
    }
    (Length::Rem(lhs), Length::Rem(rhs)) => Some(Length::Rem(lerp(lhs, rhs, progress))),
    (Length::Em(lhs), Length::Em(rhs)) => Some(Length::Em(lerp(lhs, rhs, progress))),
    (Length::Vh(lhs), Length::Vh(rhs)) => Some(Length::Vh(lerp(lhs, rhs, progress))),
    (Length::Vw(lhs), Length::Vw(rhs)) => Some(Length::Vw(lerp(lhs, rhs, progress))),
    (Length::CqH(lhs), Length::CqH(rhs)) => Some(Length::CqH(lerp(lhs, rhs, progress))),
    (Length::CqW(lhs), Length::CqW(rhs)) => Some(Length::CqW(lerp(lhs, rhs, progress))),
    (Length::CqMin(lhs), Length::CqMin(rhs)) => Some(Length::CqMin(lerp(lhs, rhs, progress))),
    (Length::CqMax(lhs), Length::CqMax(rhs)) => Some(Length::CqMax(lerp(lhs, rhs, progress))),
    (Length::VMin(lhs), Length::VMin(rhs)) => Some(Length::VMin(lerp(lhs, rhs, progress))),
    (Length::VMax(lhs), Length::VMax(rhs)) => Some(Length::VMax(lerp(lhs, rhs, progress))),
    (Length::Cm(lhs), Length::Cm(rhs)) => Some(Length::Cm(lerp(lhs, rhs, progress))),
    (Length::Mm(lhs), Length::Mm(rhs)) => Some(Length::Mm(lerp(lhs, rhs, progress))),
    (Length::In(lhs), Length::In(rhs)) => Some(Length::In(lerp(lhs, rhs, progress))),
    (Length::Q(lhs), Length::Q(rhs)) => Some(Length::Q(lerp(lhs, rhs, progress))),
    (Length::Pt(lhs), Length::Pt(rhs)) => Some(Length::Pt(lerp(lhs, rhs, progress))),
    (Length::Pc(lhs), Length::Pc(rhs)) => Some(Length::Pc(lerp(lhs, rhs, progress))),
    (Length::Px(lhs), Length::Px(rhs)) => Some(Length::Px(lerp(lhs, rhs, progress))),
    (Length::Auto, Length::Auto) => Some(Length::Auto),
    _ => None,
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_length<const DEFAULT_AUTO: bool>(
  from: Option<Length<DEFAULT_AUTO>>,
  to: Option<Length<DEFAULT_AUTO>>,
  progress: f32,
) -> Option<Length<DEFAULT_AUTO>> {
  match (from, to) {
    (Some(lhs), Some(rhs)) => interpolate_length(lhs, rhs, progress).or(if progress >= 0.5 {
      Some(rhs)
    } else {
      Some(lhs)
    }),
    (Some(lhs), None) => {
      if progress >= 0.5 {
        None
      } else {
        Some(lhs)
      }
    }
    (None, Some(rhs)) => {
      if progress >= 0.5 {
        Some(rhs)
      } else {
        None
      }
    }
    (None, None) => None,
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_space_pair_length<const DEFAULT_AUTO: bool, const Y_FIRST: bool>(
  from: SpacePair<Length<DEFAULT_AUTO>, Y_FIRST>,
  to: SpacePair<Length<DEFAULT_AUTO>, Y_FIRST>,
  progress: f32,
) -> SpacePair<Length<DEFAULT_AUTO>, Y_FIRST> {
  SpacePair::from_pair(
    interpolate_length(from.x, to.x, progress).unwrap_or(if progress >= 0.5 {
      to.x
    } else {
      from.x
    }),
    interpolate_length(from.y, to.y, progress).unwrap_or(if progress >= 0.5 {
      to.y
    } else {
      from.y
    }),
  )
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_space_pair_length<const DEFAULT_AUTO: bool, const Y_FIRST: bool>(
  from: Option<SpacePair<Length<DEFAULT_AUTO>, Y_FIRST>>,
  to: Option<SpacePair<Length<DEFAULT_AUTO>, Y_FIRST>>,
  progress: f32,
) -> Option<SpacePair<Length<DEFAULT_AUTO>, Y_FIRST>> {
  interpolate_option(
    from,
    to,
    progress,
    interpolate_space_pair_length::<DEFAULT_AUTO, Y_FIRST>,
  )
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_space_pair_auto_length<const Y_FIRST: bool>(
  from: Option<SpacePair<Length, Y_FIRST>>,
  to: Option<SpacePair<Length, Y_FIRST>>,
  progress: f32,
) -> Option<SpacePair<Length, Y_FIRST>> {
  interpolate_option(
    from,
    to,
    progress,
    interpolate_space_pair_length::<true, Y_FIRST>,
  )
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_sides_length<const DEFAULT_AUTO: bool>(
  from: Sides<Length<DEFAULT_AUTO>>,
  to: Sides<Length<DEFAULT_AUTO>>,
  progress: f32,
) -> Sides<Length<DEFAULT_AUTO>> {
  Sides([
    interpolate_length(from.0[0], to.0[0], progress).unwrap_or(if progress >= 0.5 {
      to.0[0]
    } else {
      from.0[0]
    }),
    interpolate_length(from.0[1], to.0[1], progress).unwrap_or(if progress >= 0.5 {
      to.0[1]
    } else {
      from.0[1]
    }),
    interpolate_length(from.0[2], to.0[2], progress).unwrap_or(if progress >= 0.5 {
      to.0[2]
    } else {
      from.0[2]
    }),
    interpolate_length(from.0[3], to.0[3], progress).unwrap_or(if progress >= 0.5 {
      to.0[3]
    } else {
      from.0[3]
    }),
  ])
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_sides_auto_length(
  from: Option<Sides<Length>>,
  to: Option<Sides<Length>>,
  progress: f32,
) -> Option<Sides<Length>> {
  interpolate_option(from, to, progress, interpolate_sides_length::<true>)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_gap(
  from: SpacePair<Length<false>, true>,
  to: SpacePair<Length<false>, true>,
  progress: f32,
) -> SpacePair<Length<false>, true> {
  interpolate_space_pair_length(from, to, progress)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_border_radius(from: BorderRadius, to: BorderRadius, progress: f32) -> BorderRadius {
  BorderRadius(Sides([
    interpolate_space_pair_length(from.0.0[0], to.0.0[0], progress),
    interpolate_space_pair_length(from.0.0[1], to.0.0[1], progress),
    interpolate_space_pair_length(from.0.0[2], to.0.0[2], progress),
    interpolate_space_pair_length(from.0.0[3], to.0.0[3], progress),
  ]))
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_color(from: Color, to: Color, progress: f32) -> Color {
  let [r1, g1, b1, a1] = from.0;
  let [r2, g2, b2, a2] = to.0;
  Color([
    lerp(r1 as f32, r2 as f32, progress).round() as u8,
    lerp(g1 as f32, g2 as f32, progress).round() as u8,
    lerp(b1 as f32, b2 as f32, progress).round() as u8,
    lerp(a1 as f32, a2 as f32, progress).round() as u8,
  ])
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_color_input<const DEFAULT_CURRENT_COLOR: bool>(
  from: ColorInput<DEFAULT_CURRENT_COLOR>,
  to: ColorInput<DEFAULT_CURRENT_COLOR>,
  progress: f32,
  current_color: Color,
) -> ColorInput<DEFAULT_CURRENT_COLOR> {
  match (from, to) {
    (ColorInput::Value(lhs), ColorInput::Value(rhs)) => {
      ColorInput::Value(interpolate_color(lhs, rhs, progress))
    }
    (ColorInput::CurrentColor, ColorInput::CurrentColor) => ColorInput::CurrentColor,
    _ => {
      let resolved = interpolate_color(
        from.resolve(current_color),
        to.resolve(current_color),
        progress,
      );
      ColorInput::Value(resolved)
    }
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_color_input<const DEFAULT_CURRENT_COLOR: bool>(
  from: Option<ColorInput<DEFAULT_CURRENT_COLOR>>,
  to: Option<ColorInput<DEFAULT_CURRENT_COLOR>>,
  progress: f32,
  current_color: Color,
) -> Option<ColorInput<DEFAULT_CURRENT_COLOR>> {
  match (from, to) {
    (Some(lhs), Some(rhs)) => Some(interpolate_color_input(lhs, rhs, progress, current_color)),
    (Some(lhs), None) => {
      if progress >= 0.5 {
        None
      } else {
        Some(lhs)
      }
    }
    (None, Some(rhs)) => {
      if progress >= 0.5 {
        Some(rhs)
      } else {
        None
      }
    }
    (None, None) => None,
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_background_position(
  from: BackgroundPosition,
  to: BackgroundPosition,
  progress: f32,
) -> BackgroundPosition {
  BackgroundPosition(SpacePair::from_pair(
    PositionComponent::Length(
      interpolate_length(Length::from(from.0.x), Length::from(to.0.x), progress).unwrap_or(
        if progress >= 0.5 {
          Length::from(to.0.x)
        } else {
          Length::from(from.0.x)
        },
      ),
    ),
    PositionComponent::Length(
      interpolate_length(Length::from(from.0.y), Length::from(to.0.y), progress).unwrap_or(
        if progress >= 0.5 {
          Length::from(to.0.y)
        } else {
          Length::from(from.0.y)
        },
      ),
    ),
  ))
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_background_position(
  from: Option<BackgroundPosition>,
  to: Option<BackgroundPosition>,
  progress: f32,
) -> Option<BackgroundPosition> {
  interpolate_option(from, to, progress, interpolate_background_position)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_scale_pair(
  from: SpacePair<PercentageNumber>,
  to: SpacePair<PercentageNumber>,
  progress: f32,
) -> SpacePair<PercentageNumber> {
  SpacePair::from_pair(
    interpolate_percentage(from.x, to.x, progress),
    interpolate_percentage(from.y, to.y, progress),
  )
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option_scale_pair(
  from: Option<SpacePair<PercentageNumber>>,
  to: Option<SpacePair<PercentageNumber>>,
  progress: f32,
) -> Option<SpacePair<PercentageNumber>> {
  interpolate_option(from, to, progress, interpolate_scale_pair)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn interpolate_option<T: Copy>(
  from: Option<T>,
  to: Option<T>,
  progress: f32,
  interpolate: impl Fn(T, T, f32) -> T,
) -> Option<T> {
  match (from, to) {
    (Some(lhs), Some(rhs)) => Some(interpolate(lhs, rhs, progress)),
    (Some(lhs), None) => {
      if progress >= 0.5 {
        None
      } else {
        Some(lhs)
      }
    }
    (None, Some(rhs)) => {
      if progress >= 0.5 {
        Some(rhs)
      } else {
        None
      }
    }
    (None, None) => None,
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
fn lerp(lhs: f32, rhs: f32, progress: f32) -> f32 {
  lhs + (rhs - lhs) * progress
}
