#[cfg(feature = "css_stylesheet_parsing")]
use std::borrow::Cow;

#[cfg(feature = "css_stylesheet_parsing")]
use parley::{FontFeature, FontVariation};
#[cfg(feature = "css_stylesheet_parsing")]
use std::cmp::Ordering;

use super::StyleDeclarationBlock;
use serde::Deserialize;

#[cfg(feature = "css_stylesheet_parsing")]
use crate::{
  layout::style::{selector::StyleSheet, *},
  rendering::{RenderContext, Sizing},
};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
/// A single structured keyframe rule.
pub struct KeyframeRule {
  /// Keyframe offsets as values between 0.0 and 1.0.
  pub offsets: Vec<f32>,
  /// Declarations applied at this step.
  pub declarations: StyleDeclarationBlock,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
/// Structured keyframes that can be passed directly in render options.
pub struct KeyframesRule {
  /// Animation name matched by `animation-name`.
  pub name: String,
  /// Individual keyframe rules for this animation.
  pub keyframes: Vec<KeyframeRule>,
}

#[cfg(feature = "css_stylesheet_parsing")]
pub(crate) fn apply_stylesheet_animations(
  mut base_style: ComputedStyle,
  context: &RenderContext<'_>,
) -> ComputedStyle {
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

    let resolved_frames = resolve_keyframes(&keyframes, &base_snapshot);
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
  base_style: ComputedStyle,
  _context: &crate::rendering::RenderContext<'_>,
) -> ComputedStyle {
  base_style
}

#[cfg(feature = "css_stylesheet_parsing")]
fn find_keyframes<'a>(stylesheets: &'a [StyleSheet], name: &str) -> Option<Cow<'a, KeyframesRule>> {
  stylesheets
    .iter()
    .rev()
    .find_map(|sheet| {
      sheet
        .keyframes
        .iter()
        .rev()
        .find(|rule| rule.name.eq_ignore_ascii_case(name))
    })
    .map(Cow::Borrowed)
    .or_else(|| tailwind_animation_keyframes(name).map(Cow::Owned))
}

fn tailwind_animation_keyframes(name: &str) -> Option<KeyframesRule> {
  match name.to_ascii_lowercase().as_str() {
    "spin" => Some(KeyframesRule {
      name: "spin".to_string(),
      keyframes: vec![
        keyframe(0.25, [StyleDeclaration::rotate(Some(Angle::new(90.0)))]),
        keyframe(0.5, [StyleDeclaration::rotate(Some(Angle::new(180.0)))]),
        keyframe(0.75, [StyleDeclaration::rotate(Some(Angle::new(270.0)))]),
        keyframe(1.0, [StyleDeclaration::rotate(Some(Angle::new(359.999)))]),
      ],
    }),
    "ping" => Some(KeyframesRule {
      name: "ping".to_string(),
      keyframes: vec![
        keyframe(
          0.75,
          [
            StyleDeclaration::scale(SpacePair::from_single(PercentageNumber(2.0))),
            StyleDeclaration::opacity(PercentageNumber(0.0)),
          ],
        ),
        keyframe(
          1.0,
          [
            StyleDeclaration::scale(SpacePair::from_single(PercentageNumber(2.0))),
            StyleDeclaration::opacity(PercentageNumber(0.0)),
          ],
        ),
      ],
    }),
    "pulse" => Some(KeyframesRule {
      name: "pulse".to_string(),
      keyframes: vec![keyframe(
        0.5,
        [StyleDeclaration::opacity(PercentageNumber(0.5))],
      )],
    }),
    "bounce" => Some(KeyframesRule {
      name: "bounce".to_string(),
      keyframes: vec![
        keyframe(
          0.0,
          [StyleDeclaration::translate(SpacePair::from_pair(
            Length::Px(0.0),
            Length::Percentage(-25.0),
          ))],
        ),
        keyframe(
          0.5,
          [StyleDeclaration::translate(SpacePair::from_pair(
            Length::Px(0.0),
            Length::Percentage(0.0),
          ))],
        ),
        keyframe(
          1.0,
          [StyleDeclaration::translate(SpacePair::from_pair(
            Length::Px(0.0),
            Length::Percentage(-25.0),
          ))],
        ),
      ],
    }),
    _ => None,
  }
}

