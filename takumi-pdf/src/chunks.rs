//! A scene's paint chunks, resolved once for every page that walks them, and
//! indexed by the content height their paint reaches so a page window visits
//! only its own.

use takumi_core::{
  layout::tree::RenderNode,
  paint_chunk::PaintChunk,
  painter::OwnContent,
  scene::{NodePaint, Scene},
};

use crate::window::Window;

/// The content height one index bucket covers, before a sparse scene widens it.
const BUCKET_HEIGHT: usize = 1024;

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
  /// Per band of `bucket_height` content height, the chunks whose paint reaches it.
  buckets: Vec<Vec<usize>>,
  bucket_height: usize,
  /// Chunks without paint bounds, which every window visits.
  unbounded: Vec<usize>,
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
    let bottom = chunks
      .iter()
      .filter_map(|resolved| resolved.chunk.node.paint_bounds)
      .map(|bounds| bounds.bottom)
      .max()
      .unwrap_or_default();
    // A sparse scene widens its buckets so their count stays within its chunk count.
    let bucket_height = BUCKET_HEIGHT.max(bottom / (chunks.len() + 1) + 1);
    let mut buckets = vec![Vec::new(); bottom / bucket_height + 1];
    let mut unbounded = Vec::new();

    for (index, resolved) in chunks.iter().enumerate() {
      match resolved.chunk.node.paint_bounds {
        Some(bounds) => {
          for bucket in &mut buckets[bounds.top / bucket_height..=bounds.bottom / bucket_height] {
            bucket.push(index);
          }
        }
        None => unbounded.push(index),
      }
    }

    Self {
      chunks,
      owners,
      buckets,
      bucket_height,
      unbounded,
    }
  }

  /// The chunks whose paint reaches `window`, in paint order.
  pub(crate) fn visible(&self, window: Window) -> Vec<&ResolvedChunk<'s>> {
    let mut indices = match window.y {
      Some((top, bottom)) => {
        let bucket =
          |y: f32| (y.max(0.0) as usize / self.bucket_height).min(self.buckets.len() - 1);
        let mut indices: Vec<usize> = self.buckets[bucket(top)..=bucket(bottom)]
          .iter()
          .flatten()
          .chain(&self.unbounded)
          .copied()
          .collect();

        indices.sort_unstable();
        indices.dedup();
        indices
      }
      None => (0..self.chunks.len()).collect(),
    };

    indices.retain(|&index| !window.excludes_bounds(self.chunks[index].chunk.node.paint_bounds));
    indices
      .into_iter()
      .map(|index| &self.chunks[index])
      .collect()
  }
}
