//! Blink's `HarfBuzzShaper`, which shapes text with a font and its fallbacks, after
//! [`harfbuzz_shaper.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/harfbuzz_shaper.cc).
//! Follows Blink under the notice in LICENSE-CHROMIUM.
//!
//! Offsets count UTF-8 bytes where Blink's count UTF-16 code units. harfrust stands in for
//! HarfBuzz and returns font units, which this scales to HarfBuzz's 16.16 pixels; Blink's HarfBuzz
//! reads glyph advances from Skia instead, so positions are approximate to 1/65536 px.
//!
//! Not ported: vertical text, the `font-variant-caps` overlay, `HanKerning` for
//! `text-spacing-trim`, incremental font transfer, and `font-variant-emoji`. harfrust has no hook
//! for variation sequences a font lacks, so the stages that reshape without the variation selector
//! never start.

use std::{collections::VecDeque, rc::Rc};

use harfrust::{
  Direction as HbDirection, Feature, GlyphInfo, GlyphPosition, Language, ShapeOptions,
  UnicodeBuffer,
};
use icu_properties::{PropertyNamesShort, props::Script};

use super::{
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
    simple_font_data::SimpleFontData,
  },
  style::Direction as TextDirection,
  text::character::{IDEOGRAPHIC_SPACE, is_common_or_inherited_script},
};

/// Blink's `ShouldSubpixelPosition`. takumi positions glyphs at fractions of a pixel, as Chromium
/// does on macOS and at high device scale factors.
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

/// Blink's `RangeContext`: what shaping one range carries along.
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

/// Blink's `BufferSlice`: glyphs of a buffer and the characters they are for.
#[derive(Debug, Clone, Copy)]
struct BufferSlice {
  start_character_index: usize,
  num_characters: usize,
  start_glyph_index: usize,
  num_glyphs: usize,
}

/// Blink's `FallbackFontStage`, without the variation selector stages.
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
    .unwrap_or(harfrust::script::UNKNOWN)
}

/// Blink's `RoundHarfBuzzPosition`: a 16.16 value with a fraction, rounded to whole pixels.
fn round_harfbuzz_position(value: &mut i32) {
  if *value & 0xFFFF != 0 {
    *value = ((*value as f32 / 65536.0).round() as i32) << 16;
  }
}

