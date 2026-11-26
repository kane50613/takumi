use std::sync::LazyLock;

use serde::Deserialize;
use typed_builder::TypedBuilder;

use crate::{
  Xxh3HashSet,
  sort_key::sort_by_key,
  style::{
    StyleDeclarationBlock,
    selector::{MediaQueryList, StyleSheet},
    *,
  },
  viewport::Viewport,
};

#[derive(Debug, Clone, Deserialize, PartialEq, TypedBuilder)]
#[serde(rename_all = "camelCase")]
/// A single structured keyframe rule.
pub struct KeyframeRule {
  /// Keyframe offsets as values between 0.0 and 1.0.
  #[builder(setter(into))]
  pub offsets: Vec<f32>,
  /// Declarations applied at this step.
  pub declarations: StyleDeclarationBlock,
}

#[derive(Debug, Clone, Deserialize, PartialEq, TypedBuilder)]
#[serde(rename_all = "camelCase")]
/// Structured keyframes that can be passed directly in render options.
pub struct KeyframesRule {
  /// Animation name matched by `animation-name`.
  #[builder(setter(into))]
  pub name: String,
  /// Individual keyframe rules for this animation.
  #[builder(setter(into))]
  pub keyframes: Vec<KeyframeRule>,
  /// Media queries gating this `@keyframes` rule.
  #[serde(skip, default)]
  #[builder(default, setter(skip))]
  pub media_queries: Vec<MediaQueryList>,
}

/// The animations an element runs at a render's time, decided from its style before they apply,
/// as Blink's `CSSAnimations` decides them from the base style.
pub(crate) struct ElementAnimations<'s> {
  animations: Vec<ActiveAnimation<'s>>,
}

impl<'s> ElementAnimations<'s> {
  /// The animations `base_style` runs at `time` from the keyframes of `stylesheet`, then of
  /// `theme_keyframes`, or `None` when it runs none.
  pub(crate) fn new(
    base_style: &ComputedStyle,
    stylesheet: &'s StyleSheet,
    theme_keyframes: &[&'static KeyframesRule],
    time: u64,
    viewport: Viewport,
  ) -> Option<Self> {
    let animations: Vec<ActiveAnimation> =
      active_animations(base_style, stylesheet, theme_keyframes, time, viewport).collect();

    (!animations.is_empty()).then_some(Self { animations })
  }

  /// The longhands a keyframe of an animation declares.
  pub(crate) fn longhands(&self) -> PropertyMask {
    let mut longhands = PropertyMask::new();

    for animation in &self.animations {
      longhands.union(&animation.longhands);
    }
    longhands
  }

  /// `style` with the animations of `longhands` applied, a keyframe that leaves a longhand out
  /// taking its value from `style` with the earlier animations applied.
  pub(crate) fn apply(
    &self,
    style: &mut ComputedStyle,
    longhands: &PropertyMask,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    let mut animations = self
      .animations
      .iter()
      .filter(|animation| animation.longhands.intersects(longhands))
      .peekable();

    if animations.peek().is_none() {
      return;
    }

    for animation in animations {
      let underlying = style.clone();
      let keyframes = ResolvedKeyframes::new(animation.keyframes, &underlying);
      let mut animated = animation.longhands;

      animated.intersect(longhands);
      for interval in keyframes.intervals(&animated, animation.sample.progress) {
        let eased_progress = keyframes
          .timing_function(interval.from)
          .unwrap_or(animation.timing_function)
          .apply(interval.progress, animation.sample.before);

        style.apply_interpolated_properties(
          keyframes.style(interval.from, &underlying),
          keyframes.style(interval.to, &underlying),
          &interval.longhands,
          eased_progress,
          sizing,
          current_color,
        );
      }
    }
  }

  /// The custom properties the animations set on `style`, each after the earlier animations that
  /// set it: a registered one interpolated as its syntax types it, any other switching half way,
  /// as css-variables-1 animates one.
  /// <https://drafts.css-houdini.org/css-properties-values-api-1/#animation-behavior-of-custom-properties>
  pub(crate) fn custom_properties(
    &self,
    style: &ComputedStyle,
    sizing: &SizingContext,
    current_color: Color,
  ) -> Vec<(String, String)> {
    let mut animated: Vec<(String, String)> = Vec::new();

    for animation in &self.animations {
      for name in animation.keyframes.custom_property_names() {
        let underlying = animated
          .iter()
          .rev()
          .find(|(animated_name, _)| animated_name == name)
          .map(|(_, value)| value.as_str())
          .or_else(|| style.custom_properties.get(name));
        let (from, to, progress, timing_function) =
          animation
            .keyframes
            .custom_property_segment(name, underlying, animation.sample.progress);
        let resolve = |value: Option<&str>| {
          value.map(|value| {
            resolve_var_references(value, &style.custom_properties)
              .unwrap_or_else(|| value.to_owned())
          })
        };
        let (from, to) = (resolve(from), resolve(to));
        let progress = timing_function
          .unwrap_or(animation.timing_function)
          .apply(progress, animation.sample.before);
        let interpolated = from.as_deref().zip(to.as_deref()).and_then(|(from, to)| {
          style
            .custom_properties
            .registration(name)?
            .syntax
            .interpolate(from, to, progress, sizing, current_color)
        });
        // An endpoint no keyframe and no underlying value gives is the guaranteed-invalid value,
        // which `initial` declares.
        let value = interpolated
          .or_else(|| discrete(&from, &to, progress))
          .unwrap_or_else(|| "initial".to_owned());

        animated.push((name.to_owned(), value));
      }
    }

    animated
  }
}

/// An animation `animation-name` runs that is sampled at the render's time.
struct ActiveAnimation<'s> {
  keyframes: &'s KeyframesRule,
  /// The longhands its keyframes declare.
  longhands: PropertyMask,
  sample: AnimationSample,
  timing_function: AnimationTimingFunction,
}

