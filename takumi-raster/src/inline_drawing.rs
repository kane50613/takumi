use takumi_core::{
  geometry::{ComputedLayout as Layout, Point},
  layout::inline_box::{InlineBoxPaint, resolve_inline_box},
};

use crate::{
  BorderProperties, Canvas, CanvasDevice, DeferredOutline, PaintSource, RenderContext, Result,
  SizedFontStyle, collect_background_layers, draw_box_shell,
  layout::{
    inline::{
      BuiltInlineLayout, InlineBoxItem, InlineLayoutMode, InlineLayoutRequest, InlinePass,
      ProcessedInlineSpan, VisualInlineBox, create_inline_layout,
    },
    tree::RenderNode,
  },
  node_paint::draw_image_node_content,
  painter::{BoxFrame, BoxPainter, GlyphFill, OwnContent},
  rasterize_layers,
  stacking_context::paint_scene,
  style::{Affine, BackgroundClip},
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

      draw_box_shell(&context, canvas, layout)?;
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
  let Some(items) = content.inline_items(&font_style) else {
    return Ok(());
  };
  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    items,
    layout.unsnapped_content,
    &font_style,
    context,
    InlineLayoutMode::Draw,
  ));
  let boxes = built.spans.iter().filter_map(|span| match span {
    ProcessedInlineSpan::Box(item) => Some(item),
    _ => None,
  });
  let positioned_inline_boxes = match pass {
    InlinePass::Content => draw_inline_layout(context, canvas, layout, &built, &font_style)?,
    InlinePass::Floats => built.resolve_runs(context, layout)?.inline_boxes,
  };

  for (item, positioned) in boxes.zip(positioned_inline_boxes.iter()) {
    if pass.paints(positioned) {
      draw_inline_box(positioned, item, layout, canvas, context.transform)?;
    }
  }
  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  built: &BuiltInlineLayout<'_>,
  font_style: &SizedFontStyle,
) -> Result<Vec<VisualInlineBox>> {
  let resolved = built.resolve_runs(context, layout)?;
  let text_background = if context.style.background_clip == BackgroundClip::Text {
    let layers =
      collect_background_layers(&BoxPainter::new(context, layout).background(), context)?;

    rasterize_layers(
      layers,
      layout.size.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
    )?
  } else {
    None
  };
  let fill = if text_background.is_some() {
    GlyphFill::Background
  } else {
    GlyphFill::Text
  };
  let mut device = CanvasDevice::of(canvas, context);

  device.text_background = text_background.as_ref().map(PaintSource::from);
  resolved.paint(
    &built.spans,
    font_style,
    fill,
    BoxFrame::new(layout, Point::ZERO),
    &mut device,
  );
  device.finish()?;

  Ok(resolved.inline_boxes)
}
