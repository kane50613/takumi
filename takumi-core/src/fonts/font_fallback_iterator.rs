//! Blink's [`FontFallbackIterator`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_fallback_iterator.cc).
//! Approximate: system fallback also records pending faces, which Blink never picks there.

use std::{collections::HashSet, rc::Rc};

use super::{
  font_data_for_range_set::FontDataForRangeSet,
  font_face::PendingFace,
  font_fallback_list::{FontData, FontFallbackList},
  font_fallback_priority::FontFallbackPriority,
  simple_font_data::SimpleFontData,
};
use crate::text::character::has_likely_script;

/// Blink's `HintCharList`.
pub(crate) type HintCharList = Vec<char>;

/// Blink's `FallbackStage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FallbackStage {
  FallbackPriorityFonts,
  FontGroupFonts,
  SegmentedFace,
  SystemFonts,
  FirstCandidateForNotdefGlyph,
  OutOfLuck,
}

/// Blink's `FontFallbackIterator`.
pub(crate) struct FontFallbackIterator<'a> {
  font_fallback_list: &'a FontFallbackList,
  current_font_data_index: usize,
  segmented_face_index: usize,
  fallback_stage: FallbackStage,
  font_fallback_priority: FontFallbackPriority,
  previously_asked_for_hint: HashSet<char>,
  /// The faces returned whole, by font blob id and face index: Blink's typeface ids.
  unique_font_data_for_range_sets_returned: HashSet<(u64, u32)>,
  first_candidate: Option<FontDataForRangeSet>,
  /// Blink's `tracked_loading_range_sets_`.
  tracked_loading_range_sets: Vec<PendingFace>,
}

impl<'a> FontFallbackIterator<'a> {
  pub(crate) fn new(
    font_fallback_list: &'a FontFallbackList,
    font_fallback_priority: FontFallbackPriority,
  ) -> Self {
    Self {
      font_fallback_list,
      current_font_data_index: 0,
      segmented_face_index: 0,
      fallback_stage: FallbackStage::FontGroupFonts,
      font_fallback_priority,
      previously_asked_for_hint: HashSet::new(),
      unique_font_data_for_range_sets_returned: HashSet::new(),
      first_candidate: None,
      tracked_loading_range_sets: Vec::new(),
    }
  }

  /// Blink's `Reset`: back to the first font, for the pass that ignores variation selectors.
  pub(crate) fn reset(&mut self) {
    self.current_font_data_index = 0;
    self.segmented_face_index = 0;
    self.fallback_stage = FallbackStage::FontGroupFonts;
    self.previously_asked_for_hint.clear();
    self.unique_font_data_for_range_sets_returned.clear();
    self.first_candidate = None;
    self.tracked_loading_range_sets.clear();
  }

  /// The pending faces the iterator has passed, in the order it met them.
  pub(crate) fn tracked_loading_faces(&self) -> impl Iterator<Item = &PendingFace> {
    self.tracked_loading_range_sets.iter()
  }

  /// Blink's `AlreadyLoadingRangeForHintChar`.
  fn already_loading_range_for_hint_char(&self, hint_char: char) -> bool {
    self
      .tracked_loading_range_sets
      .iter()
      .any(|range| range.range_set.contains(u32::from(hint_char)))
  }

  /// Blink's `HasNext`.
  pub(crate) fn has_next(&self) -> bool {
    self.fallback_stage != FallbackStage::OutOfLuck
  }

  /// Blink's `NeedsHintList`.
  pub(crate) fn needs_hint_list(&self) -> bool {
    match self.fallback_stage {
      FallbackStage::SegmentedFace => true,
      FallbackStage::FontGroupFonts => self
        .font_fallback_list
        .font_data_at(self.current_font_data_index)
        .is_some_and(FontData::is_segmented),
      _ => false,
    }
  }

  /// Blink's `RangeSetContributesForHint`.
  fn range_set_contributes_for_hint(
    &self,
    hint_list: &[char],
    segmented_face: &FontDataForRangeSet,
  ) -> bool {
    hint_list.iter().any(|&hint| {
      segmented_face.contains(hint)
        && (segmented_face.pending_face().is_none()
          || !self.already_loading_range_for_hint_char(hint))
    })
  }

  /// Records the pending face system fallback would pick for `hint`.
  fn track_pending_fallback_font(
    &mut self,
    hint: char,
    font_fallback_priority: FontFallbackPriority,
  ) {
    if !self.font_fallback_list.has_pending_faces()
      || self.already_loading_range_for_hint_char(hint)
    {
      return;
    }

    if let Some(face) = self
      .font_fallback_list
      .font_cache
      .pending_fallback_font_for_character(hint, font_fallback_priority)
    {
      self.tracked_loading_range_sets.push(face.clone());
    }
  }

