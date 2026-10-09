//! A scene's paint chunks, resolved once for every page that walks them.

use takumi_core::{
  layout::tree::RenderNode,
  paint_chunk::PaintChunk,
  painter::OwnContent,
  scene::{NodePaint, Scene},
};

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
      .collect();

    Self { chunks, owners }
  }
}
