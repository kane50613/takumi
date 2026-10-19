//! Blink's `SimpleFontData`, with `FontPlatformData` and `HarfBuzzFace` folded in.

use std::{
  cell::OnceCell,
  fmt::{self, Debug, Formatter},
};

use harfrust::{FontRef, ShaperData, ShaperInstance, Tag, Variation};
use skrifa::{MetadataProvider, raw::TableProvider};

use crate::resources::font::MatchedFace;

/// Blink's `SimpleFontData`.
pub(crate) struct SimpleFontData {
  face: MatchedFace,
  /// Blink's `FontPlatformData::size`.
  size: f32,
  /// The size `trak` reads, which Blink passes to `hb_font_set_ptem`.
  specified_size: f32,
  variations: Vec<Variation>,
  units_per_em: u16,
  space_glyph: u16,
  shaper_data: OnceCell<ShaperData>,
  shaper_instance: OnceCell<Option<ShaperInstance>>,
}

impl SimpleFontData {
  /// `face` at `size`, with the variation `settings` after the ones its synthesis asks for; none for a face that does not parse.
  pub(crate) fn new(
    face: MatchedFace,
    size: f32,
    specified_size: f32,
    settings: &[Variation],
  ) -> Option<Self> {
    let font = FontRef::from_index(face.blob.data(), face.index).ok()?;
    let units_per_em = font.head().ok()?.units_per_em();
    let space_glyph = font
      .charmap()
      .map(' ')
      .map_or(0, |glyph| glyph.to_u32() as u16);
    let variations = face
      .synthesis
      .variation_settings()
      .iter()
      .map(|&(tag, value)| Variation {
        tag: Tag::new(&tag.to_be_bytes()),
        value,
      })
      .chain(settings.iter().copied())
      .collect();

    Some(Self {
      face,
      size,
      specified_size,
      variations,
      units_per_em,
      space_glyph,
      shaper_data: OnceCell::new(),
      shaper_instance: OnceCell::new(),
    })
  }

  pub(crate) fn face(&self) -> &MatchedFace {
    &self.face
  }

  pub(crate) fn size(&self) -> f32 {
    self.size
  }

  pub(crate) fn specified_size(&self) -> f32 {
    self.specified_size
  }

  pub(crate) fn units_per_em(&self) -> u16 {
    self.units_per_em
  }

  /// Blink's `SpaceGlyph`.
  pub(crate) fn space_glyph(&self) -> u16 {
    self.space_glyph
  }

  /// The font the face reads from.
  pub(crate) fn font_ref(&self) -> Option<FontRef<'_>> {
    FontRef::from_index(self.face.blob.data(), self.face.index).ok()
  }

  /// Whether the face maps `ch` to a glyph, within its `unicode-range`.
  pub(crate) fn has_glyph(&self, ch: char) -> bool {
    self
      .font_ref()
      .is_some_and(|font| font.charmap().map(ch).is_some())
  }

  /// The tables harfrust shapes with, read once.
  pub(crate) fn shaper_data(&self, font: &FontRef<'_>) -> &ShaperData {
    self.shaper_data.get_or_init(|| ShaperData::new(font))
  }

  /// The face at its variation coordinates, or none for a face without variations.
  pub(crate) fn shaper_instance(&self, font: &FontRef<'_>) -> Option<&ShaperInstance> {
    self
      .shaper_instance
      .get_or_init(|| {
        (!self.variations.is_empty())
          .then(|| ShaperInstance::from_variations(font, self.variations.iter().copied()))
      })
      .as_ref()
  }
}

impl Debug for SimpleFontData {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("SimpleFontData")
      .field("face", &self.face)
      .field("size", &self.size)
      .finish_non_exhaustive()
  }
}
