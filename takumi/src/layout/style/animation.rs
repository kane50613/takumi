#[cfg(feature = "css_stylesheet_parsing")]
use std::cmp::Ordering;

#[cfg(feature = "css_stylesheet_parsing")]
use crate::{
  layout::style::{
    selector::{KeyframeRule, KeyframesRule, StyleSheet},
    *,
  },
  rendering::{RenderContext, Sizing},
};

#[cfg(feature = "css_stylesheet_parsing")]
pub(crate) fn apply_stylesheet_animations(
  mut base_style: ResolvedStyle,
  context: &RenderContext<'_>,
) -> ResolvedStyle {
  if base_style.animation_name.0.is_empty() {
    return base_style;
  }

  let base_snapshot = base_style.clone();

  for (animation_index, animation_name) in base_snapshot.animation_name.0.iter().enumerate() {
    let Some(keyframes) = find_keyframes(&context.stylesheets, animation_name) else {
      continue;
    };

    if play_state_at(&base_snapshot.animation_play_state, animation_index)
      == AnimationPlayState::Paused
    {
      continue;
    }

    let duration = time_at(
      &base_snapshot.animation_duration,
      animation_index,
      AnimationTime::from_milliseconds(0.0),
    );
    let delay = time_at(
      &base_snapshot.animation_delay,
      animation_index,
      AnimationTime::from_milliseconds(0.0),
    );
    let iteration_count =
      iteration_count_at(&base_snapshot.animation_iteration_count, animation_index);
    let direction = direction_at(&base_snapshot.animation_direction, animation_index);
    let fill_mode = fill_mode_at(&base_snapshot.animation_fill_mode, animation_index);
    let timing_function =
      timing_function_at(&base_snapshot.animation_timing_function, animation_index);

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
      sample_keyframe_segment(keyframes, &base_snapshot, progress)
    else {
      continue;
    };

    let eased_progress = apply_timing_function(&timing_function, segment_progress);
    base_style.apply_interpolated_properties(
      from_style,
      &to_style,
      eased_progress,
      &context.sizing,
      context.current_color,
    );
  }

  base_style
}

#[cfg(not(feature = "css_stylesheet_parsing"))]
pub(crate) fn apply_stylesheet_animations(
  base_style: ResolvedStyle,
  _context: &crate::rendering::RenderContext<'_>,
) -> ResolvedStyle {
  base_style
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
  let progress = (active_time / duration_ms).fract();

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
  let mut style = base_style.clone();
  for declaration in &keyframe.declarations {
    declaration.apply_to_resolved(&mut style);
  }
  style
}

#[cfg(feature = "css_stylesheet_parsing")]
macro_rules! impl_passthrough_animatable {
  ($($ty:ty),* $(,)?) => {
    $(
      impl Animatable for $ty {}
    )*
  };
}

#[cfg(feature = "css_stylesheet_parsing")]
impl_passthrough_animatable!(
  BoxSizing,
  AnimationNames,
  AnimationDurations,
  AnimationTimingFunctions,
  AnimationIterationCounts,
  AnimationDirections,
  AnimationFillModes,
  AnimationPlayStates,
  Display,
  AspectRatio,
  FlexDirection,
  AlignItems,
  JustifyContent,
  FlexWrap,
  Position,
  BorderStyle,
  Border,
  ObjectFit,
  Overflow,
  BackgroundClip,
  GridAutoFlow,
  GridLine,
  GridTemplateAreas,
  TextOverflow,
  TextTransform,
  FontStyle,
  FontStretch,
  FontFamily,
  LineHeight,
  FontWeight,
  FontSynthesis,
  FontSynthesic,
  LineClamp,
  TextAlign,
  TextStroke,
  LineJoin,
  TextDecoration,
  TextDecorationLines,
  TextDecorationThickness,
  TextDecorationSkipInk,
  ImageScalingAlgorithm,
  OverflowWrap,
  WordBreak,
  BasicShape,
  FillRule,
  WhiteSpace,
  WhiteSpaceCollapse,
  TextWrapMode,
  TextWrapStyle,
  TextWrap,
  Isolation,
  BlendMode,
  Visibility,
  VerticalAlign,
  Flex,
  FlexGrow,
);

