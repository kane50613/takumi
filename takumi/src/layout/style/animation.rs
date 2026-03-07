#[cfg(feature = "css_stylesheet_parsing")]
use parley::{FontFeature, FontVariation};
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

    let resolved_frames = resolve_keyframes(keyframes, &base_snapshot);
    let Some(segment) = sample_keyframe_segment(&resolved_frames, &base_snapshot, progress) else {
      continue;
    };

    let eased_progress = apply_timing_function(&timing_function, segment.progress);
    base_style.apply_interpolated_properties(
      segment.from_style,
      segment.to_style,
      &segment.animated_properties,
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
        let end_progress = match iteration_count {
          AnimationIterationCount::Infinite => end_progress(direction, 0),
          AnimationIterationCount::Number(count) => {
            let count = count.max(0.0);
            let completed_iterations = count.floor() as usize;
            let fraction = count.fract();
            if fraction > f32::EPSILON {
              let iteration_index = completed_iterations.saturating_sub(1);
              apply_direction(fraction, direction, iteration_index)
            } else {
              end_progress(direction, count.max(1.0) as usize - 1)
            }
          }
        };
        Some(end_progress)
      }
      _ => None,
    };
  }

  let progress_within_iteration = active_time / duration_ms;
  let mut iteration_index = progress_within_iteration.floor() as usize;
  let mut progress = progress_within_iteration.fract();
  if active_time > 0.0 && progress_within_iteration.fract().abs() <= f32::EPSILON {
    progress = 1.0;
    iteration_index = iteration_index.saturating_sub(1);
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
fn sample_keyframe_segment<'a>(
  resolved_frames: &'a ResolvedKeyframes,
  base_style: &'a ResolvedStyle,
  progress: f32,
) -> Option<InterpolationSegment<'a>> {
  let first = resolved_frames.points.first()?;

  if progress <= first.offset {
    let segment_progress = if first.offset <= 0.0 {
      1.0
    } else {
      progress / first.offset
    };
    return Some(InterpolationSegment::new(
      base_style,
      None,
      &resolved_frames.style(first.style_index).style,
      Some(&resolved_frames.style(first.style_index).mask),
      segment_progress.clamp(0.0, 1.0),
    ));
  }

  for window in resolved_frames.points.windows(2) {
    let [start_point, end_point] = window else {
      continue;
    };
    if progress <= end_point.offset {
      let width = end_point.offset - start_point.offset;
      let segment_progress = if width <= f32::EPSILON {
        1.0
      } else {
        (progress - start_point.offset) / width
      };
      return Some(InterpolationSegment::new(
        &resolved_frames.style(start_point.style_index).style,
        Some(&resolved_frames.style(start_point.style_index).mask),
        &resolved_frames.style(end_point.style_index).style,
        Some(&resolved_frames.style(end_point.style_index).mask),
        segment_progress.clamp(0.0, 1.0),
      ));
    }
  }

  let last = resolved_frames.points.last()?;
  let segment_progress = if last.offset >= 1.0 {
    1.0
  } else {
    (progress - last.offset) / (1.0 - last.offset)
  };
  Some(InterpolationSegment::new(
    &resolved_frames.style(last.style_index).style,
    Some(&resolved_frames.style(last.style_index).mask),
    base_style,
    None,
    segment_progress.clamp(0.0, 1.0),
  ))
}

