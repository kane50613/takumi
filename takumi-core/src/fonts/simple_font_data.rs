//! Blink's `SimpleFontData`, with `FontPlatformData` and `HarfBuzzFace` folded in.

use std::{
  cell::{OnceCell, RefCell},
  fmt::{self, Debug, Formatter},
  rc::Rc,
};

use harfrust::{
  Direction, Feature, Language, Script, ShapePlan, ShapePlanKey,
  font::{Font as ShapingFont, Variation},
};
use skrifa::{
  FontRef, GlyphId, MetadataProvider,
  instance::Size,
  raw::{TableProvider, types::F2Dot14},
};

use crate::{
  fonts::font_description::FontDescription,
  resources::{
    font::{MatchedFace, PrimaryFontMetrics, RunSynthesis},
    glyph::synthesis_embolden_strength,
  },
};

/// Blink's `kSmallCapsFontSizeMultiplier`.
const SMALL_CAPS_FONT_SIZE_MULTIPLIER: f32 = 0.7;

/// The angle in degrees of the `-1/4` horizontal skew Skia gives a synthetic italic, which
/// Blink's `FontPlatformData::SetupSkFont` asks for.
const SYNTHETIC_ITALIC_SKEW: f32 = 14.036_243;

/// Blink's `SimpleFontData`.
pub(crate) struct SimpleFontData {
  pub(crate) face: MatchedFace,
  /// Blink's `FontPlatformData::size`.
  pub(crate) size: f32,
  /// The size `trak` reads, which Blink passes to `hb_font_set_ptem`.
  pub(crate) specified_size: f32,
  variations: Vec<Variation>,
  pub(crate) units_per_em: u16,
  /// Blink's `SpaceGlyph`.
  pub(crate) space_glyph: u16,
  font_metrics: OnceCell<Option<PrimaryFontMetrics>>,
  shaping_font: OnceCell<Option<ShapingFont>>,
  shape_plans: RefCell<Vec<Rc<ShapePlan>>>,
  small_caps: OnceCell<Rc<SimpleFontData>>,
}

impl SimpleFontData {
  /// `face` at `size`, scaled by its `size-adjust`; none for a face that does not parse.
  pub(crate) fn new(face: MatchedFace, size: f32, specified_size: f32) -> Option<Self> {
    let size = face
      .size_adjust
      .map_or(size, |size_adjust| size * size_adjust);
    let font = face.typeface.font_ref()?;
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
      shape_plans: RefCell::default(),
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
      size: (font_description.fallback.computed_size * scale_factor).round(),
      specified_size: self.specified_size,
      variations: self.variations.clone(),
      units_per_em: self.units_per_em,
      space_glyph: self.space_glyph,
      font_metrics: OnceCell::new(),
      shaping_font: OnceCell::new(),
      shape_plans: RefCell::default(),
      small_caps: OnceCell::new(),
    }
  }

  /// The synthetic bold and italic Blink's `FontPlatformData::SetupSkFont` asks Skia for; a color
  /// face takes neither.
  pub(crate) fn synthesis(&self) -> RunSynthesis {
    let synthesizes = !self.face.typeface.is_color;

    RunSynthesis {
      embolden: (synthesizes && self.face.synthetic_bold)
        .then(|| synthesis_embolden_strength(self.size)),
      skew: (synthesizes && self.face.synthetic_italic).then_some(-SYNTHETIC_ITALIC_SKEW),
    }
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

      Some(PrimaryFontMetrics::of(
        &font,
        self.size,
        (&location).into(),
        self.face.metrics_override,
      ))
    })
  }

  /// The font the face reads from.
  pub(crate) fn font_ref(&self) -> Option<FontRef<'_>> {
    self.face.typeface.font_ref()
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
        let font = self.face.typeface.shaping_font()?;

        Some(if self.variations.is_empty() {
          font.clone()
        } else {
          font
            .instance_builder()
            .variations(self.variations.iter().copied())
            .build()
        })
      })
      .as_ref()
  }

  /// The harfrust plan for a segment's direction, script, language, and features, compiled once
  /// per font, as HarfBuzz's `hb_shape_plan_create_cached` keeps plans on the face for Blink.
  pub(crate) fn shape_plan(
    &self,
    direction: Direction,
    script: Script,
    language: Option<&Language>,
    features: &[Feature],
  ) -> Option<Rc<ShapePlan>> {
    let font = self.shaping_font()?;
    let key = ShapePlanKey::new(font, Some(script), direction)
      .language(language)
      .features(features);
    let mut plans = self.shape_plans.borrow_mut();

    if let Some(plan) = plans.iter().find(|plan| key.matches(plan)) {
      return Some(Rc::clone(plan));
    }

    let plan = Rc::new(ShapePlan::new(
      font,
      direction,
      Some(script),
      language,
      features,
    ));

    plans.push(Rc::clone(&plan));
    Some(plan)
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
