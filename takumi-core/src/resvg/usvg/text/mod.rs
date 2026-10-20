// Copyright 2024 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::resvg::usvg::{Cache, Text};

pub(crate) mod flatten;

/// Provides access to the layout of a text node.
pub mod layout;

/// The ID of a glyph within a font
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphId(pub u32);

impl From<GlyphId> for skrifa::raw::types::GlyphId {
  fn from(value: GlyphId) -> Self {
    Self::new(value.0)
  }
}

/// Convert a text into its paths. This is done in two steps:
/// 1. We convert the text into glyphs and position them according to the rules specified
///    in the SVG specification. While doing so, we also calculate the text bbox (which
///    is not based on the outlines of a glyph, but instead the glyph metrics as well
///    as decoration spans).
/// 2. We convert all of the positioned glyphs into outlines.
pub(crate) fn convert(text: &mut Text, cache: &mut Cache) -> Option<()> {
  let (text_fragments, bbox) = layout::layout_text(text, cache)?;
  text.layouted = text_fragments;
  text.bounding_box = bbox.to_rect();
  text.abs_bounding_box = bbox.transform(text.abs_transform)?.to_rect();

  let (group, stroke_bbox) = flatten::flatten(text, cache)?;
  text.flattened = Box::new(group);
  text.stroke_bounding_box = stroke_bbox.to_rect();
  text.abs_stroke_bounding_box = stroke_bbox.transform(text.abs_transform)?.to_rect();

  Some(())
}