/// The animations `base_style` names that `time` falls in or fills, in `animation-name` order.
fn active_animations<'s>(
  base_style: &ComputedStyle,
  stylesheet: &'s StyleSheet,
  theme_keyframes: &[&'static KeyframesRule],
  time: u64,
  viewport: Viewport,
) -> impl Iterator<Item = ActiveAnimation<'s>> {
  let animation = &base_style.misc1_data;

  animation
    .animation_name
    .iter()
    .enumerate()
    .filter_map(move |(index, name)| {
      let keyframes = find_keyframes(stylesheet, theme_keyframes, name.as_ref()?, viewport)?;
      let AnimationDuration(duration) = repeated_list_value(&animation.animation_duration, index);
      let delay: AnimationTime = repeated_list_value(&animation.animation_delay, index);
      // Blink pauses a paused CSS animation as it creates it, holding its current time at 0.
      // <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/animation/css/css_animations.cc>
      let current_time = match repeated_list_value(&animation.animation_play_state, index) {
        AnimationPlayState::Running => time as f32,
        AnimationPlayState::Paused => 0.0,
      };
      let sample = sample_animation_progress(
        current_time,
        duration.milliseconds,
        delay.milliseconds,
        repeated_list_value(&animation.animation_iteration_count, index),
        repeated_list_value(&animation.animation_direction, index),
        repeated_list_value(&animation.animation_fill_mode, index),
      )?;

      Some(ActiveAnimation {
        longhands: keyframes.longhands(),
        keyframes,
        sample,
        timing_function: repeated_list_value(&animation.animation_timing_function, index),
      })
    })
}

/// The last `@keyframes` named `name` in `stylesheet` whose media queries match, else the one
/// in `theme_keyframes`.
pub(crate) fn find_keyframes<'a>(
  stylesheet: &'a StyleSheet,
  theme_keyframes: &[&'static KeyframesRule],
  name: &str,
  viewport: Viewport,
) -> Option<&'a KeyframesRule> {
  stylesheet
    .keyframes
    .iter()
    .rev()
    .find(|rule| {
      rule.name == name
        && rule
          .media_queries
          .iter()
          .all(|media_query| media_query.matches(viewport))
    })
    .or_else(|| {
      theme_keyframes
        .iter()
        .find(|rule| rule.name == name)
        .copied()
    })
}

impl KeyframeRule {
  /// The longhands its declarations set.
  fn longhands(&self) -> PropertyMask {
    let mut longhands = PropertyMask::new();

    for declaration in self.declarations.iter() {
      longhands.union(&declaration.affected_longhands());
    }
    longhands
  }

  /// The `animation-timing-function` it declares, which eases the interval it starts.
  /// <https://drafts.csswg.org/css-animations-1/#keyframes>
  fn timing_function(&self) -> Option<AnimationTimingFunction> {
    self
      .declarations
      .iter()
      .rev()
      .find_map(|declaration| match declaration {
        StyleDeclaration::AnimationTimingFunction(timing_functions) => {
          Some(repeated_list_value(timing_functions, 0))
        }
        _ => None,
      })
  }
}

impl KeyframesRule {
  /// The names of the custom properties its keyframes declare, each once.
  fn custom_property_names(&self) -> Vec<&str> {
    let mut seen = Xxh3HashSet::default();

    self
      .custom_properties()
      .filter(|name| seen.insert(*name))
      .collect()
  }

  /// The values on either side of `progress` of the keyframes that declare the custom property
  /// `name`, the `underlying` value standing in for an offset of 0 or 1 none declares, the
  /// progress between them, and the timing function the earlier keyframe declares.
  fn custom_property_segment<'k>(
    &'k self,
    name: &str,
    underlying: Option<&'k str>,
    progress: f32,
  ) -> (
    Option<&'k str>,
    Option<&'k str>,
    f32,
    Option<AnimationTimingFunction>,
  ) {
    let mut points: Vec<(f32, Option<(&str, &KeyframeRule)>)> = self
      .keyframes
      .iter()
      .filter_map(|keyframe| {
        let value =
          keyframe
            .declarations
            .iter()
            .rev()
            .find_map(|declaration| match declaration {
              StyleDeclaration::CustomProperty(declared, value) if declared == name => {
                Some(value.as_str())
              }
              _ => None,
            })?;

        Some(
          keyframe
            .offsets
            .iter()
            .map(move |&offset| (offset, Some((value, keyframe)))),
        )
      })
      .flatten()
      .collect();

    sort_by_key(&mut points, |&(offset, _)| offset);

    let (from, to, local) = property_interval(points, progress);

    (
      from.map(|(value, _)| value).or(underlying),
      to.map(|(value, _)| value).or(underlying),
      local,
      from.and_then(|(_, keyframe)| keyframe.timing_function()),
    )
  }

  /// The longhands its keyframes declare.
  fn longhands(&self) -> PropertyMask {
    let mut longhands = PropertyMask::new();

    for keyframe in &self.keyframes {
      longhands.union(&keyframe.longhands());
    }
    longhands
  }

  /// The name of each custom property declaration in its keyframes.
  fn custom_properties(&self) -> impl Iterator<Item = &str> {
    self
      .keyframes
      .iter()
      .flat_map(|keyframe| keyframe.declarations.iter())
      .filter_map(|declaration| match declaration {
        StyleDeclaration::CustomProperty(name, _) => Some(name.as_str()),
        _ => None,
      })
  }
}

/// Tailwind's theme `@keyframes`, which it emits once a utility's `animation` names them.
/// <https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/theme.css>
///
/// Approximate: Tailwind's `spin` is one `to { transform: rotate(360deg) }`. An [`Angle`] keeps
/// its value in [0, 360) and interpolates the shorter way round, so `spin` turns through quarter
/// keyframes instead, which match Tailwind only under the `linear` easing `animate-spin` uses.
const TAILWIND_THEME_KEYFRAMES: &str = r"
@keyframes spin {
  25% {
    transform: rotate(90deg);
  }

  50% {
    transform: rotate(180deg);
  }

  75% {
    transform: rotate(270deg);
  }

  to {
    transform: rotate(360deg);
  }
}

@keyframes ping {
  75%,
  100% {
    transform: scale(2);
    opacity: 0;
  }
}

