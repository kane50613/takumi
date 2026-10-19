//! Blink's `SimpleFontData`, with `FontPlatformData` and `HarfBuzzFace` folded in.

use std::{
  cell::OnceCell,
  fmt::{self, Debug, Formatter},
  rc::Rc,
};

use harfrust::font::{Font as ShapingFont, Variation};
use skrifa::{
  FontRef, GlyphId, MetadataProvider,
  instance::Size,
  raw::{TableProvider, types::F2Dot14},
};

use crate::{
  fonts::font_description::FontDescription,
  resources::font::{MatchedFace, PrimaryFontMetrics},
};

/// Blink's `kSmallCapsFontSizeMultiplier`.
const SMALL_CAPS_FONT_SIZE_MULTIPLIER: f32 = 0.7;

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
  font_metrics: OnceCell<Option<PrimaryFontMetrics>>,
  shaping_font: OnceCell<Option<ShapingFont>>,
  small_caps: OnceCell<Rc<SimpleFontData>>,
}

impl SimpleFontData {
  /// `face` at `size`; none for a face that does not parse.
  pub(crate) fn new(face: MatchedFace, size: f32, specified_size: f32) -> Option<Self> {
    let font = FontRef::from_index(face.blob.data(), face.index).ok()?;
    let units_per_em = font.head().ok()?.units_per_em();
    let space_glyph = font
      .charmap()
      .map(' ')
      .map_or(0, |glyph| glyph.to_u32() as u16);
    let axes = font.axes();
    let mut variations: Vec<Variation> = Vec::new();

    // A setting on an axis the face lacks does nothing, and a later one on an axis replaces an
    // earlier one, as skrifa resolves a location.
    for &variation in face
      .variations
      .iter()
      .filter(|variation| axes.iter().any(|axis| axis.tag() == variation.tag))
    {
      match variations
        .iter_mut()
        .find(|existing| existing.tag == variation.tag)
      {
        Some(existing) => *existing = variation,
        None => variations.push(variation),
      }
    }

    // A setting at its axis's default leaves the default instance, which the face draws without.
    variations.retain(|variation| {
      axes
        .iter()
        .find(|axis| axis.tag() == variation.tag)
        .is_some_and(|axis| axis.default_value() != variation.value)
    });

    Some(Self {
      face,
      size,
      specified_size,
      variations,
      units_per_em,
      space_glyph,
      font_metrics: OnceCell::new(),
      shaping_font: OnceCell::new(),
      small_caps: OnceCell::new(),
    })
  }

  /// Blink's `SmallCapsFontData`: this font at the size synthesized small capitals take.
  pub(crate) fn small_caps_font_data(&self, font_description: &FontDescription) -> Rc<Self> {
    Rc::clone(self.small_caps.get_or_init(|| {
      Rc::new(self.create_scaled_font_data(font_description, SMALL_CAPS_FONT_SIZE_MULTIPLIER))
    }))
  }

  /// Blink's `CreateScaledFontData`.
  fn create_scaled_font_data(&self, font_description: &FontDescription, scale_factor: f32) -> Self {
    Self {
      face: self.face.clone(),
      size: (font_description.computed_size * scale_factor).round(),
      specified_size: self.specified_size,
      variations: self.variations.clone(),
      units_per_em: self.units_per_em,
      space_glyph: self.space_glyph,
      font_metrics: OnceCell::new(),
      shaping_font: OnceCell::new(),
      small_caps: OnceCell::new(),
    }
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

  /// The variation coordinates the face is instanced at, as user-space `(tag, value)` pairs.
  pub(crate) fn variation_settings(&self) -> Vec<([u8; 4], f32)> {
    self
      .variations
      .iter()
      .map(|variation| (variation.tag.to_be_bytes(), variation.value))
      .collect()
  }

  /// The face's variation coordinates, normalized.
  pub(crate) fn normalized_coords(&self) -> Vec<F2Dot14> {
    let Some(font) = self.font_ref() else {
      return Vec::new();
    };

    font
      .axes()
      .location(
        self
          .variations
          .iter()
          .map(|variation| (variation.tag, variation.value)),
      )
      .coords()
      .to_vec()
  }

  /// Blink's `SpaceGlyph`.
  pub(crate) fn space_glyph(&self) -> u16 {
    self.space_glyph
  }

  /// Blink's `SpaceWidth`: the space glyph's advance.
  pub(crate) fn space_width(&self) -> f32 {
    let Some(font) = self.font_ref() else {
      return 0.0;
    };
    let location = font.axes().location(
      self
        .variations
        .iter()
        .map(|variation| (variation.tag, variation.value)),
    );

    font
      .glyph_metrics(Size::new(self.size), &location)
      .advance_width(GlyphId::new(u32::from(self.space_glyph)))
      .unwrap_or(0.0)
      .max(0.0)
  }

  /// Blink's `GetFontMetrics`, at the face's variation coordinates.
  pub(crate) fn font_metrics(&self) -> Option<PrimaryFontMetrics> {
    *self.font_metrics.get_or_init(|| {
      let font = self.font_ref()?;
      let location = font.axes().location(
        self
          .variations
          .iter()
          .map(|variation| (variation.tag, variation.value)),
      );

      Some(PrimaryFontMetrics::of(&font, self.size, (&location).into()))
    })
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

  /// The face at its variation coordinates, as harfrust shapes with it, read once.
  pub(crate) fn shaping_font(&self) -> Option<&ShapingFont> {
    self
      .shaping_font
      .get_or_init(|| {
        let font = self.face.blob.shaping_font(self.face.index)?;

        Some(if self.variations.is_empty() {
          font
        } else {
          font
            .instance_builder()
            .variations(self.variations.iter().copied())
            .build()
        })
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