/// Blink's `ShapeRange`: `text` from `start` to `end` shaped with `current_font`, the rest of
/// `text` as context, or none for a font that does not parse.
#[allow(clippy::too_many_arguments)]
fn shape_range(
  text: &str,
  start: usize,
  end: usize,
  font_features: &[Feature],
  current_font: &SimpleFontData,
  current_run_script: Script,
  direction: HbDirection,
  language: Option<&Language>,
) -> Option<HarfBuzzBuffer> {
  let font = current_font.font_ref()?;
  let shaper = current_font
    .shaper_data(&font)
    .shaper(&font)
    .instance(current_font.shaper_instance(&font))
    .build();
  let mut buffer = UnicodeBuffer::new();

  // Blink's `CaseMappingHarfBuzzBufferFiller` keeping the case: clusters index the whole text.
  for (offset, ch) in text[start..end].char_indices() {
    buffer.add(ch, (start + offset) as u32);
  }

  buffer.set_pre_context(&text[..start]);
  buffer.set_post_context(&text[end..]);

  if let Some(language) = language {
    buffer.set_language(language.clone());
  }

  buffer.set_script(icu_script_to_hb_script(current_run_script));
  buffer.set_direction(direction);

  let glyphs = shaper.shape(
    buffer,
    ShapeOptions::new()
      .features(font_features)
      .point_size(Some(current_font.specified_size())),
  );
  // HarfBuzz scales font units to the 16.16 pixels its scale asks for.
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

/// Blink's `ComputeSlice`: the characters of glyphs `old_glyph_index..new_glyph_index`, clusters
/// counting down in RTL.
fn compute_slice(
  range_data: &RangeContext<'_>,
  current_queue_item: &ReshapeQueueItem,
  buffer: &HarfBuzzBuffer,
  old_glyph_index: usize,
  new_glyph_index: usize,
) -> BufferSlice {
  let glyph_info = &buffer.infos;
  let num_glyphs = glyph_info.len();
  // The end of the queue item, within the shaping window.
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

/// Blink's `QueueCharacters`: queues `slice` for the next font, after the font switch once.
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

  /// Blink's `Shape`: `text` from `start` to `end` with `font` in `direction`. `start` and `end`
  /// should be where a line may break.
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
    if self.text.chars().all(|ch| u32::from(ch) <= 0xFF) {
      let segment_range = RunSegmenterRange {
        start,
        end,
        script: Script::Latin,
        font_fallback_priority: FontFallbackPriority::Text,
      };

      self.shape_segment(&mut range_data, &segment_range, &mut result);
    } else {
      // Segmentation covers the whole text, whatever the shaping window.
      let mut run_segmenter = RunSegmenter::new(&self.text);

      while let Some(segment_range) = run_segmenter.consume() {
        // Every segment overlapping the window, not only those inside it.
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

  /// Blink's `Shape` of the whole text.
  pub(crate) fn shape_all(&self, font: &Font, direction: TextDirection) -> ShapeResult {
    self.shape(font, direction, 0, self.text.len())
  }

  /// Blink's `ShapeSegment`: shapes `segment` font by font, queuing the characters a font lacks
  /// for the next one.
  fn shape_segment(
    &self,
    range_data: &mut RangeContext<'_>,
    segment: &RunSegmenterRange,
    result: &mut ShapeResult,
  ) {
    let font = range_data.font;
    let language = font.description.locale.as_ref();
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

    while let Some(current_queue_item) = range_data.reshape_queue.pop_front() {
      if current_queue_item.action != ReshapeQueueItemAction::Range {
        // Without hint characters there is no font to fall back to.
        if !self.collect_fallback_hint_chars(
          &range_data.reshape_queue,
          fallback_iterator.needs_hint_list(),
          &mut fallback_chars_hint,
        ) {
          range_data.reshape_queue.clear();

          break;
        }

        current_font_data_for_range_set = fallback_iterator.next(&fallback_chars_hint);

        if current_font_data_for_range_set.font_data().is_none() {
          debug_assert!(range_data.reshape_queue.is_empty());

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

      debug_assert!(current_queue_item.num_characters > 0);

      // The queue item within the shaping window.
      let shape_start = range_data.start.max(current_queue_item.start_index);
      let shape_end = range_data
        .end
        .min(current_queue_item.start_index + current_queue_item.num_characters);

      debug_assert!(shape_end > shape_start);

      let Some(buffer) = shape_range(
        &self.text,
        shape_start,
        shape_end,
        &range_data.font_features,
        &font_data,
        segment.script,
        range_data.harfbuzz_direction(),
        language,
      ) else {
        continue;
      };

      self.extract_shape_results(
        range_data,
        &mut font_cycle_queued,
        &current_queue_item,
        &font_data,
        segment.script,
        &mut fallback_stage,
        result,
        &buffer,
      );
    }
  }

  /// Blink's `ExtractShapeResults`: commits the clusters `current_font` shaped, and queues the
  /// ones it could not for the next font, unless it is the last.
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

    // Each cluster is shaped or not as a whole.
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
        // The cluster just passed is complete: a change from the one before it ends a slice.
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

          // A shaped cluster after the change means the ones before it were not, so they queue,
          // unless this is the last font; else the ones before it were shaped and commit.
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

    // The last cluster differs from the ones before it: one shaped and one unshaped slice.
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

    // No change at the last cluster: everything since the last change commits or queues.
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

  /// Blink's `CommitGlyphs`: inserts runs for `slice`, several when it is longer than a run holds.
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

      // The rest of a slice longer than a run holds goes into another run.
      let run_num_characters = run_num_characters as usize;

      debug_assert!(current_slice.num_characters > run_num_characters);

      current_slice = BufferSlice {
        start_character_index: current_slice.start_character_index + run_num_characters,
        num_characters: current_slice.num_characters - run_num_characters,
        start_glyph_index: next_start_glyph,
        num_glyphs: next_num_glyphs,
      };

      // The run lost its right end: the end characters in LTR, the start ones in RTL.
      if buffer.is_forward() {
        run_start_index = current_slice.start_character_index;
      }
    }
  }

  /// Blink's `CollectFallbackHintChars`: the characters queued for the next font, or only the
  /// first with a script when `needs_hint_list` is false. False when none are queued.
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

        // Fallback needs one character with a script to find a font.
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
  use std::{fs::read, path::Path, rc::Rc};

  use parley::fontique::Attributes;

  use super::*;
  use crate::{
    font_style::ExpandedFontFamily,
    fonts::{font_description::FontDescription, shaping::glyph_data::HarfBuzzRunGlyphData},
    resources::font::{FontOverride, FontResource, Fonts},
    style::{FontFamily, FromCssStr},
  };

  const GEIST: &str = "../assets/fonts/geist/Geist[wght].woff2";
  const NOTO_SANS_TC: &str = "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2";

  fn register(fonts: &mut Fonts, path: &str, family_name: &str) {
    let bytes = read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();

    fonts
      .register(FontResource::new(bytes).override_info(FontOverride {
        family_name: Some(family_name.into()),
        ..Default::default()
      }))
      .unwrap();
  }

  fn font(fonts: &mut Fonts, family: &str) -> Font {
    let family = FontFamily::from_css_str(family).unwrap();

    Font::new(
      fonts,
      FontDescription {
        family: Rc::new(ExpandedFontFamily::expand(&family)),
        attributes: Attributes::default(),
        computed_size: 16.0,
        specified_size: 16.0,
        features: Vec::new(),
        variation_settings: Vec::new(),
        locale: None,
      },
    )
  }

  fn geist() -> (Fonts, Font) {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");

    let font = font(&mut fonts, "Geist");

    (fonts, font)
  }

  /// Blink's `ShapeResultRunData`: each run's start, characters, glyphs, and script.
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

    // A pixel of tolerance for kerning across the cuts.
    assert!((first_reference.width() - first.width()).abs() <= 1.0);
    assert!((second_reference.width() - second.width()).abs() <= 1.0);
    assert!((third_reference.width() - third.width()).abs() <= 1.0);

    let total_width = first.width() + second.width() + third.width();

    assert!((combined.width() - total_width).abs() <= 1.0);
  }

  #[test]
  fn missing_glyph() {
    let (_fonts, font) = geist();

    // U+FFF0 is unassigned, and U+00AD SOFT HYPHEN often has no glyph.
    for string in ["\u{FFF0}", "\u{FFF0}Hello", "\u{AD}"] {
      let result = HarfBuzzShaper::new(string.into()).shape_all(&font, TextDirection::Ltr);

      assert_eq!(result.start_index(), 0);
      assert_eq!(result.end_index() as usize, string.len());
    }
  }

  // Code point, glyph, and cluster are all 1:1, so runs split at `MAX_CHARACTER_INDEX`.
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

    let font = font(&mut fonts, "Noto Sans TC");
    // IDEOGRAPHIC COMMA, IDEOGRAPHIC SPACE.
    let result =
      HarfBuzzShaper::new("\u{3001}\u{3000}".into()).shape_all(&font, TextDirection::Ltr);
    let mut fonts = run_fonts(&result);

    fonts.dedup();

    assert_eq!(fonts.len(), 1);
  }

  // A character the first font lacks falls back to the next family, in a run of its own.
  #[test]
  fn falls_back_to_the_next_family() {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");
    register(&mut fonts, NOTO_SANS_TC, "Noto Sans TC");

    let font = font(&mut fonts, "Geist, 'Noto Sans TC'");
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