@keyframes pulse {
  50% {
    opacity: 0.5;
  }
}

@keyframes bounce {
  0%,
  100% {
    transform: translateY(-25%);
    animation-timing-function: cubic-bezier(0.8, 0, 1, 1);
  }

  50% {
    transform: none;
    animation-timing-function: cubic-bezier(0, 0, 0.2, 1);
  }
}
";

/// The theme `@keyframes` Tailwind emits for the `animate-<token>` utility.
pub(crate) fn tailwind_theme_keyframes(token: &str) -> Option<&'static KeyframesRule> {
  static KEYFRAMES: LazyLock<Vec<KeyframesRule>> =
    LazyLock::new(|| StyleSheet::parse_loosy(TAILWIND_THEME_KEYFRAMES).keyframes);

  KEYFRAMES.iter().find(|rule| rule.name == token)
}

/// One sampled point on an animation's timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimationSample {
  /// Directed progress through the current iteration.
  progress: f32,
  /// Whether a step easing function sitting exactly on a jump takes the lower step, which Blink
  /// calls the left limit direction.
  before: bool,
}

/// Where a sample falls against an animation's active interval.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AnimationPhase {
  Before,
  Active,
  After,
}

/// The sample of an animation at `time_ms`, or `None` when it neither runs nor fills then, as
/// Blink's `Timing::CalculateTimings` samples it.
/// <https://w3.org/TR/web-animations-1/#core-animation-model-calculations>
fn sample_animation_progress(
  time_ms: f32,
  duration_ms: f32,
  delay_ms: f32,
  iteration_count: AnimationIterationCount,
  direction: AnimationDirection,
  fill_mode: AnimationFillMode,
) -> Option<AnimationSample> {
  let iteration_count = match iteration_count {
    AnimationIterationCount::Infinite => f32::INFINITY,
    AnimationIterationCount::Number(count) => count.max(0.0),
  };
  let duration_ms = duration_ms.max(0.0);
  let active_duration = if duration_ms == 0.0 || iteration_count == 0.0 {
    0.0
  } else {
    duration_ms * iteration_count
  };
  let local_active_time = time_ms - delay_ms;
  let fills_backwards = matches!(
    fill_mode,
    AnimationFillMode::Backwards | AnimationFillMode::Both
  );
  let fills_forwards = matches!(
    fill_mode,
    AnimationFillMode::Forwards | AnimationFillMode::Both
  );
  let (phase, active_time) = if local_active_time < 0.0 {
    (AnimationPhase::Before, fills_backwards.then_some(0.0)?)
  } else if local_active_time >= active_duration {
    (
      AnimationPhase::After,
      fills_forwards.then_some(active_duration)?,
    )
  } else {
    (AnimationPhase::Active, local_active_time)
  };

  let overall_progress = match phase {
    _ if duration_ms > 0.0 => active_time / duration_ms,
    AnimationPhase::Before => 0.0,
    _ => iteration_count,
  };
  let mut simple_progress = if overall_progress.is_infinite() {
    0.0
  } else {
    overall_progress.fract()
  };

  if simple_progress <= f32::EPSILON && phase == AnimationPhase::After && iteration_count != 0.0 {
    simple_progress = 1.0;
  }

  let current_iteration = if phase == AnimationPhase::After && iteration_count.is_infinite() {
    f32::INFINITY
  } else if simple_progress == 1.0 {
    (overall_progress.floor() - 1.0).max(0.0)
  } else {
    overall_progress.floor()
  };
  let forwards = is_current_direction_forwards(direction, current_iteration);
  let mut progress = if forwards {
    simple_progress
  } else {
    1.0 - simple_progress
  };

  if phase == AnimationPhase::After {
    progress = match forwards {
      true if (progress - 1.0).abs() <= f32::EPSILON => 1.0,
      false if progress.abs() <= f32::EPSILON => 0.0,
      _ => progress,
    };
  }

  Some(AnimationSample {
    progress,
    before: if forwards {
      phase == AnimationPhase::Before
    } else {
      phase == AnimationPhase::After
    },
  })
}

/// Whether `direction` plays the iteration at `current_iteration` forwards.
fn is_current_direction_forwards(direction: AnimationDirection, current_iteration: f32) -> bool {
  let even = current_iteration.is_infinite() || current_iteration % 2.0 == 0.0;

  match direction {
    AnimationDirection::Normal => true,
    AnimationDirection::Reverse => false,
    AnimationDirection::Alternate => even,
    AnimationDirection::AlternateReverse => !even,
  }
}

/// The points on either side of `progress` among `points`, sorted by offset, `None` standing in for
/// the underlying value at an offset of 0 or 1 no point takes, and the progress between them, as
/// Blink samples each property's keyframes apart from the others.
/// <https://drafts.csswg.org/web-animations-1/#the-effect-value-of-a-keyframe-animation-effect>
fn property_interval<T: Copy>(
  mut points: Vec<(f32, Option<T>)>,
  progress: f32,
) -> (Option<T>, Option<T>, f32) {
  if points.first().is_none_or(|&(offset, _)| offset > 0.0) {
    points.insert(0, (0.0, None));
  }
  if points.last().is_none_or(|&(offset, _)| offset < 1.0) {
    points.push((1.0, None));
  }

  let start = points
    .iter()
    .rposition(|&(offset, _)| offset <= progress && offset < 1.0)
    .unwrap_or(0);
  let ((from_offset, from), (to_offset, to)) = (points[start], points[start + 1]);
  let width = to_offset - from_offset;
  let local = if width <= f32::EPSILON {
    1.0
  } else {
    ((progress - from_offset) / width).clamp(0.0, 1.0)
  };

  (from, to, local)
}

/// A keyframe's style and the longhands its declarations animate.
#[derive(Debug)]
struct ResolvedKeyframe {
  offset: f32,
  style: ComputedStyle,
  mask: PropertyMask,
  timing_function: Option<AnimationTimingFunction>,
}

