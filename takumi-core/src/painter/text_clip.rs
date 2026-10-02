//! The text a box's `background-clip: text` background shows through: the inline content of the
//! box and of every box inside it, as Blink's `kTextClip` paint phase walks them.

use super::{BoxFrame, BoxPainter, FillShape, GlyphDevice, OwnContent, StripBackground};
use crate::{
  error::Result,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, NodeId, Point},
  layout::{
    inline::{
      InlineLayoutMode, InlineLayoutRequest, InlinePass, ProcessedInlineSpan, create_inline_layout,
    },
    inline_box::{InlineBoxPaint, resolve_inline_box},
    tree::{LayoutResults, RenderNode},
  },
  paint_chunk::{ChunkPart, PaintChunk},
  scene::NodePaint,
  style::{Affine, BackgroundClip},
};

/// A box whose background shows only through the text inside it, and that text.
///
/// Naive next to Blink: a box at zero opacity, which the scene drops, adds no text, where Blink's
/// mask keeps it.
pub struct TextClip<'s> {
  node: &'s RenderNode,
  layout: ComputedLayout,
  root: &'s RenderNode,
  results: &'s LayoutResults,
  mask: TextMask,
}

impl<'s> TextClip<'s> {
  /// The text clip of the box `owner` paints in the scene of `root`, laid out in `results` and
  /// flattened into `chunks`, when its `background-clip` is `text`.
  pub fn of(
    root: &'s RenderNode,
    results: &'s LayoutResults,
    chunks: &[PaintChunk],
    owner: &NodePaint,
  ) -> Result<Option<Self>> {
    let Some(node) = root.node_at_path(&owner.path) else {
      return Ok(None);
    };

    if node.context.style.background_clip != BackgroundClip::Text || !node.paints_own_box() {
      return Ok(None);
    }

    Ok(Some(Self {
      node,
      layout: results.layout(owner.node_id)?,
      root,
      results,
      mask: TextMask::of(owner, chunks),
    }))
  }

  /// Fills the box's border box, at `origin`, with its background through the glyphs and
  /// decorations of its text.
  pub fn paint_background(&self, origin: Point<f32>, device: &mut dyn GlyphDevice) -> Result<()> {
    let context = &self.node.context;
    let background = BoxPainter::new(context, self.layout).background();
    let at = origin + background.offset;
    let clip = FillShape::Rect(background.size);
    let background = StripBackground {
      node: self.node,
      id: usize::MAX,
      background,
      strip: BoxFrame::new(self.layout, origin),
    };
    let mut result = Ok(());

    device.fill_text_clip(
      &background,
      &clip,
      Affine::translation(at.x, at.y),
      &mut |mask| {
        if result.is_ok() {
          result = self.mask.paint(self.root, self.results, origin, mask);
        }
      },
    );

    result
  }
}

/// The boxes drawing inline content into a text clip.
struct TextMask {
  contents: Vec<ClipContent>,
}

/// A box drawing its inline content into a [`TextMask`].
struct ClipContent {
  path: Vec<usize>,
  node_id: NodeId,
  /// Its border-box origin in the clipping box's space.
  origin: Point<f32>,
  /// Which of its inline content it draws.
  pass: InlinePass,
}

impl TextMask {
  /// The boxes inside the box `owner` paints, the box included, from the scene's `chunks`. Each sits
  /// where layout alone puts it, without the transforms between, as `kTextClip` places them.
  fn of(owner: &NodePaint, chunks: &[PaintChunk]) -> Self {
    let contents = chunks
      .iter()
      .filter_map(|chunk| {
        let pass = match chunk.part {
          ChunkPart::Content => InlinePass::Content,
          ChunkPart::Floats => InlinePass::Floats,
          ChunkPart::Decorations | ChunkPart::Outline => return None,
        };
        let node = chunk.node;

        node.path.starts_with(&owner.path).then(|| ClipContent {
          path: node.path.clone(),
          node_id: node.node_id,
          origin: node.layout_origin - owner.layout_origin,
          pass,
        })
      })
      .collect();

    Self { contents }
  }

  /// Paints the glyphs and decorations of the boxes in `root`, laid out in `results`, in black, the
  /// clipping box's border box at `offset` in the device's space.
  fn paint(
    &self,
    root: &RenderNode,
    results: &LayoutResults,
    offset: Point<f32>,
    device: &mut dyn GlyphDevice,
  ) -> Result<()> {
    for content in &self.contents {
      let Some(node) = root.node_at_path(&content.path) else {
        continue;
      };

      content.paint(node, results, offset, device)?;
    }

    Ok(())
  }
}

impl ClipContent {
  /// Paints `node`'s inline content, then the text inside the inline-level containers it places.
  fn paint(
    &self,
    node: &RenderNode,
    results: &LayoutResults,
    offset: Point<f32>,
    device: &mut dyn GlyphDevice,
  ) -> Result<()> {
    let context = &node.context;
    let layout = results.layout(self.node_id)?;
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some(items) = OwnContent::of(node).inline_items(&font_style) else {
      return Ok(());
    };
    let built = create_inline_layout(InlineLayoutRequest::in_content_box(
      items,
      layout.content_box_size(),
      &font_style,
      context,
      InlineLayoutMode::Draw,
    ));
    let runs = built.resolve_runs(context, layout)?;
    let origin = offset + self.origin;

    if self.pass == InlinePass::Content {
      runs.lines(layout, |_| true).paint_mask(
        &built.spans,
        &font_style,
        BoxFrame::new(layout, origin),
        device,
      );
    }

    for positioned in runs
      .inline_boxes
      .iter()
      .filter(|positioned| self.pass.paints(positioned))
    {
      let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(positioned.id as usize) else {
        continue;
      };
      let Some((box_origin, InlineBoxPaint::Container(subtree))) =
        resolve_inline_box(positioned, item, layout)
      else {
        continue;
      };
      let subtree_origin = origin + subtree.border_box_origin(box_origin);
      let scene = subtree.into_scene(Affine::IDENTITY, false)?;
      let chunks = PaintChunk::in_paint_order(&scene.contexts);
      let Some(subtree_root) = scene.contexts[0].root() else {
        continue;
      };

      TextMask::of(subtree_root, &chunks).paint(
        &scene.root,
        &scene.results,
        subtree_origin,
        device,
      )?;
    }

    Ok(())
  }
}