#[cfg(feature = "css_stylesheet_parsing")]
fn resolve_keyframes(keyframes: &KeyframesRule, base_style: &ResolvedStyle) -> ResolvedKeyframes {
  let mut points = keyframes
    .keyframes
    .iter()
    .enumerate()
    .flat_map(|(style_index, keyframe)| {
      keyframe
        .offsets
        .iter()
        .copied()
        .map(move |offset| ResolvedKeyframePoint {
          offset,
          style_index,
        })
    })
    .collect::<Vec<_>>();

  points.sort_by(|lhs, rhs| {
    lhs
      .offset
      .partial_cmp(&rhs.offset)
      .unwrap_or(Ordering::Equal)
  });

  let mut styles = Vec::with_capacity(points.len());
  let mut merged_points: Vec<ResolvedKeyframePoint> = Vec::with_capacity(points.len());
  for point in points {
    if let Some(last_point) = merged_points.last_mut()
      && (last_point.offset - point.offset).abs() <= f32::EPSILON
    {
      merge_keyframe_style(
        &mut styles[last_point.style_index],
        &keyframes.keyframes[point.style_index],
      );
      continue;
    }

    let style_index = styles.len();
    styles.push(resolve_keyframe_style(
      &keyframes.keyframes[point.style_index],
      base_style,
    ));
    merged_points.push(ResolvedKeyframePoint {
      offset: point.offset,
      style_index,
    });
  }

  ResolvedKeyframes {
    points: merged_points,
    styles,
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
#[derive(Debug)]
struct ResolvedKeyframeStyle {
  style: ResolvedStyle,
  mask: PropertyMask,
}

#[cfg(feature = "css_stylesheet_parsing")]
impl ResolvedKeyframeStyle {
  fn new(style: ResolvedStyle, mask: PropertyMask) -> Self {
    Self { style, mask }
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
#[derive(Debug)]
struct ResolvedKeyframePoint {
  offset: f32,
  style_index: usize,
}

#[cfg(feature = "css_stylesheet_parsing")]
#[derive(Debug)]
struct ResolvedKeyframes {
  points: Vec<ResolvedKeyframePoint>,
  styles: Vec<ResolvedKeyframeStyle>,
}

#[cfg(feature = "css_stylesheet_parsing")]
impl ResolvedKeyframes {
  fn style(&self, index: usize) -> &ResolvedKeyframeStyle {
    &self.styles[index]
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
#[derive(Debug)]
struct InterpolationSegment<'a> {
  from_style: &'a ResolvedStyle,
  to_style: &'a ResolvedStyle,
  animated_properties: PropertyMask,
  progress: f32,
}

#[cfg(feature = "css_stylesheet_parsing")]
impl<'a> InterpolationSegment<'a> {
  fn new(
    from_style: &'a ResolvedStyle,
    from_mask: Option<&'a PropertyMask>,
    to_style: &'a ResolvedStyle,
    to_mask: Option<&'a PropertyMask>,
    progress: f32,
  ) -> Self {
    let mut animated_properties = PropertyMask::new();
    if let Some(mask) = from_mask {
      animated_properties.extend(mask.iter().copied());
    }
    if let Some(mask) = to_mask {
      animated_properties.extend(mask.iter().copied());
    }
    Self {
      from_style,
      to_style,
      animated_properties,
      progress,
    }
  }
}

#[cfg(feature = "css_stylesheet_parsing")]
fn resolve_keyframe_style(
  keyframe: &KeyframeRule,
  base_style: &ResolvedStyle,
) -> ResolvedKeyframeStyle {
  let mut style = base_style.clone();
  let mut mask = PropertyMask::new();
  apply_keyframe_declarations(&mut style, &mut mask, keyframe);
  ResolvedKeyframeStyle::new(style, mask)
}

#[cfg(feature = "css_stylesheet_parsing")]
fn merge_keyframe_style(style: &mut ResolvedKeyframeStyle, keyframe: &KeyframeRule) {
  apply_keyframe_declarations(&mut style.style, &mut style.mask, keyframe);
}

#[cfg(feature = "css_stylesheet_parsing")]
fn apply_keyframe_declarations(
  style: &mut ResolvedStyle,
  mask: &mut PropertyMask,
  keyframe: &KeyframeRule,
) {
  for declaration in keyframe.declarations.iter() {
    declaration.apply_to_resolved(style);
    mask.insert(declaration.longhand_id());
  }
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
  TextDecorationStyle,
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
  Transform,
  Background,
  BackgroundImage,
  BackgroundSize,
  BackgroundRepeat,
  BoxShadow,
  GridTrackSize,
  GridTemplateComponent,
  Filter,
  TextShadow,
  FontFeature,
  FontVariation,
);

#[cfg(feature = "css_stylesheet_parsing")]
impl<const DEFAULT_AUTO: bool> Animatable for Length<DEFAULT_AUTO> {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &Sizing,
    _current_color: Color,
  ) {
    *self = interpolate_length(*from, *to, progress)
      .or_else(|| {
        resolve_length_with_sizing(*from, sizing).and_then(|resolved_from| {
          resolve_length_with_sizing(*to, sizing)
            .map(|resolved_to| Length::Px(lerp(resolved_from, resolved_to, progress)))
        })
      })
      .unwrap_or(if progress >= 0.5 { *to } else { *from });
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
fn resolve_length_with_sizing<const DEFAULT_AUTO: bool>(
  value: Length<DEFAULT_AUTO>,
  sizing: &Sizing,
) -> Option<f32> {
  if matches!(value, Length::Auto) {
    return None;
  }

  Some(value.to_px(sizing, sizing.viewport.width.unwrap_or_default() as f32))
}

#[cfg(feature = "css_stylesheet_parsing")]
fn lerp(lhs: f32, rhs: f32, progress: f32) -> f32 {
  lhs + (rhs - lhs) * progress
}

#[cfg(all(test, feature = "css_stylesheet_parsing"))]
mod tests {
  use std::collections::BTreeSet;
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
    target.interpolate(&Dummy(3), &Dummy(7), 0.5, &sizing(), current_color());
    assert_eq!(target, Dummy(3));

    target.interpolate(&Dummy(3), &Dummy(7), 1.0, &sizing(), current_color());
    assert_eq!(target, Dummy(7));
  }

  #[test]
  fn length_interpolates_continuously() {
    let mut target: Length = Length::zero();
    target.interpolate(
      &Length::Px(10.0),
      &Length::Px(30.0),
      0.25,
      &sizing(),
      current_color(),
    );
    assert_eq!(target, Length::Px(15.0));
  }

  #[test]
  fn mixed_unit_length_interpolates_via_sizing() {
    let mut target: Length = Length::zero();
    target.interpolate(
      &Length::Px(0.0),
      &Length::Percentage(50.0),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, Length::Px(50.0));
  }

  #[test]
  fn option_length_uses_discrete_fallback() {
    let mut target: Option<Length> = None;
    target.interpolate(
      &Some(Length::Px(10.0)),
      &None,
      0.25,
      &sizing(),
      current_color(),
    );
    assert_eq!(target, Some(Length::Px(10.0)));

    target.interpolate(
      &Some(Length::Px(10.0)),
      &None,
      0.75,
      &sizing(),
      current_color(),
    );
    assert_eq!(target, None);
  }

  #[test]
  fn background_position_interpolates_components() {
    let mut target: BackgroundPosition = BackgroundPosition::default();
    target.interpolate(
      &BackgroundPosition(SpacePair::from_pair(
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
      &ColorInput::CurrentColor,
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
      &BorderRadius::from(4.0),
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
      &PercentageNumber(0.2),
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

  #[test]
  fn animation_progress_keeps_fractional_final_iteration_state() {
    let progress = sample_animation_progress(
      1500.0,
      1000.0,
      0.0,
      AnimationIterationCount::Number(1.5),
      AnimationDirection::Normal,
      AnimationFillMode::Forwards,
    );

    assert_eq!(progress, Some(0.5));
  }

  #[test]
  fn animation_progress_uses_end_of_iteration_for_exact_normal_boundaries() {
    let progress = sample_animation_progress(
      1000.0,
      1000.0,
      0.0,
      AnimationIterationCount::Infinite,
      AnimationDirection::Normal,
      AnimationFillMode::Both,
    );

    assert_eq!(progress, Some(1.0));
  }

  #[test]
  fn vec_animates_pairwise() {
    let mut values: Vec<Length> = vec![Length::Px(0.0), Length::Px(10.0)];
    values.interpolate(
      &vec![Length::Px(0.0), Length::Px(10.0)],
      &vec![Length::Px(20.0), Length::Px(30.0)],
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(values, vec![Length::Px(10.0), Length::Px(20.0)]);
  }

  #[test]
  fn apply_interpolated_properties_only_updates_masked_fields() {
    let mut base_style = ResolvedStyle {
      width: Length::Px(10.0),
      height: Length::Px(20.0),
      ..ResolvedStyle::default()
    };
    let from = ResolvedStyle {
      width: Length::Px(10.0),
      height: Length::Px(100.0),
      ..ResolvedStyle::default()
    };
    let to = ResolvedStyle {
      width: Length::Px(30.0),
      height: Length::Px(200.0),
      ..ResolvedStyle::default()
    };
    let animated_properties = BTreeSet::from([LonghandId::Width]);

    base_style.apply_interpolated_properties(
      &from,
      &to,
      &animated_properties,
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(base_style.width, Length::Px(20.0));
    assert_eq!(base_style.height, Length::Px(20.0));
  }
}
