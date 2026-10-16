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

/// Every glyph of one run, by its index in the run, with `None` where the run's colour does not fill it.
pub(super) struct RunOutline(Box<[Option<OutlineGlyph>]>);

impl FromIterator<Option<OutlineGlyph>> for RunOutline {
  fn from_iter<I: IntoIterator<Item = Option<OutlineGlyph>>>(glyphs: I) -> Self {
    Self(glyphs.into_iter().collect())
  }
}

impl OutlineGlyph {
  /// The glyph's outline as SVG path data, positioned from the run's baseline start.
  fn path_data(&self) -> Option<String> {
    match self.glyph.as_ref() {
      ResolvedGlyph::Outline(outline) => Some(path_data(
        outline.paths(),
        Affine::translation(self.offset.x, self.offset.y),
      )),
      ResolvedGlyph::Bitmap(_) => None,
    }
  }
}

impl RunOutline {
  /// Each glyph's outline as SVG path data, positioned from the run's baseline start.
  fn path_data(&self) -> Vec<Option<String>> {
    self
      .0
      .iter()
      .map(|glyph| glyph.as_ref().and_then(OutlineGlyph::path_data))
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

  /// The path data of each glyph of run `run` of text node `node`.
  pub(super) fn path_data(&self, node: usize, run: usize) -> Option<Vec<Option<String>>> {
    Some(self.0.get(&node)?.get(run)?.path_data())
  }
}
