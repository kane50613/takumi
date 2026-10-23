//! The text a box's `background-clip: text` background shows through: the inline content of the
//! box and of every box inside it, as Blink's `kTextClip` paint phase walks them.

use super::{
  BoxFrame, BoxPainter, FillShape, GlyphDevice, OwnContent, PaintedInline, StripBackground,
};
use crate::{
  context::RenderContext,
  error::{Error, Result},
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, NodeId, Point},
  layout::{
    layout_view::LayoutView,
    tree::{ContainingBlocks, RenderNode},
  },
  scene::NodePaint,
  style::{Affine, BackgroundClip, Display},
};

/// A box whose background shows only through the text inside it, and that text.
pub struct TextClip<'s> {
  node: &'s RenderNode,
  context: &'s RenderContext,
  node_id: NodeId,
  layout: ComputedLayout,
  results: LayoutView<'s>,
}

impl<'s> TextClip<'s> {
  /// The text clip of the box `owner` paints in `root` under `context`, laid out in `results`,
  /// when its `background-clip` is `text`.
  pub fn of(
    root: &'s RenderNode,
    results: LayoutView<'s>,
    owner: &NodePaint,
    context: &'s RenderContext,
  ) -> Result<Option<Self>> {
    let Some(node) = root.node_at_path(&owner.path) else {
      return Ok(None);
    };

    if node.context.style.background_data.background_clip != BackgroundClip::Text
      || !node.paints_own_box()
    {
      return Ok(None);
    }

    Ok(Some(Self {
      node,
      context,
      node_id: owner.node_id,
      layout: results.layout(owner.node_id)?,
      results,
    }))
  }

  /// Fills the box's border box, at `origin`, with its background through the glyphs and
  /// decorations of its text.
  pub fn paint_background(&self, origin: Point<f32>, device: &mut dyn GlyphDevice) -> Result<()> {
    let context = self.context;
    let background = BoxPainter::new(context, self.layout).background();
    let at = origin + background.offset;
    let clip = FillShape::Rect(background.size);
    let background = StripBackground {
      node: self.node,
      context,
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
          result =
            TextMask::new(self.results).paint(self.node, self.context, self.node_id, origin, mask);
        }
      },
    );

    result
  }
}

/// The boxes of one layout drawing their text into a text clip, as `kTextClip` reaches them: every
/// box but a float in a layer of its own, and an out-of-flow box only when its containing block is
/// one of them laying out no inline content. Each sits where layout alone puts it, without its
/// transform or effects.
struct TextMask<'r> {
  results: LayoutView<'r>,
  containing_blocks: ContainingBlocks<Point<f32>>,
}

impl<'r> TextMask<'r> {
  fn new(results: LayoutView<'r>) -> Self {
    Self {
      results,
      containing_blocks: ContainingBlocks::default(),
    }
  }

  /// Paints the text of `node` under `context`, laid out as `node_id` with its border box at
  /// `origin`, and of the boxes inside it, in black.
  fn paint(
    &mut self,
    node: &RenderNode,
    context: &RenderContext,
    node_id: NodeId,
    origin: Point<f32>,
    device: &mut dyn GlyphDevice,
  ) -> Result<()> {
    self.paint_inline_content(node, context, node_id, origin, device)?;

    if node.children.is_none() {
      return Ok(());
    }

    let is_inline_formatting_context = node.should_create_inline_layout();

    self.containing_blocks.record_placement(node_id, origin);

    let is_flex_or_grid_item = node
      .context
      .style
      .base_data
      .display
      .should_blockify_children();

    for child in self.results.box_children(node_id)? {
      let Some(base) = self
        .containing_blocks
        .recorded_placement_for(self.results.hoisted_cb(node_id, child), origin)
      else {
        continue;
      };
      let mut path = Vec::new();

      child.extend_path(&mut path);

      let Some(child_node) = node.node_at_path(&path) else {
        return Err(Error::InvalidLayoutNode(child.node_id.into()));
      };

      // Of an inline formatting context's boxes, only its inline content boxes draw text in.
      if is_inline_formatting_context && !child_node.is_inline_content_box() {
        continue;
      }

      let child_layout = self.results.layout(child.node_id)?;
      let child_context = &child_node.context;

      if child_context.style.base_data.display == Display::None
        || child_context
          .style
          .floats_in_own_layer(is_flex_or_grid_item)
      {
        continue;
      }

      self.paint(
        child_node,
        child_context,
        child.node_id,
        base + child_layout.location,
        device,
      )?;
    }

    Ok(())
  }

  /// Paints `node`'s inline content, laid out as `node_id` with its border box at `origin`.
  fn paint_inline_content(
    &self,
    node: &RenderNode,
    context: &RenderContext,
    node_id: NodeId,
    origin: Point<f32>,
    device: &mut dyn GlyphDevice,
  ) -> Result<()> {
    let layout = self.results.layout(node_id)?;
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some(painted) = OwnContent::of(node).lay_out_inline(&font_style, layout) else {
      return Ok(());
    };
    let PaintedInline { spans, runs, .. } = painted?;

    runs.lines(layout, |_| true).paint_mask(
      &spans,
      &font_style,
      BoxFrame::new(layout, origin),
      device,
    );

    Ok(())
  }
}