impl ResolvedKeyframe {
  /// Approximate: a CSS-wide keyword in a keyframe keeps the base style's value, where Blink rolls
  /// `revert` back to the user-agent origin and resolves `inherit` and `unset` against the parent.
  fn apply(&mut self, keyframe: &KeyframeRule) {
    for declaration in keyframe.declarations.iter() {
      declaration.apply_to_computed(&mut self.style);
      self.mask.extend(declaration.affected_longhands().iter());
    }
  }
}

/// An animation's keyframes in offset order, each over the base style, as Blink's
/// `CreateKeyframeEffectModel` builds them: those at one offset with one timing function merged,
/// and a longhand at an offset taken from the last keyframe that declares it there.
#[derive(Debug)]
struct ResolvedKeyframes {
  frames: Vec<ResolvedKeyframe>,
}

impl ResolvedKeyframes {
  fn new(keyframes: &KeyframesRule, base_style: &ComputedStyle) -> Self {
    let mut points = keyframes
      .keyframes
      .iter()
      .flat_map(|keyframe| {
        keyframe
          .offsets
          .iter()
          .map(move |&offset| (offset, keyframe))
      })
      .collect::<Vec<_>>();

    sort_by_key(&mut points, |&(offset, _)| offset);

    let mut frames: Vec<ResolvedKeyframe> = Vec::with_capacity(points.len());

    for (offset, keyframe) in points {
      let timing_function = keyframe.timing_function();
      let longhands = keyframe.longhands();
      let mut matching = None;

      for frame in frames
        .iter_mut()
        .rev()
        .take_while(|frame| (frame.offset - offset).abs() <= f32::EPSILON)
      {
        frame.mask.difference(&longhands);
        if matching.is_none() && frame.timing_function == timing_function {
          matching = Some(frame);
        }
      }

      match matching {
        Some(frame) => frame.apply(keyframe),
        None => {
          let mut frame = ResolvedKeyframe {
            offset,
            style: base_style.clone(),
            mask: PropertyMask::new(),
            timing_function,
          };

          frame.apply(keyframe);
          frames.push(frame);
        }
      }
    }

    Self { frames }
  }

  /// The keyframe intervals `progress` falls in for `longhands`, each with the longhands it holds.
  fn intervals(&self, longhands: &PropertyMask, progress: f32) -> Vec<KeyframeInterval> {
    let mut intervals: Vec<KeyframeInterval> = Vec::new();

    for longhand in longhands.iter() {
      let points = self
        .frames
        .iter()
        .enumerate()
        .filter(|(_, frame)| frame.mask.contains(&longhand))
        .map(|(index, frame)| (frame.offset, Some(index)))
        .collect();
      let (from, to, progress) = property_interval(points, progress);

      match intervals
        .iter_mut()
        .find(|interval| interval.from == from && interval.to == to)
      {
        Some(interval) => {
          interval.longhands.insert(longhand);
        }
        None => intervals.push(KeyframeInterval {
          from,
          to,
          progress,
          longhands: [longhand].into_iter().collect(),
        }),
      }
    }

    intervals
  }

  /// The timing function the keyframe at `index` declares, which an implicit keyframe has none of.
  fn timing_function(&self, index: Option<usize>) -> Option<AnimationTimingFunction> {
    index.and_then(|index| self.frames[index].timing_function)
  }

  /// The style of the keyframe at `index`, or `base_style` for an implicit keyframe.
  fn style<'a>(&'a self, index: Option<usize>, base_style: &'a ComputedStyle) -> &'a ComputedStyle {
    index.map_or(base_style, |index| &self.frames[index].style)
  }
}

/// The keyframes on either side of an animation's progress that some longhands share.
#[derive(Debug)]
struct KeyframeInterval {
  from: Option<usize>,
  to: Option<usize>,
  progress: f32,
  longhands: PropertyMask,
}

macro_rules! impl_passthrough_animatable {
  ($($ty:ty),* $(,)?) => {
    $(
      impl Animatable for $ty {}
    )*
  };
}

impl_passthrough_animatable!(
  AnimationNames,
  AnimationDurations,
  AnimationDelays,
  AnimationTimingFunctions,
  AnimationIterationCounts,
  AlignItems,
  JustifyContent,
  Border,
  GridAutoFlow,
  GridLine,
  GridTemplateAreas,
  TextOverflow,
  FontStyle,
  FontFamily,
  LineHeight,
  FontSynthesis,
  LineClamp,
  TextStroke,
  TextDecoration,
  TextDecorationLines,
  BasicShape,
  OffsetPath,
  OffsetAnchor,
  OffsetPosition,
  WhiteSpace,
  TextWrap,
  VerticalAlign,
  Flex,
  Background,
  GridTrackSize,
  GridTemplateComponent,
  FontFeature,
  FontVariation,
);

impl Animatable for Length {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    _current_color: Color,
  ) {
    *self = interpolate_length(*from, *to, progress)
      .or_else(|| {
        resolve_length_with_sizing(*from, sizing).and_then(|resolved_from| {
          resolve_length_with_sizing(*to, sizing)
            .map(|resolved_to| Length::Px(lerp(resolved_from, resolved_to, progress)))
        })
      })
      .unwrap_or_else(|| discrete(from, to, progress));
  }
}

fn interpolate_length(from: Length, to: Length, progress: f32) -> Option<Length> {
  macro_rules! lerp_variants {
    ($($variant:ident),+ $(,)?) => {
      match (from, to) {
        $(
          (Length::$variant(lhs), Length::$variant(rhs)) => {
            Some(Length::$variant(lerp(lhs, rhs, progress)))
          }
        )+
        (Length::Auto, Length::Auto) => Some(Length::Auto),
        _ => None,
      }
    };
  }
  lerp_variants!(
    Percentage, Rem, Em, Vh, Vw, CqH, CqW, CqMin, CqMax, VMin, VMax, Cm, Mm, In, Q, Pt, Pc, Px,
  )
}

