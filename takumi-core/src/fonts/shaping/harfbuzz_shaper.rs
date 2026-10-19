//! Blink's [`HarfBuzzShaper`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/harfbuzz_shaper.cc).

use std::{borrow::Cow, collections::VecDeque, rc::Rc};

use harfrust::{
  Buffer, Direction as HbDirection, Feature, GlyphInfo, GlyphPosition, Language, ShapeOptions,
  ShaperFont, shape,
};
use icu_properties::{PropertyNamesShort, props::Script};

use super::{
  case_mapping_harfbuzz_buffer_filler::CaseMapIntend,
  run_segmenter::{RunSegmenter, RunSegmenterRange},
  shape_result::ShapeResult,
  shape_result_run::ShapeResultRun,
};
use crate::{
  fonts::{
    font::Font,
    font_data_for_range_set::FontDataForRangeSet,
    font_fallback_iterator::{FontFallbackIterator, HintCharList},
    font_fallback_priority::FontFallbackPriority,
    open_type_caps_support::OpenTypeCapsSupport,
    simple_font_data::SimpleFontData,
    small_caps_iterator::{SmallCapsBehavior, SmallCapsIterator},
  },
  style::{Direction as TextDirection, FontFeature, FontVariantCaps},
  text::character::{IDEOGRAPHIC_SPACE, is_8bit, is_common_or_inherited_script},
};

/// Blink's `ShouldSubpixelPosition`.
const SHOULD_SUBPIXEL_POSITION: bool = true;

/// A shaped `hb_buffer_t`: its glyphs, and their positions in 16.16 pixels.
pub(crate) struct HarfBuzzBuffer {
  pub(crate) infos: Vec<GlyphInfo>,
  pub(crate) positions: Vec<GlyphPosition>,
  direction: HbDirection,
}

impl HarfBuzzBuffer {
  /// `HB_DIRECTION_IS_FORWARD`.
  pub(crate) fn is_forward(&self) -> bool {
    matches!(
      self.direction,
      HbDirection::LeftToRight | HbDirection::TopToBottom
    )
  }
}

/// Blink's `ReshapeQueueItemAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReshapeQueueItemAction {
  NextFont,
  Range,
}

/// Blink's `ReshapeQueueItem`.
#[derive(Debug, Clone, Copy)]
struct ReshapeQueueItem {
  action: ReshapeQueueItemAction,
  start_index: usize,
  num_characters: usize,
}

impl ReshapeQueueItem {
  const NEXT_FONT: Self = Self {
    action: ReshapeQueueItemAction::NextFont,
    start_index: 0,
    num_characters: 0,
  };

  fn range(start_index: usize, num_characters: usize) -> Self {
    Self {
      action: ReshapeQueueItemAction::Range,
      start_index,
      num_characters,
    }
  }
}

/// Blink's `RangeContext`.
struct RangeContext<'a> {
  font: &'a Font,
  text_direction: TextDirection,
  start: usize,
  end: usize,
  font_features: Vec<Feature>,
  reshape_queue: VecDeque<ReshapeQueueItem>,
}

impl<'a> RangeContext<'a> {
  fn new(font: &'a Font, text_direction: TextDirection, start: usize, end: usize) -> Self {
    debug_assert!(end >= start);

    Self {
      font,
      text_direction,
      start,
      end,
      font_features: font.description.features.clone(),
      reshape_queue: VecDeque::new(),
    }
  }

  /// Blink's `HarfBuzzDirection` for horizontal text.
  fn harfbuzz_direction(&self) -> HbDirection {
    match self.text_direction {
      TextDirection::Ltr => HbDirection::LeftToRight,
      TextDirection::Rtl => HbDirection::RightToLeft,
    }
  }
}

/// Blink's `BufferSlice`.
#[derive(Debug, Clone, Copy)]
struct BufferSlice {
  start_character_index: usize,
  num_characters: usize,
  start_glyph_index: usize,
  num_glyphs: usize,
}

/// Blink's `FallbackFontStage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FallbackFontStage {
  Intermediate,
  Last,
}

impl FallbackFontStage {
  /// Blink's `IsLastFontToShape`.
  fn is_last_font_to_shape(self) -> bool {
    self == Self::Last
  }