#[cfg(feature = "css_stylesheet_parsing")]
impl<const DEFAULT_AUTO: bool> Animatable for Length<DEFAULT_AUTO> {
  fn interpolate(
    &mut self,
    from: Self,
    to: &Self,
    progress: f32,
    _sizing: &Sizing,
    _current_color: Color,
  ) {
    *self =
      interpolate_length(from, *to, progress).unwrap_or(if progress >= 0.5 { *to } else { from });
  }
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
fn lerp(lhs: f32, rhs: f32, progress: f32) -> f32 {
  lhs + (rhs - lhs) * progress
}

#[cfg(all(test, feature = "css_stylesheet_parsing"))]
mod tests {
  use std::rc::Rc;

  use taffy::Size;

  use crate::{
    layout::style::animation::sample_animation_progress,
    layout::{Viewport, style::*},
    rendering::Sizing,
  };

  fn sizing() -> Sizing {
    Sizing {
      viewport: Viewport::new(Some(200), Some(100)),
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  fn current_color() -> Color {
    Color([10, 20, 30, 255])
  }

  #[derive(Clone, Copy, Debug, PartialEq)]
  struct Dummy(u8);

  impl Animatable for Dummy {}

  #[test]
  fn animatable_default_uses_from_value() {
    let mut target = Dummy(9);
    target.interpolate(Dummy(3), &Dummy(7), 0.5, &sizing(), current_color());
    assert_eq!(target, Dummy(3));
  }

  #[test]
  fn length_interpolates_continuously() {
    let mut target: Length = Length::zero();
    target.interpolate(
      Length::Px(10.0),
      &Length::Px(30.0),
      0.25,
      &sizing(),
      current_color(),
    );
    assert_eq!(target, Length::Px(15.0));
  }

  #[test]
  fn option_length_uses_discrete_fallback() {
    let mut target: Option<Length> = None;
    target.interpolate(
      Some(Length::Px(10.0)),
      &None,
      0.25,
      &sizing(),
      current_color(),
    );
    assert_eq!(target, Some(Length::Px(10.0)));

    target.interpolate(
      Some(Length::Px(10.0)),
      &None,
      0.75,
      &sizing(),
      current_color(),
    );
    assert_eq!(target, None);
  }

  #[test]
  fn background_position_interpolates_components() {
    let mut target = BackgroundPosition::default();
    target.interpolate(
      BackgroundPosition(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Left),
        PositionComponent::KeywordY(PositionKeywordY::Top),
      )),
      &BackgroundPosition(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Right),
        PositionComponent::KeywordY(PositionKeywordY::Bottom),
      )),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      target,
      BackgroundPosition(SpacePair::from_pair(
        PositionComponent::Length(Length::Percentage(50.0)),
        PositionComponent::Length(Length::Percentage(50.0)),
      ))
    );
  }

  #[test]
  fn color_input_interpolates_using_current_color() {
    let mut target: ColorInput = ColorInput::CurrentColor;
    target.interpolate(
      ColorInput::CurrentColor,
      &ColorInput::Value(Color([110, 120, 130, 255])),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, ColorInput::Value(Color([60, 70, 80, 255])));
  }

  #[test]
  fn border_radius_interpolates_via_container_impls() {
    let mut target = BorderRadius::default();
    target.interpolate(
      BorderRadius::from(4.0),
      &BorderRadius::from(12.0),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, BorderRadius::from(8.0));
  }

  #[test]
  fn percentage_number_interpolates() {
    let mut target = PercentageNumber::default();
    target.interpolate(
      PercentageNumber(0.2),
      &PercentageNumber(0.6),
      0.5,
      &sizing(),
      current_color(),
    );

    assert!((target.0 - 0.4).abs() < f32::EPSILON);
  }

  #[test]
  fn animation_progress_uses_next_iteration_start_at_boundaries() {
    let progress = sample_animation_progress(
      1000.0,
      1000.0,
      0.0,
      AnimationIterationCount::Infinite,
      AnimationDirection::Alternate,
      AnimationFillMode::Both,
    );

    assert_eq!(progress, Some(1.0));
  }

  #[test]
  fn animation_progress_keeps_final_state_after_finite_completion() {
    let progress = sample_animation_progress(
      2000.0,
      1000.0,
      0.0,
      AnimationIterationCount::Number(2.0),
      AnimationDirection::Alternate,
      AnimationFillMode::Forwards,
    );

    assert_eq!(progress, Some(0.0));
  }
}
