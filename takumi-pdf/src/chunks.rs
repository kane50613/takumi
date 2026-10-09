//! A scene's paint chunks, resolved once for every page that walks them, and
//! indexed by the content height their paint reaches so a page window visits
//! only its own.

use takumi_core::{
  layout::tree::RenderNode,
  paint_chunk::PaintChunk,
  painter::OwnContent,
  scene::{NodePaint, Scene, SceneBounds},
};

use crate::window::Window;

/// The content height one index bucket covers, before a sparse scene widens it.
const BUCKET_HEIGHT: usize = 1024;

/// The most buckets one chunk enters. A taller one joins the chunks every
/// window checks, which keeps the index linear in its chunks.
const MAX_SPAN: usize = 16;

/// One chunk with the box it paints and what that box paints inside itself.
pub(crate) struct ResolvedChunk<'s> {
  pub(crate) chunk: PaintChunk<'s>,
  pub(crate) node: Option<&'s RenderNode>,
  pub(crate) content: OwnContent<'s>,
}

/// A scene's chunks in paint order, with the box owning each effect.
pub(crate) struct SceneChunks<'s> {
  pub(crate) chunks: Vec<ResolvedChunk<'s>>,
  pub(crate) owners: Vec<Option<&'s NodePaint>>,
  index: HeightIndex,
}

/// Chunk indices by the content height their paint bounds reach.
struct HeightIndex {
  /// Per band of `bucket_height` content height, the chunks reaching it.
  buckets: Vec<Vec<usize>>,
  bucket_height: usize,
  /// Chunks every window checks: those without bounds, and those too tall to bucket.
  unindexed: Vec<usize>,
}

impl<'s> SceneChunks<'s> {
  pub(crate) fn new(scene: &'s Scene) -> Self {
    let chunks = PaintChunk::in_paint_order(&scene.contexts);
    let owners = PaintChunk::effect_owners(&chunks, &scene.properties);
    let chunks = chunks
      .into_iter()
      .map(|chunk| {
        let node = scene.root.node_at_path(&chunk.node.path);

        ResolvedChunk {
          chunk,
          node,
          content: node.map_or(OwnContent::None, OwnContent::of),
        }
      })
      .collect::<Vec<_>>();
    let index = HeightIndex::new(
      &chunks
        .iter()
        .map(|resolved| resolved.chunk.node.paint_bounds)
        .collect::<Vec<_>>(),
    );

    Self {
      chunks,
      owners,
      index,
    }
  }

  /// The chunks whose paint reaches `window`, in paint order.
  pub(crate) fn visible(&self, window: Window) -> Vec<&ResolvedChunk<'s>> {
    let mut indices = match window.y {
      Some((top, bottom)) => self.index.candidates(top, bottom),
      None => (0..self.chunks.len()).collect(),
    };

    indices.retain(|&index| !window.excludes_bounds(self.chunks[index].chunk.node.paint_bounds));
    indices
      .into_iter()
      .map(|index| &self.chunks[index])
      .collect()
  }
}

impl HeightIndex {
  fn new(bounds: &[Option<SceneBounds>]) -> Self {
    let bottom = bounds
      .iter()
      .flatten()
      .map(|bounds| bounds.bottom)
      .max()
      .unwrap_or_default();
    // A sparse scene widens its buckets so their count stays within its chunk count.
    let bucket_height = BUCKET_HEIGHT.max(bottom / (bounds.len() + 1) + 1);
    let mut buckets = vec![Vec::new(); bottom / bucket_height + 1];
    let mut unindexed = Vec::new();

    for (index, bounds) in bounds.iter().enumerate() {
      let span = bounds
        .map(|bounds| bounds.top / bucket_height..=bounds.bottom / bucket_height)
        .filter(|span| {
          span
            .end()
            .checked_sub(*span.start())
            .is_some_and(|reach| reach < MAX_SPAN)
        });

      match span {
        Some(span) => {
          for bucket in &mut buckets[span] {
            bucket.push(index);
          }
        }
        None => unindexed.push(index),
      }
    }

    Self {
      buckets,
      bucket_height,
      unindexed,
    }
  }

  /// The chunks that may reach `[top, bottom)`, in paint order, with every unindexed one.
  fn candidates(&self, top: f32, bottom: f32) -> Vec<usize> {
    let bucket = |y: f32| (y.max(0.0) as usize / self.bucket_height).min(self.buckets.len() - 1);
    let mut indices: Vec<usize> = self
      .buckets
      .get(bucket(top)..=bucket(bottom))
      .unwrap_or_default()
      .iter()
      .flatten()
      .chain(&self.unindexed)
      .copied()
      .collect();

    indices.sort_unstable();
    indices.dedup();
    indices
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn bounds(top: usize, bottom: usize) -> Option<SceneBounds> {
    Some(SceneBounds {
      left: 0,
      top,
      right: 10,
      bottom,
    })
  }

  #[test]
  fn a_window_finds_the_chunks_it_reaches() {
    let index = HeightIndex::new(&[bounds(0, 10), bounds(5000, 5010), None, bounds(9000, 9010)]);

    assert_eq!(index.candidates(4800.0, 5200.0), [1, 2]);
  }

  #[test]
  fn an_inverted_window_finds_only_the_unindexed_chunks() {
    let index = HeightIndex::new(&[bounds(0, 10), None, bounds(9000, 9010)]);

    assert_eq!(index.candidates(9000.0, 100.0), [1]);
  }

  #[test]
  fn tall_chunks_stay_out_of_the_buckets() {
    let tall = vec![bounds(0, 1_000_000); 1000];
    let index = HeightIndex::new(&tall);

    assert_eq!(index.buckets.iter().map(Vec::len).sum::<usize>(), 0);
    assert_eq!(index.unindexed.len(), 1000);
  }

  #[test]
  fn inverted_bounds_stay_out_of_the_buckets() {
    let index = HeightIndex::new(&[bounds(5000, 10)]);

    assert_eq!(index.unindexed, [0]);
  }
}