  /// Blink's `ChangeStageToLast`.
  fn change_stage_to_last(self) -> Self {
    Self::Last
  }
}

/// Blink's `ICUScriptToHBScript`.
fn icu_script_to_hb_script(script: Script) -> harfrust::Script {
  PropertyNamesShort::<Script>::new()
    .get(script)
    .and_then(|name| {
      let tag: [u8; 4] = name.as_bytes().try_into().ok()?;

      harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&tag))
    })
    .unwrap_or(harfrust::Script::UNKNOWN)
}

/// Blink's `RoundHarfBuzzPosition`.
fn round_harfbuzz_position(value: &mut i32) {
  if *value & 0xFFFF != 0 {
    *value = ((*value as f32 / 65536.0).round() as i32) << 16;
  }
}

/// Blink's `ShapeRange`.
#[allow(clippy::too_many_arguments)]
fn shape_range(
  mut buffer: Buffer,
  font_features: &[Feature],
  current_font: &SimpleFontData,
  current_run_script: Script,
  direction: HbDirection,
  language: Option<&Language>,
) -> Option<HarfBuzzBuffer> {
  let shaper_font = ShaperFont::new(current_font.shaping_font()?);

  buffer.set_language(language.cloned());
  buffer.set_script(Some(icu_script_to_hb_script(current_run_script)));
  buffer.set_direction(direction);
  shape(
    &shaper_font,
    &mut buffer,
    ShapeOptions::new()
      .features(font_features)
      .point_size(Some(current_font.specified_size())),
  )
  .ok()?;

  let glyphs = buffer;
  // Approximate: Blink's HarfBuzz scales Skia's advances; see #2001.
  let scale = f64::from(current_font.size()) * 65536.0 / f64::from(current_font.units_per_em());
  let to_harfbuzz_position = |units: i32| (f64::from(units) * scale).round() as i32;
  let positions = glyphs
    .glyph_positions()
    .iter()
    .map(|position| {
      let mut position = *position;

      position.x_advance = to_harfbuzz_position(position.x_advance);
      position.y_advance = to_harfbuzz_position(position.y_advance);
      position.x_offset = to_harfbuzz_position(position.x_offset);
      position.y_offset = to_harfbuzz_position(position.y_offset);

      if !SHOULD_SUBPIXEL_POSITION {
        round_harfbuzz_position(&mut position.x_offset);
        round_harfbuzz_position(&mut position.y_offset);
        round_harfbuzz_position(&mut position.x_advance);
        round_harfbuzz_position(&mut position.y_advance);
      }

      position
    })
    .collect();

  Some(HarfBuzzBuffer {
    infos: glyphs.glyph_infos().to_vec(),
    positions,
    direction,
  })
}

/// Blink's `ComputeSlice`.
fn compute_slice(
  range_data: &RangeContext<'_>,
  current_queue_item: &ReshapeQueueItem,
  buffer: &HarfBuzzBuffer,
  old_glyph_index: usize,
  new_glyph_index: usize,
) -> BufferSlice {
  let glyph_info = &buffer.infos;
  let num_glyphs = glyph_info.len();
  let shape_end = range_data
    .end
    .min(current_queue_item.start_index + current_queue_item.num_characters);
  let (start_character_index, num_characters) = if buffer.is_forward() {
    let start_character_index = glyph_info[old_glyph_index].cluster as usize;
    let num_characters = if new_glyph_index == num_glyphs {
      shape_end - start_character_index
    } else {
      glyph_info[new_glyph_index].cluster as usize - start_character_index
    };

    (start_character_index, num_characters)
  } else {
    let start_character_index = glyph_info[new_glyph_index - 1].cluster as usize;
    let num_characters = if old_glyph_index == 0 {
      shape_end - start_character_index
    } else {
      glyph_info[old_glyph_index - 1].cluster as usize - start_character_index
    };

    (start_character_index, num_characters)
  };

  BufferSlice {
    start_character_index,
    num_characters,
    start_glyph_index: old_glyph_index,
    num_glyphs: new_glyph_index - old_glyph_index,
  }
}

