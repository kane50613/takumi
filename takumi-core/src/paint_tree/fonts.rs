//! The font instance table runs reference by index.

use std::collections::HashMap;

use parley::fontique::Blob;
use skrifa::{FontRef, MetadataProvider, attribute::Style};

use super::document::{PaintFont, PaintVariation};
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

/// Deduplicates the font instances the runs use.
#[derive(Default)]
pub(super) struct FontTable {
  indices: HashMap<FontKey, usize>,
  fonts: Vec<PaintFont>,
  data: Vec<Blob<u8>>,
}

impl FontTable {
  /// The table index of the instance `run` was shaped with.
  pub(super) fn intern(&mut self, fonts: &FontsSnapshot, run: &ShapedRun) -> usize {
    let key = FontKey::of(run);
    if let Some(&index) = self.indices.get(&key) {
      return index;
    }
    let index = self.fonts.len();
    self.fonts.push(describe(fonts, run));
    self.data.push(run.font_blob());
    self.indices.insert(key, index);
    index
  }

  /// The instances, and each one's font file, in table order.
  pub(super) fn finish(self) -> (Vec<PaintFont>, Vec<Blob<u8>>) {
    (self.fonts, self.data)
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
  let stretch = axis(b"wdth")
    .or_else(|| attributes.map(|attributes| attributes.stretch.percentage()))
    .unwrap_or(100.0);

  PaintFont {
    family: fonts.face_family(run.font_id(), run.font_index),
    face_index: run.font_index,
    weight,
    style,
    stretch,
    variation_settings: run
      .variations
      .iter()
      .map(|(tag, value)| PaintVariation {
        tag: String::from_utf8_lossy(tag).into_owned(),
        value: *value,
      })
      .collect(),
  }
}
