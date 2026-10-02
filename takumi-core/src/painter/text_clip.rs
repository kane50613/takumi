//! The text a box's `background-clip: text` background shows through: the inline content of the
//! box and of every box inside it, as Blink's `kTextClip` paint phase walks them.

use parley::InlineBoxKind;

use super::{BoxFrame, BoxPainter, FillShape, GlyphDevice, OwnContent, StripBackground};
use crate::{
  error::{Error, Result},
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, NodeId, Point},
  layout::{
    inline::{InlineLayoutMode, InlineLayoutRequest, ProcessedInlineSpan, create_inline_layout},
    inline_box::{InlineBoxPaint, InlineSubtree},
    tree::{ContainingBlocks, LayoutResults, RenderNode},
  },
  scene::NodePaint,
  style::{Affine, BackgroundClip, Display},
};

/// A box whose background shows only through the text inside it, and that text.
pub struct TextClip<'s> {
  node: &'s RenderNode,
  node_id: NodeId,
  layout: ComputedLayout,
  results: &'s LayoutResults,
}

impl<'s> TextClip<'s> {
  /// The text clip of the box `owner` paints in `root`, laid out in `results`, when its
  /// `background-clip` is `text`.
  pub fn of(
    root: &'s RenderNode,
    results: &'s LayoutResults,
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
      node_id: owner.node_id,
      layout: results.layout(owner.node_id)?,
      results,
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
          result = TextMask::new(self.results).paint(self.node, self.node_id, origin, mask);
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
  results: &'r LayoutResults,
  containing_blocks: ContainingBlocks<Point<f32>>,
}

impl<'r> TextMask<'r> {
  fn new(results: &'r LayoutResults) -> Self {
    Self {
      results,
      containing_blocks: ContainingBlocks::default(),
    }
  }

  /// Paints the text of `node`, laid out as `node_id` with its border box at `origin`, and of the
  /// boxes inside it, in black.
  fn paint(
    &mut self,
    node: &RenderNode,
    node_id: NodeId,
    origin: Point<f32>,
    device: &mut dyn GlyphDevice,
  ) -> Result<()> {
    self.paint_inline_content(node, node_id, origin, device)?;

    if node.children.is_none() || node.should_create_inline_layout() {
      return Ok(());
    }

    self.containing_blocks.record_placement(node_id, origin);

    let is_flex_or_grid_item = node.context.style.display.should_blockify_children();

    for child in self.results.box_children(node_id)? {
      let Some(base) = self.containing_blocks.recorded_placement_for(child, origin) else {
        continue;
      };
      let mut path = Vec::new();

      child.extend_path(&mut path);

      let Some(child_node) = node.node_at_path(&path) else {
        return Err(Error::InvalidLayoutNode(child.node_id.into()));
      };
      let child_layout = self.results.layout(child.node_id)?;
      let child_context = &child_node.context;

      if child_context.style.display == Display::None
        || child_context.style.floats_in_own_layer(
          child_layout.size.width,
          child_layout.size.height,
          &child_context.sizing,
          is_flex_or_grid_item,
        )
      {
        continue;
      }

      self.paint(
        child_node,
        child.node_id,
        base + child_layout.location,
        device,
      )?;
    }

    Ok(())
  }

  /// Paints `node`'s inline content, laid out as `node_id` with its border box at `origin`, then
  /// the text of the inline-level containers it places.
  fn paint_inline_content(
    &self,
    node: &RenderNode,
    node_id: NodeId,
    origin: Point<f32>,
    device: &mut dyn GlyphDevice,
  ) -> Result<()> {
    let context = &node.context;
    let layout = self.results.layout(node_id)?;
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

    runs.lines(layout, |_| true).paint_mask(
      &built.spans,
      &font_style,
      BoxFrame::new(layout, origin),
      device,
    );

    for positioned in runs
      .inline_boxes
      .iter()
      .filter(|positioned| positioned.kind != InlineBoxKind::OutOfFlow)
    {
      let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(positioned.id as usize) else {
        continue;
      };
      let (box_origin, InlineBoxPaint::Container(subtree)) =
        InlineBoxPaint::of(positioned, item, layout)
      else {
        continue;
      };
      let root_context = &subtree.root.context;

      if root_context.style.floats_in_own_layer(
        subtree.size.width,
        subtree.size.height,
        &root_context.sizing,
        false,
      ) {
        continue;
      }

      TextMask::new(&subtree.results).paint(
        &subtree.root,
        InlineSubtree::ROOT,
        origin + subtree.border_box_origin(box_origin),
        device,
      )?;
    }

    Ok(())
  }
}
