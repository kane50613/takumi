//! The glyph outlines of text runs, kept to write as path data on request.

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::{
  geometry::Point, path_data::path_data, resources::glyph::ResolvedGlyph, style::Affine,
};

/// A glyph a run's colour fills, and its origin from the run's baseline start.
pub(super) struct OutlineGlyph {
  pub(super) glyph: Arc<ResolvedGlyph>,
  pub(super) offset: Point<f32>,
}

/// The glyphs of one run that its colour fills.
pub(super) struct RunOutline(Box<[OutlineGlyph]>);

impl FromIterator<OutlineGlyph> for RunOutline {
  fn from_iter<I: IntoIterator<Item = OutlineGlyph>>(glyphs: I) -> Self {
    Self(glyphs.into_iter().collect())
  }
}

impl RunOutline {
  /// The outlines as SVG path data, positioned from the run's baseline start.
  fn path_data(&self) -> String {
    self
      .0
      .iter()
      .filter_map(|OutlineGlyph { glyph, offset }| match glyph.as_ref() {
        ResolvedGlyph::Outline(outline) => Some(path_data(
          outline.paths(),
          Affine::translation(offset.x, offset.y),
        )),
        ResolvedGlyph::Bitmap(_) => None,
      })
      .collect()
  }
}

/// Every text node's run outlines, by node id.
#[derive(Default)]
pub(super) struct Outlines(FxHashMap<usize, Box<[RunOutline]>>);

impl Outlines {
  /// Keeps the outlines of the runs of text node `node`.
  pub(super) fn insert(&mut self, node: usize, runs: Box<[RunOutline]>) {
    self.0.insert(node, runs);
  }

  /// The path data of run `run` of text node `node`.
  pub(super) fn path_data(&self, node: usize, run: usize) -> Option<String> {
    Some(self.0.get(&node)?.get(run)?.path_data())
  }
}