fn resolve_length_with_sizing(value: Length, sizing: &SizingContext) -> Option<f32> {
  if matches!(value, Length::Auto) {
    return None;
  }

  Some(value.to_px(
    sizing,
    sizing.viewport.size.width.unwrap_or_default() as f32,
  ))
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use crate::{
    style::{
      SizingContext,
      animation::{find_keyframes, sample_animation_progress, tailwind_theme_keyframes},
      *,
    },
    viewport::Viewport,
  };

  #[test]
  fn stylesheet_animation_interpolates_corner_shape() {
    let stylesheet = StyleSheet::parse(
      r#"
      @keyframes morph {
        from { corner-shape: squircle; }
        to { corner-shape: bevel; }
      }
      "#,
    )
    .unwrap();

    let style = ComputedStyle {
      misc1_data: Rc::new(StyleMisc1Data {
        animation_name: AnimationNames::from_css_str("morph").unwrap(),
        animation_duration: AnimationDurations::from_css_str("1000ms").unwrap(),
        animation_timing_function: AnimationTimingFunctions::from_css_str("linear").unwrap(),
        animation_fill_mode: AnimationFillModes::from_css_str("both").unwrap(),
        ..Default::default()
      }),
      ..Default::default()
    };

    let at = |time: u64| {
      animate(style.clone(), &stylesheet, time)
        .surround_data
        .corner_top_left_shape
    };

    assert_eq!(at(0), Superellipse::SQUIRCLE);
    assert_eq!(at(1000), Superellipse::BEVEL);

    let midpoint = at(500);

    assert!(
      midpoint.0 > 0.0 && midpoint.0 < 2.0,
      "expected value between bevel and squircle, got {}",
      midpoint.0
    );
  }

  /// At its start an animation samples the interval from its first keyframe, so a step that jumps
  /// at the start shows its first step there.
  /// <https://drafts.csswg.org/web-animations-1/#the-effect-value-of-a-keyframe-animation-effect>
  #[test]
  fn a_start_step_jumps_at_the_first_keyframe() {
    let stylesheet =
      StyleSheet::parse("@keyframes fade { from { opacity: 0 } to { opacity: 1 } }").unwrap();
    let style = ComputedStyle {
      misc1_data: Rc::new(StyleMisc1Data {
        animation_name: AnimationNames::from_css_str("fade").unwrap(),
        animation_duration: AnimationDurations::from_css_str("1000ms").unwrap(),
        animation_timing_function: AnimationTimingFunctions::from_css_str("steps(2, start)")
          .unwrap(),
        ..Default::default()
      }),
      ..Default::default()
    };
    let animated = animate(style, &stylesheet, 0);

    assert_eq!(animated.svg_data.opacity.0, 0.5);
  }

  /// A paused animation holds the frame at its start time, whatever the render's time.
  #[test]
  fn a_paused_animation_holds_its_start_time() {
    let (style, stylesheet) = linear_animation("from { opacity: 0 } to { opacity: 1 }");
    let paused = |delay: &str| {
      let mut style = style.clone();
      let animation = Rc::make_mut(&mut style.misc1_data);

      animation.animation_play_state = AnimationPlayStates::from_css_str("paused").unwrap();
      animation.animation_delay = AnimationDelays::from_css_str(delay).unwrap();
      animate(style, &stylesheet, 500).svg_data.opacity.0
    };

    assert_eq!(paused("0ms"), 0.0);
    assert_eq!(paused("-250ms"), 0.25);
  }

  /// A step easing takes its lower step on the side the iteration's direction plays away from:
  /// before the start played forwards, after the end played in reverse.
  /// <https://w3.org/TR/web-animations-1/#calculating-the-transformed-progress>
  #[test]
  fn a_reversed_step_takes_its_limit_from_the_playback_direction() {
    let (style, stylesheet) = linear_animation("from { opacity: 0 } to { opacity: 1 }");
    let reversed = |timing: &str, delay: &str, fill: &str, time| {
      let mut style = style.clone();
      let animation = Rc::make_mut(&mut style.misc1_data);

      animation.animation_timing_function = AnimationTimingFunctions::from_css_str(timing).unwrap();
      animation.animation_delay = AnimationDelays::from_css_str(delay).unwrap();
      animation.animation_fill_mode = AnimationFillModes::from_css_str(fill).unwrap();
      animation.animation_direction = AnimationDirections::from_css_str("reverse").unwrap();
      animate(style, &stylesheet, time).svg_data.opacity.0
    };

    assert_eq!(reversed("steps(2, start)", "0ms", "forwards", 1500), 0.0);
    assert_eq!(reversed("steps(2)", "500ms", "backwards", 0), 1.0);
  }

  /// A linear 1000ms animation of `keyframes` on a default style.
  fn linear_animation(keyframes: &str) -> (ComputedStyle, StyleSheet) {
    let stylesheet = StyleSheet::parse(&format!("@keyframes k {{ {keyframes} }}")).unwrap();
    let style = ComputedStyle {
      misc1_data: Rc::new(StyleMisc1Data {
        animation_name: AnimationNames::from_css_str("k").unwrap(),
        animation_duration: AnimationDurations::from_css_str("1000ms").unwrap(),
        animation_timing_function: AnimationTimingFunctions::from_css_str("linear").unwrap(),
        ..Default::default()
      }),
      ..Default::default()
    };

    (style, stylesheet)
  }

  /// A keyframe that leaves a property out is skipped for it, so the property interpolates between
  /// the keyframes that declare it.
  /// <https://drafts.csswg.org/web-animations-1/#the-effect-value-of-a-keyframe-animation-effect>
  #[test]
  fn a_property_skips_the_keyframes_that_leave_it_out() {
    let (style, stylesheet) = linear_animation(
      "0% { opacity: 0; rotate: 0deg } 50% { rotate: 90deg } 100% { opacity: 1; rotate: 180deg }",
    );
    let animated = animate(style, &stylesheet, 250);

    assert_eq!(animated.svg_data.opacity.0, 0.25);
    assert_eq!(animated.misc3_data.rotate, Some(Angle::new(45.0)));
  }

  /// A keyframe's `animation-timing-function` eases the interval it starts.
  /// <https://drafts.csswg.org/css-animations-1/#keyframes>
  #[test]
  fn a_keyframe_timing_function_eases_the_interval_it_starts() {
    let (style, stylesheet) = linear_animation(
      "from { opacity: 0; animation-timing-function: steps(1, end) } 50% { opacity: 0.5 } to { opacity: 1 }",
    );
    let at = |time| animate(style.clone(), &stylesheet, time).svg_data.opacity.0;

    assert_eq!(at(250), 0.0);
    assert_eq!(at(750), 0.75);
  }

  /// Keyframes at one offset with different timing functions stay apart, so each eases only the
  /// longhands it declares.
  #[test]
  fn a_keyframe_timing_function_eases_only_its_own_declarations() {
    let (style, stylesheet) = linear_animation(
      "0% { opacity: 0 } 0% { rotate: 0deg; animation-timing-function: steps(1, end) } 100% { opacity: 1; rotate: 90deg }",
    );
    let animated = animate(style, &stylesheet, 500);

    assert_eq!(animated.svg_data.opacity.0, 0.5);
    assert_eq!(animated.misc3_data.rotate, Some(Angle::new(0.0)));
  }

  /// A keyframe offset an animation leaves out takes the value the animations before it in the
  /// stack give, as Blink composites a neutral keyframe onto the underlying value.
  /// <https://drafts.csswg.org/web-animations-1/#the-effect-value-of-a-keyframe-animation-effect>
  #[test]
  fn a_missing_endpoint_takes_the_earlier_animations_value() {
    let stylesheet = StyleSheet::parse(
      "@keyframes a { from, to { opacity: 0.2 } } @keyframes b { to { opacity: 0.6 } }",
    )
    .unwrap();
    let style = ComputedStyle {
      misc1_data: Rc::new(StyleMisc1Data {
        animation_name: AnimationNames::from_css_str("a, b").unwrap(),
        animation_duration: AnimationDurations::from_css_str("1s").unwrap(),
        animation_timing_function: AnimationTimingFunctions::from_css_str("linear").unwrap(),
        animation_fill_mode: AnimationFillModes::from_css_str("both").unwrap(),
        ..Default::default()
      }),
      ..Default::default()
    };
    let at = |time| animate(style.clone(), &stylesheet, time).svg_data.opacity.0;

    assert_eq!(at(0), 0.2);
    assert!((at(500) - 0.4).abs() < 1e-6);
  }

  #[test]
  fn a_missing_custom_property_endpoint_takes_the_earlier_animations_value() {
    let stylesheet =
      StyleSheet::parse("@keyframes a { from, to { --x: red } } @keyframes b { to { --x: blue } }")
        .unwrap();
    let style = ComputedStyle {
      misc1_data: Rc::new(StyleMisc1Data {
        animation_name: AnimationNames::from_css_str("a, b").unwrap(),
        animation_duration: AnimationDurations::from_css_str("1s").unwrap(),
        animation_timing_function: AnimationTimingFunctions::from_css_str("linear").unwrap(),
        ..Default::default()
      }),
      ..Default::default()
    };
    let animations = ElementAnimations::new(&style, &stylesheet, &[], 250, sizing().viewport)
      .expect("an animation runs");
    let animated = animations.custom_properties(&style, &sizing(), current_color());

    assert_eq!(animated.last(), Some(&("--x".to_owned(), "red".to_owned())));
  }

  /// `style` with every animation it runs at `time` applied.
  fn animate(style: ComputedStyle, stylesheet: &StyleSheet, time: u64) -> ComputedStyle {
    let animations = ElementAnimations::new(&style, stylesheet, &[], time, sizing().viewport)
      .expect("an animation runs");
    let longhands = animations.longhands();

    let mut style = style;

    animations.apply(&mut style, &longhands, &sizing(), current_color());
    style
  }

  fn sizing() -> SizingContext {
    SizingContext::builder()
      .viewport(Viewport::new((200, 100)))
      .font_size(16.0)
      .line_height(0.0)
      .ch(16.0 / 2.0)
      .ex(16.0 / 2.0)
      .build()
  }

  fn current_color() -> Color {
    Color([10, 20, 30, 255])
  }

  #[derive(Clone, Copy, Debug, PartialEq)]
  struct Dummy(u8);

  impl Animatable for Dummy {}

  #[test]
  fn animatable_default_flips_at_half_progress() {
    let mut target = Dummy(9);
    target.interpolate(&Dummy(3), &Dummy(7), 0.25, &sizing(), current_color());
    assert_eq!(target, Dummy(3));

    target.interpolate(&Dummy(3), &Dummy(7), 0.5, &sizing(), current_color());
    assert_eq!(target, Dummy(7));
  }

  #[test]
  fn animation_keyword_lists_remain_discrete() {
    fn interpolate<T: Animatable>(from: T, to: T, progress: f32) -> T {
      let mut value = from.clone();

      value.interpolate(&from, &to, progress, &sizing(), current_color());
      value
    }

    macro_rules! assert_discrete_list {
      ($type:ty; [$($from:expr),*]; [$($to:expr),*]) => {
        let from: $type = Box::from([$($from),*]);
        let to: $type = Box::from([$($to),*]);

        assert_eq!(interpolate(from.clone(), to.clone(), 0.49), from);
        assert_eq!(interpolate(from.clone(), to.clone(), 0.5), to);
      };
    }

    assert_discrete_list!(
      AnimationDirections;
      [AnimationDirection::Normal, AnimationDirection::Reverse];
      [AnimationDirection::Alternate, AnimationDirection::AlternateReverse]
    );
    assert_discrete_list!(
      AnimationDirections;
      [AnimationDirection::Normal];
      [AnimationDirection::Reverse, AnimationDirection::Alternate]
    );
    assert_discrete_list!(
      AnimationFillModes;
      [AnimationFillMode::None, AnimationFillMode::Forwards];
      [AnimationFillMode::Backwards, AnimationFillMode::Both]
    );
    assert_discrete_list!(
      AnimationFillModes;
      [AnimationFillMode::None];
      [AnimationFillMode::Forwards, AnimationFillMode::Both]
    );
    assert_discrete_list!(
      AnimationPlayStates;
      [AnimationPlayState::Running, AnimationPlayState::Paused];
      [AnimationPlayState::Paused, AnimationPlayState::Running]
    );
    assert_discrete_list!(
      AnimationPlayStates;
      [AnimationPlayState::Running];
      [AnimationPlayState::Paused, AnimationPlayState::Running]
    );
  }

  #[test]
  fn blend_modes_repeat_to_lcm() {
    let from: BlendModes = Box::from([BlendMode::Normal, BlendMode::Multiply]);
    let to: BlendModes = Box::from([BlendMode::Screen, BlendMode::Overlay, BlendMode::Darken]);
    let mut value = from.clone();

    value.interpolate(&from, &to, 0.5, &sizing(), current_color());

    assert_eq!(
      value.as_ref(),
      [
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
      ]
    );
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
    let mut target: PositionValue = PositionValue::default();
    target.interpolate(
      &PositionValue(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Left),
        PositionComponent::KeywordY(PositionKeywordY::Top),
      )),
      &PositionValue(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Right),
        PositionComponent::KeywordY(PositionKeywordY::Bottom),
      )),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(
      target,
      PositionValue(SpacePair::from_pair(
        PositionComponent::Length(Length::Percentage(50.0)),
        PositionComponent::Length(Length::Percentage(50.0)),
      ))
    );
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
  fn scale_interpolates_from_none_as_identity() {
    let mut target: Option<SpacePair<PercentageNumber>> = None;
    target.interpolate(
      &None,
      &Some(SpacePair::from_single(PercentageNumber(2.0))),
      0.5,
      &sizing(),
      current_color(),
    );

    assert_eq!(target, Some(SpacePair::from_single(PercentageNumber(1.5))));
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
  fn backwards_fill_samples_from_before_the_active_phase() {
    let sample = sample_animation_progress(
      0.0,
      1000.0,
      500.0,
      AnimationIterationCount::Number(1.0),
      AnimationDirection::Normal,
      AnimationFillMode::Backwards,
    )
    .expect("backwards fill applies during the delay");

    assert_eq!(sample.progress, 0.0);
    assert!(sample.before);
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

    assert_eq!(progress.map(|sample| sample.progress), Some(1.0));
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

    assert_eq!(progress.map(|sample| sample.progress), Some(0.0));
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

    assert_eq!(progress.map(|sample| sample.progress), Some(0.5));
  }

  /// <https://w3.org/TR/web-animations-1/#calculating-the-simple-iteration-progress>
  #[test]
  fn animation_progress_starts_the_next_iteration_at_an_inner_boundary() {
    let progress = sample_animation_progress(
      1000.0,
      1000.0,
      0.0,
      AnimationIterationCount::Infinite,
      AnimationDirection::Normal,
      AnimationFillMode::Both,
    );

    assert_eq!(progress.map(|sample| sample.progress), Some(0.0));
  }

  /// <https://w3.org/TR/web-animations-1/#calculating-the-simple-iteration-progress>
  #[test]
  fn a_zero_iteration_animation_fills_forwards_with_its_start() {
    let sample = |direction| {
      sample_animation_progress(
        500.0,
        1000.0,
        0.0,
        AnimationIterationCount::Number(0.0),
        direction,
        AnimationFillMode::Forwards,
      )
      .map(|sample| sample.progress)
    };

    assert_eq!(sample(AnimationDirection::Normal), Some(0.0));
    assert_eq!(sample(AnimationDirection::Reverse), Some(1.0));
  }

  #[test]
  fn a_zero_duration_animation_fills_only_forwards() {
    let sample = |fill_mode| {
      sample_animation_progress(
        0.0,
        0.0,
        0.0,
        AnimationIterationCount::Number(1.0),
        AnimationDirection::Normal,
        fill_mode,
      )
      .map(|sample| sample.progress)
    };

    assert_eq!(sample(AnimationFillMode::None), None);
    assert_eq!(sample(AnimationFillMode::Backwards), None);
    assert_eq!(sample(AnimationFillMode::Forwards), Some(1.0));
    assert_eq!(sample(AnimationFillMode::Both), Some(1.0));
  }

  /// Blink's `StyleResolver::FindKeyframesRule` matches a keyframes name exactly.
  #[test]
  fn a_keyframes_name_matches_case_sensitively() {
    let stylesheet = StyleSheet::parse("@keyframes Fade { from { opacity: 0 } }").unwrap();
    let viewport = sizing().viewport;

    assert!(find_keyframes(&stylesheet, &[], "Fade", viewport).is_some());
    assert!(find_keyframes(&stylesheet, &[], "fade", viewport).is_none());
  }

  /// Tailwind emits its theme `@keyframes` only for a utility that reads them, so a stylesheet's
  /// `animation: spin` without them runs nothing.
  #[test]
  fn theme_keyframes_apply_only_once_a_utility_reads_them() {
    let stylesheet = StyleSheet::default();
    let viewport = sizing().viewport;
    let spin = tailwind_theme_keyframes("spin").expect("Tailwind's theme has spin");

    assert!(find_keyframes(&stylesheet, &[], "spin", viewport).is_none());
    assert_eq!(
      find_keyframes(&stylesheet, &[spin], "spin", viewport),
      Some(spin)
    );
    assert!(tailwind_theme_keyframes("wiggle").is_none());
  }

  /// Tailwind's `spin` turns `transform` a full circle from its underlying value.
  #[test]
  fn theme_spin_turns_the_transform_a_full_circle() {
    let style = ComputedStyle {
      misc1_data: Rc::new(StyleMisc1Data {
        animation_name: AnimationNames::from_css_str("spin").unwrap(),
        animation_duration: AnimationDurations::from_css_str("1s").unwrap(),
        animation_timing_function: AnimationTimingFunctions::from_css_str("linear").unwrap(),
        ..Default::default()
      }),
      ..Default::default()
    };
    let stylesheet = StyleSheet::default();
    let spin = tailwind_theme_keyframes("spin").unwrap();
    let at = |time| {
      let animations =
        ElementAnimations::new(&style, &stylesheet, &[spin], time, sizing().viewport)
          .expect("spin runs");
      let mut animated = style.clone();

      animations.apply(
        &mut animated,
        &animations.longhands(),
        &sizing(),
        current_color(),
      );
      assert_eq!(animated.misc3_data.rotate, None);
      animated.svg_data.transform.clone()
    };
    let rotated = |degrees| Some(Transforms::from([Transform::Rotate(Angle::new(degrees))]));

    assert_eq!(at(125), rotated(45.0));
    assert_eq!(at(625), rotated(225.0));
    assert_eq!(at(875), rotated(315.0));
  }

  /// Tailwind's `bounce` eases each half with the timing function its keyframes declare.
  #[test]
  fn theme_bounce_eases_each_half_as_its_keyframes_declare() {
    let bounce = tailwind_theme_keyframes("bounce").unwrap();
    let timing_functions: Vec<_> = bounce
      .keyframes
      .iter()
      .map(|keyframe| (keyframe.offsets.clone(), keyframe.timing_function()))
      .collect();

    assert_eq!(
      timing_functions,
      [
        (
          vec![0.0, 1.0],
          Some(AnimationTimingFunction::CubicBezier(0.8, 0.0, 1.0, 1.0))
        ),
        (
          vec![0.5],
          Some(AnimationTimingFunction::CubicBezier(0.0, 0.0, 0.2, 1.0))
        ),
      ]
    );
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
      box_data: Rc::new(StyleBoxData {
        width: Length::Px(10.0).into(),
        height: Length::Px(20.0).into(),
        ..Default::default()
      }),
      ..ComputedStyle::default()
    };
    let from = ComputedStyle {
      box_data: Rc::new(StyleBoxData {
        width: Length::Px(10.0).into(),
        height: Length::Px(100.0).into(),
        ..Default::default()
      }),
      ..ComputedStyle::default()
    };
    let to = ComputedStyle {
      box_data: Rc::new(StyleBoxData {
        width: Length::Px(30.0).into(),
        height: Length::Px(200.0).into(),
        ..Default::default()
      }),
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

    assert_eq!(base_style.box_data.width, Length::Px(20.0).into());
    assert_eq!(base_style.box_data.height, Length::Px(20.0).into());
  }

  #[test]
  fn apply_interpolated_properties_interpolates_rotate_from_implicit_none() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      misc3_data: Rc::new(StyleMisc3Data {
        rotate: Some(Angle::new(45.0)),
        ..Default::default()
      }),
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

    assert_eq!(base_style.misc3_data.rotate, Some(Angle::new(22.5)));
  }

  #[test]
  fn apply_interpolated_properties_interpolates_flex_grow_from_implicit_zero() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      misc1_data: Rc::new(StyleMisc1Data {
        flex_grow: Some(FlexGrow(4.0)),
        ..Default::default()
      }),
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

    assert_eq!(base_style.misc1_data.flex_grow, Some(FlexGrow(2.0)));
  }

  #[test]
  fn apply_interpolated_properties_interpolates_flex_shrink_from_implicit_one() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      misc2_data: Rc::new(StyleMisc2Data {
        flex_shrink: Some(FlexGrow(3.0)),
        ..Default::default()
      }),
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

    assert_eq!(base_style.misc2_data.flex_shrink, Some(FlexGrow(2.0)));
  }

  #[test]
  fn apply_interpolated_properties_interpolates_text_stroke_width_from_implicit_zero() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      misc_inherited1_data: Rc::new(StyleMiscInherited1Data {
        webkit_text_stroke_width: Some(Length::Px(6.0)),
        ..Default::default()
      }),
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

    assert_eq!(
      base_style.misc_inherited1_data.webkit_text_stroke_width,
      Some(Length::Px(3.0))
    );
  }

  #[test]
  fn apply_interpolated_properties_interpolates_text_stroke_color_from_current_color() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle::default();
    let to = ComputedStyle {
      misc_inherited1_data: Rc::new(StyleMiscInherited1Data {
        webkit_text_stroke_color: Some(ColorInput::Value(Color([110, 120, 130, 255]))),
        ..Default::default()
      }),
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
      base_style.misc_inherited1_data.webkit_text_stroke_color,
      Some(ColorInput::Value(Color([60, 70, 80, 255])))
    );
  }

  #[test]
  fn apply_interpolated_properties_interpolates_text_fill_color_from_style_color() {
    let mut base_style = ComputedStyle::default();
    let from = ComputedStyle {
      inherited_data: Rc::new(StyleInheritedData {
        color: ColorInput::Value(Color([20, 40, 60, 255])),
        ..Default::default()
      }),
      ..ComputedStyle::default()
    };
    let to = ComputedStyle {
      inherited_data: Rc::new(StyleInheritedData {
        color: ColorInput::Value(Color([20, 40, 60, 255])),
        ..Default::default()
      }),
      misc_inherited1_data: Rc::new(StyleMiscInherited1Data {
        webkit_text_fill_color: Some(ColorInput::Value(Color([120, 140, 160, 255]))),
        ..Default::default()
      }),
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
      base_style.misc_inherited1_data.webkit_text_fill_color,
      Some(ColorInput::Value(Color([70, 90, 110, 255])))
    );
  }

  #[test]
  fn text_indent_interpolates_amount_continuously() {
    let mut target = TextIndent::default();
    let from = TextIndent {
      amount: Length::Px(10.0),
      each_line: false,
      hanging: false,
    };
    let to = TextIndent {
      amount: Length::Px(30.0),
      each_line: true,
      hanging: true,
    };

    target.interpolate(&from, &to, 0.25, &sizing(), current_color());
    assert_eq!(target.amount, Length::Px(15.0));
    assert!(!target.each_line);
    assert!(!target.hanging);

    target.interpolate(&from, &to, 0.75, &sizing(), current_color());
    assert_eq!(target.amount, Length::Px(25.0));
    assert!(target.each_line);
    assert!(target.hanging);
  }
}