/// Blink's `QueueCharacters`.
fn queue_characters(
  range_data: &mut RangeContext<'_>,
  font_cycle_queued: &mut bool,
  slice: &BufferSlice,
) {
  if !*font_cycle_queued {
    range_data
      .reshape_queue
      .push_back(ReshapeQueueItem::NEXT_FONT);
    *font_cycle_queued = true;
  }

  debug_assert!(slice.num_characters > 0);

  range_data.reshape_queue.push_back(ReshapeQueueItem::range(
    slice.start_character_index,
    slice.num_characters,
  ));
}

/// Blink's `ClusterResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClusterResult {
  Shaped,
  NotDef,
  Unknown,
}

/// Blink's `HarfBuzzShaper`.
pub(crate) struct HarfBuzzShaper {
  text: String,
}

impl HarfBuzzShaper {
  pub(crate) fn new(text: String) -> Self {
    Self { text }
  }

  /// Blink's `Shape`.
  pub(crate) fn shape(
    &self,
    font: &Font,
    direction: TextDirection,
    start: usize,
    end: usize,
  ) -> ShapeResult {
    assert!(end >= start);
    assert!(end <= self.text.len());

    let mut result = ShapeResult::new(start as u32, (end - start) as u32, direction);
    let mut range_data = RangeContext::new(font, direction, start, end);

    // Blink keeps text that fits Latin-1 in 8 bits, and shapes it as one Latin segment.
    if is_8bit(&self.text) {
      let segment_range = RunSegmenterRange {
        start,
        end,
        script: Script::Latin,
        font_fallback_priority: FontFallbackPriority::Text,
      };

      self.shape_segment(&mut range_data, &segment_range, &mut result);
    } else {
      let mut run_segmenter = RunSegmenter::new(&self.text);

      while let Some(segment_range) = run_segmenter.consume() {
        if start < segment_range.end && end > segment_range.start {
          self.shape_segment(&mut range_data, &segment_range, &mut result);
        }

        if segment_range.end >= end {
          break;
        }
      }
    }

    result
  }

  /// Blink's `Shape` of `start..end` in the runs `ranges`, which cover it in order.
  pub(crate) fn shape_segmented(
    &self,
    font: &Font,
    direction: TextDirection,
    start: usize,
    end: usize,
    ranges: &[RunSegmenterRange],
  ) -> ShapeResult {
    assert!(end >= start);
    assert!(end <= self.text.len());
    debug_assert_eq!(ranges.first().map(|range| range.start), Some(start));
    debug_assert_eq!(ranges.last().map(|range| range.end), Some(end));

    let mut result = ShapeResult::new(start as u32, (end - start) as u32, direction);
    let mut range_data = RangeContext::new(font, direction, start, end);

    for segmented_range in ranges {
      debug_assert!(segmented_range.start >= start && segmented_range.end <= end);

      self.shape_segment(&mut range_data, segmented_range, &mut result);
    }

    result
  }

  /// Blink's `Shape` of `start..end` within one run, `pre_segmented`.
  pub(crate) fn shape_pre_segmented(
    &self,
    font: &Font,
    direction: TextDirection,
    start: usize,
    end: usize,
    pre_segmented: &RunSegmenterRange,
  ) -> ShapeResult {
    assert!(end >= start);
    assert!(end <= self.text.len());
    debug_assert!(start >= pre_segmented.start && end <= pre_segmented.end);

    let mut result = ShapeResult::new(start as u32, (end - start) as u32, direction);
    let mut range_data = RangeContext::new(font, direction, start, end);

    self.shape_segment(&mut range_data, pre_segmented, &mut result);

    result
  }

  /// Blink's `Shape` of the whole text.
  pub(crate) fn shape_all(&self, font: &Font, direction: TextDirection) -> ShapeResult {
    self.shape(font, direction, 0, self.text.len())
  }

