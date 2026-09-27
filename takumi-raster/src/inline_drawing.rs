use takumi_core::{
  geometry::{ComputedLayout as Layout, Point},
  layout::inline_box::{InlineBoxPaint, resolve_inline_box},
};

use crate::{
  BorderProperties, Canvas, CanvasDevice, DeferredOutline, PaintSource, RenderContext, Result,
  SizedFontStyle, collect_background_layers, draw_box_shell, draw_node_content,
  layout::inline::{BuiltInlineLayout, InlineBoxItem, VisualInlineBox},
  painter::{BoxFrame, BoxPainter, GlyphFill},
  rasterize_layers,
  stacking_context::ScenePainter,
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

      ScenePainter::new(&mut scene, canvas).paint_context(0)
    }
    InlineBoxPaint::Replaced { node, layout } => {
      let Some(source) = &node.node else {
        return Ok(());
      };
      let mut context = node.context.clone();
      context.transform = transform * Affine::translation(origin.x, origin.y);

      draw_box_shell(&context, canvas, layout)?;
      draw_node_content(source, &context, canvas, layout)?;
      if let Some(outline) = DeferredOutline::of(&context, layout) {
        outline.paint(canvas);
      }
      Ok(())
    }
  }
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