  /// Blink's `UniqueOrNext`.
  fn unique_or_next(
    &mut self,
    candidate: FontDataForRangeSet,
    hint_list: &[char],
  ) -> FontDataForRangeSet {
    let Some(font_data) = candidate.font_data() else {
      return self.next(hint_list);
    };
    let candidate_id = (font_data.face.typeface.id, font_data.face.typeface.index);

    if self
      .unique_font_data_for_range_sets_returned
      .contains(&candidate_id)
    {
      return self.next(hint_list);
    }

    if candidate.is_entire_range() {
      self
        .unique_font_data_for_range_sets_returned
        .insert(candidate_id);
    }

    if self.first_candidate.is_none() {
      self.first_candidate = Some(candidate.clone());
    }

    candidate
  }

  /// Blink's `Next`.
  pub(crate) fn next(&mut self, hint_list: &[char]) -> FontDataForRangeSet {
    match self.fallback_stage {
      FallbackStage::OutOfLuck => return FontDataForRangeSet::default(),
      FallbackStage::FallbackPriorityFonts => {
        self.fallback_stage = FallbackStage::SystemFonts;

        if let Some(&hint) = hint_list.first() {
          self.track_pending_fallback_font(hint, self.font_fallback_priority);
        }

        return match hint_list
          .first()
          .and_then(|&hint| self.fallback_priority_font(hint))
        {
          Some(font_data) => self.unique_or_next(FontDataForRangeSet::new(font_data), hint_list),
          None => self.next(hint_list),
        };
      }
      FallbackStage::SystemFonts => {
        if let Some(system_font) = self.unique_system_font_for_hint_list(hint_list) {
          return self.unique_or_next(FontDataForRangeSet::new(system_font), hint_list);
        }

        self.fallback_stage = FallbackStage::FirstCandidateForNotdefGlyph;

        let last_resort = self
          .font_fallback_list
          .font_cache
          .last_resort_fallback_font()
          .map(FontDataForRangeSet::new)
          .unwrap_or_default();

        return self.unique_or_next(last_resort, hint_list);
      }
      FallbackStage::FirstCandidateForNotdefGlyph => {
        self.fallback_stage = FallbackStage::OutOfLuck;

        return self.first_candidate.clone().unwrap_or_default();
      }
      FallbackStage::FontGroupFonts | FallbackStage::SegmentedFace => {}
    }

    let Some(font_data) = self
      .font_fallback_list
      .font_data_at(self.current_font_data_index)
    else {
      self.fallback_stage = if self.font_fallback_priority.is_non_text() {
        FallbackStage::FallbackPriorityFonts
      } else {
        FallbackStage::SystemFonts
      };

      return self.next(hint_list);
    };

    let faces = match font_data {
      FontData::Simple(font_data) => {
        self.current_font_data_index += 1;

        return self.unique_or_next(FontDataForRangeSet::new(font_data.clone()), hint_list);
      }
      FontData::Segmented(faces) => faces,
    };

    if self.fallback_stage != FallbackStage::SegmentedFace {
      self.segmented_face_index = 0;
      self.fallback_stage = FallbackStage::SegmentedFace;
    }

    let current_segmented_face = faces[self.segmented_face_index].clone();

    self.segmented_face_index += 1;

    if self.segmented_face_index == faces.len() {
      self.fallback_stage = FallbackStage::FontGroupFonts;
      self.current_font_data_index += 1;
    }

    if self.range_set_contributes_for_hint(hint_list, &current_segmented_face) {
      let Some(pending_face) = current_segmented_face.pending_face() else {
        return self.unique_or_next(current_segmented_face, hint_list);
      };

      self.tracked_loading_range_sets.push(pending_face.clone());
    }

    self.next(hint_list)
  }

  /// Blink's `FallbackPriorityFont`.
  fn fallback_priority_font(&self, hint: char) -> Option<Rc<SimpleFontData>> {
    self
      .font_fallback_list
      .font_cache
      .fallback_font_for_character(hint, self.font_fallback_priority)
  }

  /// Blink's `ChooseHintIndex`.
  fn choose_hint_index(hint_list: &[char]) -> usize {
    debug_assert!(!hint_list.is_empty());

    if hint_list.len() <= 1 {
      return 0;
    }

    (1..hint_list.len())
      .find(|&i| has_likely_script(hint_list[i]))
      .unwrap_or(0)
  }

  /// Blink's `UniqueSystemFontForHintList`.
  fn unique_system_font_for_hint_list(&mut self, hint_list: &[char]) -> Option<Rc<SimpleFontData>> {
    if hint_list.is_empty() {
      return None;
    }

    let hint = hint_list[Self::choose_hint_index(hint_list)];

    if hint == '\0' || !self.previously_asked_for_hint.insert(hint) {
      return None;
    }

    self.track_pending_fallback_font(hint, FontFallbackPriority::Text);

    self
      .font_fallback_list
      .font_cache
      .fallback_font_for_character(hint, FontFallbackPriority::Text)
  }
}
