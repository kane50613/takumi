//! Font instances described once per distinct face and variation.

use std::collections::HashMap;

use skrifa::{FontRef, MetadataProvider, attribute::Style};

use super::tree::{PaintFont, PaintVariation};
use crate::{layout::inline::ShapedRun, resources::font::FontsSnapshot};

#[derive(PartialEq, Eq, Hash)]
struct FontKey {
  font_id: u64,
  index: u32,
  variations: Vec<([u8; 4], u32)>,
  synthetic_bold: Option<u32>,
  synthetic_skew: Option<u32>,
}

impl FontKey {
  fn of(run: &ShapedRun) -> Self {
    Self {
      font_id: run.font_id(),
      index: run.font_index,
      variations: run
        .variations
        .iter()
        .map(|(tag, value)| (*tag, value.to_bits()))
        .collect(),
      synthetic_bold: run.synthetic_bold.map(f32::to_bits),
      synthetic_skew: run.synthetic_skew.map(f32::to_bits),
    }
  }
}

/// Caches the description of each font instance the runs use.
#[derive(Default)]
pub(super) struct FontTable {
  fonts: HashMap<FontKey, PaintFont>,
}

impl FontTable {
  /// The instance `run` was shaped with.
  pub(super) fn describe(&mut self, fonts: &FontsSnapshot, run: &ShapedRun) -> PaintFont {
    self
      .fonts
      .entry(FontKey::of(run))
      .or_insert_with(|| describe(fonts, run))
      .clone()
  }
}

fn describe(fonts: &FontsSnapshot, run: &ShapedRun) -> PaintFont {
  let attributes = FontRef::from_index(run.font_data(), run.font_index)
    .ok()
    .map(|font| font.attributes());
  let axis = |tag: &[u8; 4]| {
    run
      .variations
      .iter()
      .find(|(axis, _)| axis == tag)
      .map(|(_, value)| *value)
  };
  let weight = axis(b"wght")
    .or_else(|| attributes.map(|attributes| attributes.weight.value()))
    .unwrap_or(400.0);
  let style = match attributes.map(|attributes| attributes.style) {
    _ if run.synthetic_skew.is_some() => "oblique",
    Some(Style::Italic) => "italic",
    Some(Style::Oblique(_)) => "oblique",
    Some(Style::Normal) | None => "normal",
  };
  let width = axis(b"wdth")
    .or_else(|| attributes.map(|attributes| attributes.stretch.percentage()))
    .unwrap_or(100.0);

  PaintFont {
    family: fonts.face_family(run.font_id(), run.font_index),
    face_index: run.font_index,
    weight,
    style: style.to_string(),
    width,
    variations: run
      .variations
      .iter()
      .map(|(tag, value)| PaintVariation {
        tag: String::from_utf8_lossy(tag).into_owned(),
        value: *value,
      })
      .collect(),
    synthetic_bold_width: run.synthetic_bold,
    synthetic_oblique_angle: run.synthetic_skew,
  }
}
