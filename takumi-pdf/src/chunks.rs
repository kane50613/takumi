//! A scene's paint chunks, resolved once for every page that walks them, and
//! indexed by the content height their paint reaches so a page window visits
//! only its own.

use takumi_core::{
  layout::tree::RenderNode,
  paint_chunk::{HeightIndex, PaintChunk},
  painter::OwnContent,
  scene::{NodePaint, SceneView},
};

use crate::window::Window;

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

impl<'s> SceneChunks<'s> {
  pub(crate) fn new(scene: SceneView<'s>) -> Self {
    let chunks = PaintChunk::in_paint_order(scene.contexts);
    let owners = PaintChunk::effect_owners(&chunks, scene.properties);
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
