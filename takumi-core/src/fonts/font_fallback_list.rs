//! Blink's [`FontFallbackList`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_fallback_list.cc), resolving every family up front; approximate, see #2001.

use std::{cell::OnceCell, rc::Rc};

use super::{
  font_cache::FontCache,
  font_data_for_range_set::FontDataForRangeSet,
  font_description::FontFallbackKey,
  font_face::{FaceData, PendingSource},
  font_fallback_iterator::FontFallbackIterator,
  font_fallback_priority::FontFallbackPriority,
  shaping::shape_cache::ShapeCache,
  simple_font_data::SimpleFontData,
};
use crate::{
  resources::font::{Fonts, MatchedFace},
  style::{FontFamilyToken, GenericFamilyType},
};

/// Blink's `FontData`.
#[derive(Debug)]
pub(crate) enum FontData {
  Simple(Rc<SimpleFontData>),
  Segmented(Vec<FontDataForRangeSet>),
}

impl FontData {
  /// Blink's `IsSegmented`.
  pub(crate) fn is_segmented(&self) -> bool {
    matches!(self, Self::Segmented(_))
  }

  /// Blink's `FontDataForCharacter`, none when the face is pending.
  pub(crate) fn font_data_for_character(&self, character: char) -> Option<Rc<SimpleFontData>> {
    match self {
      Self::Simple(font_data) => Some(Rc::clone(font_data)),
      Self::Segmented(faces) => faces
        .iter()
        .find(|face| face.contains(character))
        .or(faces.first())
        .and_then(FontDataForRangeSet::font_data)
        .cloned(),
    }
  }

  /// Blink's `SegmentedFontData::ContainsCharacter`.
  fn contains_character(&self, character: char) -> bool {
    match self {
      Self::Simple(_) => true,
      Self::Segmented(faces) => faces.iter().any(|face| face.contains(character)),
    }
  }
}

/// Blink's `FontFallbackList`.
#[derive(Debug)]
pub(crate) struct FontFallbackList {
  font_list: Vec<FontData>,
  pub(crate) font_cache: FontCache,
  /// Blink's `PrimaryFont()->GetShapeCache()`.
  pub(crate) shape_cache: ShapeCache,
  /// Blink's `cached_primary_simple_font_data_with_space_`.
  primary_with_space: OnceCell<Option<Rc<SimpleFontData>>>,
  /// Blink's `cached_primary_simple_font_data_with_digit_zero_`.
  primary_with_digit_zero: OnceCell<Option<Rc<SimpleFontData>>>,
  /// Whether the registry the list resolved from holds a pending face.
  has_pending_faces: bool,
}

impl FontFallbackList {
  pub(crate) fn new(fonts: &Fonts, key: &FontFallbackKey) -> Self {
    let simple_font_data = |face: MatchedFace| {
      SimpleFontData::new(face, key.computed_size, key.specified_size).map(Rc::new)
    };
    let font_data = |face: FaceData<MatchedFace>| match face {
      FaceData::Loaded(face) => simple_font_data(face).map(FaceData::Loaded),
      FaceData::Pending(face) => Some(FaceData::Pending(face)),
    };
    let resolve = |faces: Vec<FaceData<MatchedFace>>| {
      if let [FaceData::Loaded(face)] = faces.as_slice()
        && face.unicode_range.is_none()
      {
        return simple_font_data(face.clone()).map(FontData::Simple);
      }

      let faces: Vec<FontDataForRangeSet> = faces
        .into_iter()
        .filter_map(font_data)
        .map(FontDataForRangeSet::from_face_data)
        .collect();

      (!faces.is_empty()).then_some(FontData::Segmented(faces))
    };
    let mut font_list: Vec<FontData> = fonts
      .font_group(key.family.tokens(), &key.selection)
      .into_iter()
      .filter_map(resolve)
      .collect();
    let font_cache = FontCache::new(
      fonts
        .fallback_bucket(&key.selection)
        .into_iter()
        .filter_map(font_data)
        .collect(),
    );
    let standard = fonts
      .font_group(
        &[FontFamilyToken::Generic(GenericFamilyType::Serif)],
        &key.selection,
      )
      .into_iter()
      .find_map(resolve)
      .or_else(|| font_cache.last_resort_fallback_font().map(FontData::Simple));

    font_list.extend(standard);

    Self {
      font_list,
      font_cache,
      shape_cache: ShapeCache::default(),
      primary_with_space: OnceCell::new(),
      primary_with_digit_zero: OnceCell::new(),
      has_pending_faces: fonts.has_pending_faces(),
    }
  }

  /// Whether the registry the list resolved from holds a pending face.
  pub(crate) fn has_pending_faces(&self) -> bool {
    self.has_pending_faces
  }

  /// Blink's `FontDataAt`.
  pub(crate) fn font_data_at(&self, realized_font_index: usize) -> Option<&FontData> {
    self.font_list.get(realized_font_index)
  }

  /// Blink's `PrimarySimpleFontDataWithSpace`: the font metrics come from.
  pub(crate) fn primary_simple_font_data(&self) -> Option<Rc<SimpleFontData>> {
    self
      .primary_with_space
      .get_or_init(|| self.determine_primary_simple_font_data(' '))
      .clone()
  }

  /// Blink's `PrimarySimpleFontDataWithDigitZero`: the font `ch` measures.
  pub(crate) fn primary_simple_font_data_with_digit_zero(&self) -> Option<Rc<SimpleFontData>> {
    self
      .primary_with_digit_zero
      .get_or_init(|| self.determine_primary_simple_font_data('0'))
      .clone()
  }

  /// Blink's `DeterminePrimarySimpleFontDataCore`; approximate, the last resort where Blink takes a pending font's loading fallback.
  fn determine_primary_simple_font_data(&self, character: char) -> Option<Rc<SimpleFontData>> {
    for font_data in &self.font_list {
      if !font_data.contains_character(character) {
        continue;
      }

      if let Some(font_data_for_character) = font_data.font_data_for_character(character) {
        return Some(font_data_for_character);
      }

      if let FontData::Segmented(faces) = font_data
        && let Some(range_font_data) = faces.iter().find_map(FontDataForRangeSet::font_data)
      {
        return Some(Rc::clone(range_font_data));
      }
    }

    self
      .font_list
      .first()
      .and_then(|font_data| font_data.font_data_for_character(character))
      .or_else(|| self.font_cache.last_resort_fallback_font())
  }

  /// The pending face shaping would load for `character`; approximate, hinting with `character` alone.
  pub(crate) fn pending_face_for_character(
    &self,
    character: char,
    font_fallback_priority: FontFallbackPriority,
  ) -> Option<PendingSource> {
    let mut iterator = FontFallbackIterator::new(self, font_fallback_priority);
    let hint_list = [character];

    while iterator.has_next() {
      let font_data = iterator.next(&hint_list);

      if let Some(face) = iterator.tracked_loading_faces().next() {
        return Some(face.source.clone());
      }

      if font_data
        .font_data()
        .is_some_and(|font_data| font_data.has_glyph(character))
      {
        return None;
      }
    }

    None
  }
}