  /// Blink's `ShapeSegment`.
  fn shape_segment(
    &self,
    range_data: &mut RangeContext<'_>,
    segment: &RunSegmenterRange,
    result: &mut ShapeResult,
  ) {
    let font = range_data.font;
    let language = font.description.locale.as_ref();
    let shaping_language = font.description.shaping_language();
    let needs_caps_handling = font.description.variant_caps != FontVariantCaps::Normal;
    let mut fallback_iterator =
      FontFallbackIterator::new(&font.fallback_list, segment.font_fallback_priority);

    range_data
      .reshape_queue
      .push_back(ReshapeQueueItem::NEXT_FONT);
    range_data.reshape_queue.push_back(ReshapeQueueItem::range(
      segment.start,
      segment.end - segment.start,
    ));

    let mut font_cycle_queued = false;
    let mut fallback_chars_hint = HintCharList::new();
    let mut current_font_data_for_range_set = FontDataForRangeSet::default();
    let mut fallback_stage = FallbackFontStage::Intermediate;

    while let Some(mut current_queue_item) = range_data.reshape_queue.pop_front() {
      if current_queue_item.action != ReshapeQueueItemAction::Range {
        if !self.collect_fallback_hint_chars(
          &range_data.reshape_queue,
          fallback_iterator.needs_hint_list(),
          &mut fallback_chars_hint,
        ) {
          range_data.reshape_queue.clear();

          break;
        }

        current_font_data_for_range_set = fallback_iterator.next(&fallback_chars_hint);

        // Blink's last resort font always exists; takumi may have no font that parses at all.
        if current_font_data_for_range_set.font_data().is_none() {
          range_data.reshape_queue.clear();

          break;
        }

        font_cycle_queued = false;

        continue;
      }

      if !fallback_iterator.has_next() {
        fallback_stage = fallback_stage.change_stage_to_last();
      }

      let Some(font_data) = current_font_data_for_range_set.font_data().cloned() else {
        continue;
      };
      let font_ref = font_data.font_ref();
      let mut caps_support = OpenTypeCapsSupport::default();
      let mut small_caps_behavior = SmallCapsBehavior::SameCase;

      if needs_caps_handling && let Some(font_ref) = font_ref {
        caps_support = OpenTypeCapsSupport::new(
          font_ref,
          font.description.variant_caps,
          font.description.font_synthesis_small_caps,
          icu_script_to_hb_script(segment.script),
        );

        if caps_support.needs_run_case_splitting() {
          small_caps_behavior = self
            .split_until_next_case_change(&mut range_data.reshape_queue, &mut current_queue_item);

          // The rest of a split may fall outside the range being shaped.
          if range_data.start >= current_queue_item.start_index + current_queue_item.num_characters
            || range_data.end <= current_queue_item.start_index
          {
            continue;
          }
        }
      }

      debug_assert!(current_queue_item.num_characters > 0);

      let shape_start = range_data.start.max(current_queue_item.start_index);
      let shape_end = range_data
        .end
        .min(current_queue_item.start_index + current_queue_item.num_characters);

      debug_assert!(shape_end > shape_start);

      let mut adjusted_font = Rc::clone(&font_data);
      let mut case_map_intend = CaseMapIntend::KeepSameCase;

      if needs_caps_handling {
        case_map_intend = caps_support.needs_case_change(small_caps_behavior);

        if caps_support.needs_synthetic_font(small_caps_behavior) {
          adjusted_font = font_data.small_caps_font_data(&font.description);
        }
      }

      let mut buffer = Buffer::new();

      case_map_intend.fill_buffer(
        &mut buffer,
        language.map(Language::as_str),
        &self.text,
        shape_start,
        shape_end,
      );

      // Blink's `CapsFeatureSettingsScopedOverlay`: the caps features go before the others.
      let mut caps_features = Vec::new();

      caps_support
        .font_feature_to_use(small_caps_behavior)
        .append_features(&mut caps_features);

      let font_features: Cow<'_, [Feature]> = if caps_features.is_empty() {
        Cow::Borrowed(&range_data.font_features)
      } else {
        caps_features
          .into_iter()
          .map(FontFeature::into_harfrust)
          .chain(range_data.font_features.iter().copied())
          .collect()
      };

      let Some(buffer) = shape_range(
        buffer,
        &font_features,
        &adjusted_font,
        segment.script,
        range_data.harfbuzz_direction(),
        shaping_language.as_ref(),
      ) else {
        continue;
      };

      self.extract_shape_results(
        range_data,
        &mut font_cycle_queued,
        &current_queue_item,
        &adjusted_font,
        segment.script,
        &mut fallback_stage,
        result,
        &buffer,
      );
    }
  }

  /// Blink's `SplitUntilNextCaseChange`: shortens `current_queue_item` to its first run of one
  /// case, queueing the rest, and returns that run's case.
  fn split_until_next_case_change(
    &self,
    queue: &mut VecDeque<ReshapeQueueItem>,
    current_queue_item: &mut ReshapeQueueItem,
  ) -> SmallCapsBehavior {
    let start = current_queue_item.start_index;
    let text = &self.text[start..start + current_queue_item.num_characters];
    let Some((num_characters_until_case_change, small_caps_behavior)) =
      SmallCapsIterator::new(text).consume()
    else {
      return SmallCapsBehavior::SameCase;
    };

    if num_characters_until_case_change > 0
      && num_characters_until_case_change < current_queue_item.num_characters
    {
      queue.push_front(ReshapeQueueItem::range(
        start + num_characters_until_case_change,
        current_queue_item.num_characters - num_characters_until_case_change,
      ));
      current_queue_item.num_characters = num_characters_until_case_change;
    }

    small_caps_behavior
  }

  /// Blink's `ExtractShapeResults`.
  #[allow(clippy::too_many_arguments)]
  fn extract_shape_results(
    &self,
    range_data: &mut RangeContext<'_>,
    font_cycle_queued: &mut bool,
    current_queue_item: &ReshapeQueueItem,
    current_font: &Rc<SimpleFontData>,
    current_run_script: Script,
    fallback_stage: &mut FallbackFontStage,
    shape_result: &mut ShapeResult,
    buffer: &HarfBuzzBuffer,
  ) {
    let mut current_cluster_result = ClusterResult::Unknown;
    let mut previous_cluster_result = ClusterResult::Unknown;
    let mut current_cluster = 0;
    let glyph_info = &buffer.infos;
    let num_glyphs = glyph_info.len();
    let mut last_change_glyph_index = 0;
    let mut previous_cluster_start_glyph_index = 0;

    if num_glyphs == 0 {
      return;
    }

    let space_glyph = u32::from(current_font.space_glyph());
    let commit = |range_data: &RangeContext<'_>, shape_result: &mut ShapeResult, slice| {
      self.commit_glyphs(
        range_data,
        current_font,
        current_run_script,
        slice,
        shape_result,
        buffer,
      );
    };

    for (glyph_index, glyph) in glyph_info.iter().enumerate() {
      let previous_cluster = current_cluster;

      current_cluster = glyph.cluster as usize;

      let glyph_id = glyph.glyph_id;
      let glyph_result = if glyph_id == 0 {
        // Glyph 0 is `.notdef`:
        // https://docs.microsoft.com/en-us/typography/opentype/spec/recom#glyph-0-the-notdef-glyph
        ClusterResult::NotDef
      } else if glyph_id == space_glyph
        && !fallback_stage.is_last_font_to_shape()
        && self.text[current_cluster..].starts_with(IDEOGRAPHIC_SPACE)
      {
        // HarfBuzz draws U+3000 with the space glyph, which run splitting, features, and
        // `line-height` should not take: crbug.com/1193282 and
        // https://github.com/harfbuzz/harfbuzz/issues/2889.
        ClusterResult::NotDef
      } else {
        ClusterResult::Shaped
      };

      if current_cluster != previous_cluster {
        if previous_cluster_result != current_cluster_result
          && previous_cluster_result != ClusterResult::Unknown
        {
          let slice = compute_slice(
            range_data,
            current_queue_item,
            buffer,
            last_change_glyph_index,
            previous_cluster_start_glyph_index,
          );

          if current_cluster_result == ClusterResult::Shaped
            && !fallback_stage.is_last_font_to_shape()
          {
            queue_characters(range_data, font_cycle_queued, &slice);
          } else {
            commit(range_data, shape_result, slice);
          }

          last_change_glyph_index = previous_cluster_start_glyph_index;
        }

        previous_cluster_result = current_cluster_result;
        previous_cluster_start_glyph_index = glyph_index;
        current_cluster_result = glyph_result;
      } else {
        current_cluster_result = if glyph_result == ClusterResult::Shaped
          && matches!(
            current_cluster_result,
            ClusterResult::Shaped | ClusterResult::Unknown
          ) {
          ClusterResult::Shaped
        } else {
          ClusterResult::NotDef
        };
      }
    }

    if current_cluster_result != previous_cluster_result
      && previous_cluster_result != ClusterResult::Unknown
      && !fallback_stage.is_last_font_to_shape()
    {
      let before = compute_slice(
        range_data,
        current_queue_item,
        buffer,
        last_change_glyph_index,
        previous_cluster_start_glyph_index,
      );
      let last = compute_slice(
        range_data,
        current_queue_item,
        buffer,
        previous_cluster_start_glyph_index,
        num_glyphs,
      );

      if current_cluster_result == ClusterResult::Shaped {
        queue_characters(range_data, font_cycle_queued, &before);
        commit(range_data, shape_result, last);
      } else {
        commit(range_data, shape_result, before);
        queue_characters(range_data, font_cycle_queued, &last);
      }

      return;
    }

    let slice = compute_slice(
      range_data,
      current_queue_item,
      buffer,
      last_change_glyph_index,
      num_glyphs,
    );

    if current_cluster_result == ClusterResult::NotDef && !fallback_stage.is_last_font_to_shape() {
      queue_characters(range_data, font_cycle_queued, &slice);
    } else {
      commit(range_data, shape_result, slice);
    }
  }

  /// Blink's `CommitGlyphs`.
  fn commit_glyphs(
    &self,
    range_data: &RangeContext<'_>,
    current_font: &Rc<SimpleFontData>,
    current_run_script: Script,
    slice: BufferSlice,
    shape_result: &mut ShapeResult,
    buffer: &HarfBuzzBuffer,
  ) {
    let direction = range_data.harfbuzz_direction();
    let script = icu_script_to_hb_script(current_run_script);
    let mut current_slice = slice;
    let mut run_start_index = slice.start_character_index;

    loop {
      let run = ShapeResultRun::new(
        Some(current_font.clone()),
        direction,
        script,
        run_start_index as u32,
        current_slice.num_glyphs as u32,
        current_slice.num_characters as u32,
      );
      let (next_start_glyph, run_num_characters) = shape_result.insert_run_from_buffer(
        run,
        current_slice.start_glyph_index,
        current_slice.num_glyphs,
        buffer,
      );

      debug_assert!(current_slice.start_glyph_index + current_slice.num_glyphs >= next_start_glyph);

      let next_num_glyphs =
        current_slice.num_glyphs - (next_start_glyph - current_slice.start_glyph_index);

      if next_num_glyphs == 0 {
        break;
      }

      let run_num_characters = run_num_characters as usize;

      debug_assert!(current_slice.num_characters > run_num_characters);

      current_slice = BufferSlice {
        start_character_index: current_slice.start_character_index + run_num_characters,
        num_characters: current_slice.num_characters - run_num_characters,
        start_glyph_index: next_start_glyph,
        num_glyphs: next_num_glyphs,
      };

      if buffer.is_forward() {
        run_start_index = current_slice.start_character_index;
      }
    }
  }

  /// Blink's `CollectFallbackHintChars`.
  fn collect_fallback_hint_chars(
    &self,
    reshape_queue: &VecDeque<ReshapeQueueItem>,
    needs_hint_list: bool,
    hint: &mut HintCharList,
  ) -> bool {
    if reshape_queue.is_empty() {
      return false;
    }

    hint.clear();

    for item in reshape_queue {
      if item.action == ReshapeQueueItemAction::NextFont {
        break;
      }

      let range = &self.text[item.start_index..item.start_index + item.num_characters];

      for hint_char in range.chars() {
        hint.push(hint_char);

        if !needs_hint_list && !is_common_or_inherited_script(hint_char) {
          return true;
        }
      }
    }

    !hint.is_empty()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fonts::shaping::{
    glyph_data::HarfBuzzRunGlyphData,
    testing::{GEIST, NOTO_SANS_TC, font, geist, register},
  };
  use crate::{
    resources::font::Fonts,
    style::{FontSynthesic, FontVariantCaps},
  };

  const EB_GARAMOND_CAPS: &str = "../assets/fonts/eb-garamond/EBGaramondCaps.woff2";

  /// A 16px `family` font with `font-variant-caps: caps` and `font-synthesis-small-caps:
  /// synthesis`.
  fn caps_font(
    fonts: &Fonts,
    family: &str,
    caps: FontVariantCaps,
    synthesis: FontSynthesic,
  ) -> Font {
    let mut description = font(fonts, family).description;

    description.variant_caps = caps;
    description.font_synthesis_small_caps = synthesis;

    Font::new(fonts, description)
  }

  /// Each run's characters, font size, and glyphs.
  fn caps_runs(font: &Font, text: &str) -> Vec<(u32, u32, f32, Vec<u16>)> {
    HarfBuzzShaper::new(text.into())
      .shape_all(font, TextDirection::Ltr)
      .runs
      .iter()
      .map(|run| {
        (
          run.start_index,
          run.num_characters,
          run
            .font_data
            .as_ref()
            .map_or(0.0, |font_data| font_data.size()),
          run
            .glyph_data
            .glyphs()
            .iter()
            .map(|glyph| glyph.glyph)
            .collect(),
        )
      })
      .collect()
  }

  /// A font without `smcp` draws lowercase letters as uppercase ones at 70% of the size, and
  /// leaves uppercase ones.
  #[test]
  fn synthesizes_small_caps() {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");

    let plain = caps_runs(&font(&fonts, "Geist"), "AB");
    let small_caps = caps_font(
      &fonts,
      "Geist",
      FontVariantCaps::SmallCaps,
      FontSynthesic::Auto,
    );

    assert_eq!(
      caps_runs(&small_caps, "aB"),
      [
        (0, 1, 11.0, vec![plain[0].3[0]]),
        (1, 1, 16.0, vec![plain[0].3[1]])
      ]
    );
  }

  /// A letter whose uppercase is longer keeps all of it at its one character.
  #[test]
  fn synthesized_small_caps_keep_a_longer_uppercase() {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");

    let small_caps = caps_font(
      &fonts,
      "Geist",
      FontVariantCaps::SmallCaps,
      FontSynthesic::Auto,
    );
    let runs = caps_runs(&small_caps, "\u{DF}");

    assert_eq!((runs[0].0, runs[0].1, runs[0].3.len()), (0, 2, 2));
  }

  /// `font-synthesis-small-caps: none` leaves the text as it is.
  #[test]
  fn small_caps_without_synthesis() {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");

    let plain = caps_runs(&font(&fonts, "Geist"), "aB");
    let small_caps = caps_font(
      &fonts,
      "Geist",
      FontVariantCaps::SmallCaps,
      FontSynthesic::None,
    );

    assert_eq!(caps_runs(&small_caps, "aB"), plain);
  }

  /// A font with `smcp` draws small capitals with it, at its size.
  #[test]
  fn small_caps_from_the_font() {
    let mut fonts = Fonts::default();

    register(&mut fonts, EB_GARAMOND_CAPS, "EB Garamond Caps");

    let plain = caps_runs(&font(&fonts, "EB Garamond Caps"), "a");
    let small_caps = caps_font(
      &fonts,
      "EB Garamond Caps",
      FontVariantCaps::SmallCaps,
      FontSynthesic::Auto,
    );
    let runs = caps_runs(&small_caps, "a");

    assert_eq!((runs.len(), runs[0].2), (1, 16.0));
    assert_ne!(runs[0].3, plain[0].3);
  }

  /// Blink's `ShapeResultRunData`.
  fn runs(result: &ShapeResult) -> Vec<(u32, u32, u32, harfrust::Script)> {
    result
      .runs
      .iter()
      .map(|run| {
        (
          run.start_index,
          run.num_characters,
          run.num_glyphs(),
          run.script,
        )
      })
      .collect()
  }

  /// The family each run's font was registered under, as the font blob tells.
  fn run_fonts(result: &ShapeResult) -> Vec<u64> {
    result
      .runs
      .iter()
      .map(|run| run.font_data.as_ref().unwrap().face().blob.id())
      .collect()
  }

  #[test]
  fn shape_latin_segment() {
    let (_fonts, font) = geist();
    let string = "Hello World!";
    let direction = TextDirection::Ltr;
    let shaper = HarfBuzzShaper::new(string.into());
    let combined = shaper.shape_all(&font, direction);
    let first = shaper.shape(&font, direction, 0, 6);
    let second = shaper.shape(&font, direction, 6, 11);
    let third = shaper.shape(&font, direction, 11, 12);

    assert_eq!((runs(&first)[0].0, runs(&first)[0].1), (0, 6));
    assert_eq!((runs(&second)[0].0, runs(&second)[0].1), (6, 5));
    assert_eq!((runs(&third)[0].0, runs(&third)[0].1), (11, 1));

    let first_reference = HarfBuzzShaper::new(string[0..6].into()).shape_all(&font, direction);
    let second_reference = HarfBuzzShaper::new(string[6..11].into()).shape_all(&font, direction);
    let third_reference = HarfBuzzShaper::new(string[11..12].into()).shape_all(&font, direction);

    assert!((first_reference.width() - first.width()).abs() <= 1.0);
    assert!((second_reference.width() - second.width()).abs() <= 1.0);
    assert!((third_reference.width() - third.width()).abs() <= 1.0);

    let total_width = first.width() + second.width() + third.width();

    assert!((combined.width() - total_width).abs() <= 1.0);
  }

  /// With no font, the text shapes to no runs.
  #[test]
  fn no_font() {
    let fonts = Fonts::default();
    let font = font(&fonts, "Geist");
    let result = HarfBuzzShaper::new("a".into()).shape_all(&font, TextDirection::Ltr);

    assert!(result.runs.is_empty());
  }

  #[test]
  fn missing_glyph() {
    let (_fonts, font) = geist();

    for string in ["\u{FFF0}", "\u{FFF0}Hello", "\u{AD}"] {
      let result = HarfBuzzShaper::new(string.into()).shape_all(&font, TextDirection::Ltr);

      assert_eq!(result.start_index(), 0);
      assert_eq!(result.end_index() as usize, string.len());
    }
  }

  #[test]
  fn max_glyphs_simple() {
    let (_fonts, font) = geist();
    let length = HarfBuzzRunGlyphData::MAX_CHARACTERS + 1;
    let string = "X".repeat(length as usize);
    let shaper = HarfBuzzShaper::new(string);
    let latin = icu_script_to_hb_script(Script::Latin);

    let ltr = shaper.shape_all(&font, TextDirection::Ltr);

    assert_eq!((ltr.num_characters(), ltr.num_glyphs()), (length, length));
    assert_eq!(
      runs(&ltr),
      [
        (0, length - 1, length - 1, latin),
        (length - 1, 1, 1, latin)
      ]
    );

    let rtl = shaper.shape_all(&font, TextDirection::Rtl);

    assert_eq!((rtl.num_characters(), rtl.num_glyphs()), (length, length));
    assert_eq!(
      runs(&rtl),
      [(1, length - 1, length - 1, latin), (0, 1, 1, latin)]
    );
  }

  #[test]
  fn zero_width_space() {
    let (_fonts, font) = geist();
    let string = "\u{200B}\u{200B}\u{627}\u{631}\u{62F}\u{648}\u{200B}\u{200B}";

    for direction in [TextDirection::Ltr, TextDirection::Rtl] {
      let result = HarfBuzzShaper::new(string.into()).shape_all(&font, direction);

      assert_eq!(result.start_index(), 0);
      assert_eq!(result.end_index() as usize, string.len());
    }
  }

  #[test]
  fn ideographic_space() {
    let mut fonts = Fonts::default();

    register(&mut fonts, NOTO_SANS_TC, "Noto Sans TC");

    let font = font(&fonts, "Noto Sans TC");
    let result =
      HarfBuzzShaper::new("\u{3001}\u{3000}".into()).shape_all(&font, TextDirection::Ltr);
    let mut fonts = run_fonts(&result);

    fonts.dedup();

    assert_eq!(fonts.len(), 1);
  }

  #[test]
  fn falls_back_to_the_next_family() {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");
    register(&mut fonts, NOTO_SANS_TC, "Noto Sans TC");

    let font = font(&fonts, "Geist, 'Noto Sans TC'");
    let geist = run_fonts(&HarfBuzzShaper::new("a".into()).shape_all(&font, TextDirection::Ltr))[0];
    let result = HarfBuzzShaper::new("ab\u{203B}cd".into()).shape_all(&font, TextDirection::Ltr);
    let ranges: Vec<(u32, u32)> = runs(&result)
      .into_iter()
      .map(|(start, characters, ..)| (start, characters))
      .collect();
    let fonts = run_fonts(&result);

    assert_eq!(ranges, [(0, 2), (2, 3), (5, 2)]);
    assert_eq!(fonts[0], geist);
    assert_ne!(fonts[1], geist);
    assert_eq!(fonts[2], geist);
    assert_eq!(result.num_characters() as usize, "ab\u{203B}cd".len());
  }
}
