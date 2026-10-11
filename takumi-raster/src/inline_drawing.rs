use takumi_core::{
  geometry::{ComputedLayout as Layout, Point},
  layout::inline_box::{InlineBoxPaint, resolve_inline_box},
};

use crate::{
  Canvas, CanvasDevice, DeferredOutline, RenderContext, Result, SizedFontStyle, draw_box_shell,
  layout::{
    inline::{InlineBoxItem, InlinePass, ProcessedInlineSpan, VisualInlineBox},
    tree::RenderNode,
  },
  node_paint::draw_image_node_content,
  painter::{BoxFrame, OwnContent, PaintedInline},
  stacking_context::paint_scene,
  style::Affine,
};

pub(crate) fn draw_inline_box(
  inline_box: &VisualInlineBox,
  item: &InlineBoxItem<'_>,
  container: Layout,
  canvas: &mut Canvas,
  transform: Affine,
) -> Result<()> {
  let Some((origin, paint)) = resolve_inline_box(inline_box, item, container) else {
    return Ok(());
  };

  match paint {
    InlineBoxPaint::Container(subtree) => {
      let at = subtree.border_box_origin(origin);
      let mut scene = subtree.into_scene(transform * Affine::translation(at.x, at.y), true)?;

      paint_scene(&mut scene, canvas)
    }
    InlineBoxPaint::Replaced { node, layout } => {
      if !node.paints_own_box() {
        return Ok(());
      }
      let mut context = node.context.clone();
      context.transform = transform * Affine::translation(origin.x, origin.y);

      draw_box_shell(&context, canvas, layout, None)?;
      draw_own_content(node, &context, canvas, layout, InlinePass::Content)?;
      if let Some(outline) = DeferredOutline::of(&context, layout) {
        outline.paint(canvas)?;
      }
      Ok(())
    }
  }
}

/// Draws the node's own image or inline content in `context`, then the inline boxes the content
/// places.
pub(crate) fn draw_own_content(
  node: &RenderNode,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  pass: InlinePass,
) -> Result<()> {
  let content = OwnContent::of(node);

  if let OwnContent::Image(image) = content {
    return match pass {
      InlinePass::Content => draw_image_node_content(image, context, canvas, layout),
      InlinePass::Floats => Ok(()),
    };
  }

  let font_style = SizedFontStyle::from_style(&context.style, context);
  let Some(painted) = content.lay_out_inline(&font_style, layout) else {
    return Ok(());
  };
  let painted = painted?;

  if pass == InlinePass::Content {
    draw_inline_layout(context, canvas, layout, &painted, &font_style)?;
  }

  for positioned in painted
    .runs
    .inline_boxes
    .iter()
    .filter(|positioned| pass.paints(positioned))
  {
    if let Some(ProcessedInlineSpan::Box(item)) = painted.spans.get(positioned.id as usize) {
      draw_inline_box(positioned, item, layout, canvas, context.transform)?;
    }
  }
  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  painted: &PaintedInline<'_>,
  font_style: &SizedFontStyle,
) -> Result<()> {
  let mut device = CanvasDevice::of(canvas, context);

  painted.runs.paint(
    &painted.spans,
    font_style,
    BoxFrame::new(layout, Point::ZERO),
    &mut device,
  );
  device.finish()
}