fn keyframe<const N: usize>(offset: f32, declarations: [StyleDeclaration; N]) -> KeyframeRule {
  let mut block = StyleDeclarationBlock::default();
  for declaration in declarations {
    block.push(declaration, false);
  }

  KeyframeRule {
    offsets: vec![offset],
    declarations: block,
  }
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
  base_style: &'a ComputedStyle,
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
fn resolve_keyframes(keyframes: &KeyframesRule, base_style: &ComputedStyle) -> ResolvedKeyframes {
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
  style: ComputedStyle,
  mask: PropertyMask,
}

#[cfg(feature = "css_stylesheet_parsing")]
impl ResolvedKeyframeStyle {
  fn new(style: ComputedStyle, mask: PropertyMask) -> Self {
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
  from_style: &'a ComputedStyle,
  to_style: &'a ComputedStyle,
  animated_properties: PropertyMask,
  progress: f32,
}

#[cfg(feature = "css_stylesheet_parsing")]
impl<'a> InterpolationSegment<'a> {
  fn new(
    from_style: &'a ComputedStyle,
    from_mask: Option<&'a PropertyMask>,
    to_style: &'a ComputedStyle,
    to_mask: Option<&'a PropertyMask>,
    progress: f32,
  ) -> Self {
    let mut animated_properties = PropertyMask::new();
    if let Some(mask) = from_mask {
      animated_properties.extend(mask.iter());
    }
    if let Some(mask) = to_mask {
      animated_properties.extend(mask.iter());
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
  base_style: &ComputedStyle,
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
  style: &mut ComputedStyle,
  mask: &mut PropertyMask,
  keyframe: &KeyframeRule,
) {
  for declaration in keyframe.declarations.iter() {
    declaration.apply_to_computed(style);
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
  FontFamily,
  LineHeight,
  FontSynthesis,
  FontSynthesic,
  LineClamp,
  TextAlign,
  TextStroke,
  LineJoin,
  TextDecoration,
  TextDecorationLines,
  TextDecorationStyle,
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
  Visibility,
  VerticalAlign,
  Flex,
  Background,
  GridTrackSize,
  GridTemplateComponent,
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
  fn option_angle_interpolates_inner_angle() {
    let mut target: Option<Angle> = None;
    target.interpolate(
      &Some(Angle::new(0.0)),
      &Some(Angle::new(90.0)),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, Some(Angle::new(45.0)));
  }

  #[test]
  fn option_angle_interpolates_from_missing_zero_angle() {
    let mut target: Option<Angle> = None;
    target.interpolate(
      &None,
      &Some(Angle::new(45.0)),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, Some(Angle::new(22.5)));
  }

  #[test]
  fn aspect_ratio_interpolates_ratio_values() {
    let mut target = AspectRatio::Auto;
    target.interpolate(
      &AspectRatio::Ratio(1.0),
      &AspectRatio::Ratio(2.0),
      0.25,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, AspectRatio::Ratio(1.25));
  }

  #[test]
  fn font_stretch_interpolates_percentages() {
    let mut target = FontStretch::from_percentage(0.0);
    target.interpolate(
      &FontStretch::from_percentage(0.75),
      &FontStretch::from_percentage(1.25),
      0.5,
      &sizing(),
      current_color(),
    );

    assert!((target.percentage() - 1.0).abs() < f32::EPSILON);
  }

  #[test]
  fn font_weight_interpolates_numeric_values() {
    let mut target = FontWeight::default();
    target.interpolate(
      &FontWeight::from(400.0),
      &FontWeight::from(700.0),
      0.5,
      &sizing(),
      current_color(),
    );

    assert!((target.value() - 550.0).abs() < f32::EPSILON);
  }

  #[test]
  fn text_decoration_thickness_interpolates_lengths() {
    let mut target = TextDecorationThickness::default();
    target.interpolate(
      &TextDecorationThickness::Length(Length::Px(2.0)),
      &TextDecorationThickness::Length(Length::Px(10.0)),
      0.25,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, TextDecorationThickness::Length(Length::Px(4.0)));
  }

  #[test]
  fn flex_grow_interpolates_numeric_values() {
    let mut target = FlexGrow(0.0);
    target.interpolate(
      &FlexGrow(1.0),
      &FlexGrow(3.0),
      0.5,
      &sizing(),
      current_color(),
    );

    assert!((target.0 - 2.0).abs() < f32::EPSILON);
  }

  #[test]
  fn transform_translate_interpolates_lengths() {
    let mut target = Transform::Translate(Length::zero(), Length::zero());
    target.interpolate(
      &Transform::Translate(Length::Px(0.0), Length::Px(10.0)),
      &Transform::Translate(Length::Px(20.0), Length::Px(30.0)),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      target,
      Transform::Translate(Length::Px(10.0), Length::Px(20.0))
    );
  }

  #[test]
  fn background_size_interpolates_explicit_lengths() {
    let mut target = BackgroundSize::default();
    target.interpolate(
      &BackgroundSize::Explicit {
        width: Length::Px(10.0),
        height: Length::Px(20.0),
      },
      &BackgroundSize::Explicit {
        width: Length::Px(30.0),
        height: Length::Px(60.0),
      },
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      target,
      BackgroundSize::Explicit {
        width: Length::Px(20.0),
        height: Length::Px(40.0),
      }
    );
  }

  #[test]
  fn box_shadow_interpolates_lengths_and_color() {
    let mut target = BoxShadow {
      inset: false,
      offset_x: Length::zero(),
      offset_y: Length::zero(),
      blur_radius: Length::zero(),
      spread_radius: Length::zero(),
      color: ColorInput::CurrentColor,
    };
    target.interpolate(
      &BoxShadow {
        inset: false,
        offset_x: Length::Px(0.0),
        offset_y: Length::Px(10.0),
        blur_radius: Length::Px(20.0),
        spread_radius: Length::Px(30.0),
        color: ColorInput::Value(Color([0, 0, 0, 255])),
      },
      &BoxShadow {
        inset: false,
        offset_x: Length::Px(20.0),
        offset_y: Length::Px(30.0),
        blur_radius: Length::Px(40.0),
        spread_radius: Length::Px(50.0),
        color: ColorInput::Value(Color([200, 100, 50, 255])),
      },
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      target,
      BoxShadow {
        inset: false,
        offset_x: Length::Px(10.0),
        offset_y: Length::Px(20.0),
        blur_radius: Length::Px(30.0),
        spread_radius: Length::Px(40.0),
        color: ColorInput::Value(Color([100, 50, 25, 255])),
      }
    );
  }

  #[test]
  fn text_shadow_interpolates_lengths_and_color() {
    let mut target = TextShadow {
      offset_x: Length::zero(),
      offset_y: Length::zero(),
      blur_radius: Length::zero(),
      color: ColorInput::CurrentColor,
    };
    target.interpolate(
      &TextShadow {
        offset_x: Length::Px(0.0),
        offset_y: Length::Px(10.0),
        blur_radius: Length::Px(20.0),
        color: ColorInput::Value(Color([0, 0, 0, 255])),
      },
      &TextShadow {
        offset_x: Length::Px(20.0),
        offset_y: Length::Px(30.0),
        blur_radius: Length::Px(40.0),
        color: ColorInput::Value(Color([200, 100, 50, 255])),
      },
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      target,
      TextShadow {
        offset_x: Length::Px(10.0),
        offset_y: Length::Px(20.0),
        blur_radius: Length::Px(30.0),
        color: ColorInput::Value(Color([100, 50, 25, 255])),
      }
    );
  }

  #[test]
  fn filter_blur_interpolates_lengths() {
    let mut target = Filter::Blur(Length::zero());
    target.interpolate(
      &Filter::Blur(Length::Px(4.0)),
      &Filter::Blur(Length::Px(12.0)),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, Filter::Blur(Length::Px(8.0)));
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
  fn tailwind_animation_presets_include_built_in_keyframes() {
    assert!(super::tailwind_animation_keyframes("spin").is_some());
    assert!(super::tailwind_animation_keyframes("ping").is_some());
    assert!(super::tailwind_animation_keyframes("pulse").is_some());
    assert!(super::tailwind_animation_keyframes("bounce").is_some());
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
  fn vec_animates_repeatable_lists_to_lcm_length() {
    let mut values: Vec<BackgroundSize> = Vec::new();
    values.interpolate(
      &vec![BackgroundSize::Explicit {
        width: Length::Px(10.0),
        height: Length::Px(20.0),
      }],
      &vec![
        BackgroundSize::Explicit {
          width: Length::Px(30.0),
          height: Length::Px(40.0),
        },
        BackgroundSize::Explicit {
          width: Length::Px(50.0),
          height: Length::Px(60.0),
        },
      ],
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      values,
      vec![
        BackgroundSize::Explicit {
          width: Length::Px(20.0),
          height: Length::Px(30.0),
        },
        BackgroundSize::Explicit {
          width: Length::Px(30.0),
          height: Length::Px(40.0),
        },
      ]
    );
  }

  #[test]
  fn boxed_background_lists_animate_repeatable_lists_to_lcm_length() {
    let mut values: Box<[BackgroundSize]> = Box::default();
    values.interpolate(
      &[BackgroundSize::Explicit {
        width: Length::Px(10.0),
        height: Length::Px(20.0),
      }]
      .into(),
      &[
        BackgroundSize::Explicit {
          width: Length::Px(30.0),
          height: Length::Px(40.0),
        },
        BackgroundSize::Explicit {
          width: Length::Px(50.0),
          height: Length::Px(60.0),
        },
      ]
      .into(),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      values,
      [
        BackgroundSize::Explicit {
          width: Length::Px(20.0),
          height: Length::Px(30.0),
        },
        BackgroundSize::Explicit {
          width: Length::Px(30.0),
          height: Length::Px(40.0),
        },
      ]
      .into()
    );
  }

  #[test]
  fn boxed_transform_lists_pad_to_longest_with_neutral_values() {
    let mut values: Box<[Transform]> = Box::default();
    values.interpolate(
      &[Transform::Scale(1.0, 1.0)].into(),
      &[Transform::Scale(2.0, 2.0), Transform::Scale(4.0, 4.0)].into(),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      values,
      [Transform::Scale(1.5, 1.5), Transform::Scale(2.5, 2.5)].into()
    );
  }

  #[test]
  fn apply_interpolated_properties_only_updates_masked_fields() {
    let mut base_style = ComputedStyle {
      width: Length::Px(10.0),
      height: Length::Px(20.0),
      ..ComputedStyle::default()
    };
    let from = ComputedStyle {
      width: Length::Px(10.0),
      height: Length::Px(100.0),
      ..ComputedStyle::default()
    };
    let to = ComputedStyle {
      width: Length::Px(30.0),
      height: Length::Px(200.0),
      ..ComputedStyle::default()
    };
    let animated_properties: PropertyMask = [LonghandId::Width].into_iter().collect();

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

  #[test]
  fn apply_interpolated_properties_interpolates_rotate_from_implicit_none() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      rotate: Some(Angle::new(45.0)),
      ..ComputedStyle::default()
    };
    let animated_properties: PropertyMask = [LonghandId::Rotate].into_iter().collect();

    base_style.apply_interpolated_properties(
      &from,
      &to,
      &animated_properties,
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(base_style.rotate, Some(Angle::new(22.5)));
  }

  #[test]
  fn apply_interpolated_properties_interpolates_flex_grow_from_implicit_zero() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      flex_grow: Some(FlexGrow(4.0)),
      ..ComputedStyle::default()
    };
    let animated_properties: PropertyMask = [LonghandId::FlexGrow].into_iter().collect();

    base_style.apply_interpolated_properties(
      &from,
      &to,
      &animated_properties,
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(base_style.flex_grow, Some(FlexGrow(2.0)));
  }

  #[test]
  fn apply_interpolated_properties_interpolates_flex_shrink_from_implicit_one() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      flex_shrink: Some(FlexGrow(3.0)),
      ..ComputedStyle::default()
    };
    let animated_properties: PropertyMask = [LonghandId::FlexShrink].into_iter().collect();

    base_style.apply_interpolated_properties(
      &from,
      &to,
      &animated_properties,
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(base_style.flex_shrink, Some(FlexGrow(2.0)));
  }

  #[test]
  fn apply_interpolated_properties_interpolates_text_stroke_width_from_implicit_zero() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      webkit_text_stroke_width: Some(Length::Px(6.0)),
      ..ComputedStyle::default()
    };
    let animated_properties: PropertyMask =
      [LonghandId::WebkitTextStrokeWidth].into_iter().collect();

    base_style.apply_interpolated_properties(
      &from,
      &to,
      &animated_properties,
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(base_style.webkit_text_stroke_width, Some(Length::Px(3.0)));
  }

  #[test]
  fn apply_interpolated_properties_interpolates_text_stroke_color_from_current_color() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      webkit_text_stroke_color: Some(ColorInput::Value(Color([110, 120, 130, 255]))),
      ..ComputedStyle::default()
    };
    let animated_properties: PropertyMask =
      [LonghandId::WebkitTextStrokeColor].into_iter().collect();

    base_style.apply_interpolated_properties(
      &from,
      &to,
      &animated_properties,
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      base_style.webkit_text_stroke_color,
      Some(ColorInput::Value(Color([60, 70, 80, 255])))
    );
  }

  #[test]
  fn apply_interpolated_properties_interpolates_text_fill_color_from_style_color() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle {
      color: ColorInput::Value(Color([20, 40, 60, 255])),
      ..ComputedStyle::default()
    };
    let to = ComputedStyle {
      color: ColorInput::Value(Color([20, 40, 60, 255])),
      webkit_text_fill_color: Some(ColorInput::Value(Color([120, 140, 160, 255]))),
      ..ComputedStyle::default()
    };
    let animated_properties: PropertyMask = [LonghandId::WebkitTextFillColor].into_iter().collect();

    base_style.apply_interpolated_properties(
      &from,
      &to,
      &animated_properties,
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      base_style.webkit_text_fill_color,
      Some(ColorInput::Value(Color([70, 90, 110, 255])))
    );
  }
}
