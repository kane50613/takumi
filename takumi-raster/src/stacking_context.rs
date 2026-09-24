use takumi_core::{
  geometry::{ComputedLayout as Layout, NodeId, Point},
  layout::decoration::OutlineGeometry,
  scene::{NodePaint, PaintItem, PaintItemKind, Scene, SceneBounds, StackingContextNode},
};
use tiny_skia::{Pixmap, PixmapMut};

use crate::{
  BlurType, BorderProperties, Canvas, CanvasSubcanvas, CanvasViewport, Error, NodeMaskAction,
  Placement, Result, SizedFontStyle, apply_backdrop_filter, apply_filters_to_pixmap, blend_pixel,
  color_to_premultiplied, draw_box_shell, draw_debug_border, draw_node_content, draw_outline,
  inline_drawing::{draw_inline_box, draw_inline_layout},
  layout::{
    inline::{
      InlineLayoutMode, InlineLayoutRequest, ProcessedInlineSpan, collect_inline_items,
      create_inline_layout,
    },
    tree::{LayoutResults, RenderNode},
  },
  placement_overlap, prepare_node_mask, resolve_outline,
  style::{Affine, BlendMode, Filter, SizingContext},
};

pub(crate) fn blend_pixmap_software(
  dst: &mut Pixmap,
  src: &Pixmap,
  mode: BlendMode,
  offset: Point<i32>,
  opacity: f32,
) {
  if opacity <= 0.0 {
    return;
  }
  if offset.x >= dst.width() as i32 || offset.y >= dst.height() as i32 {
    return;
  }

  let Some(overlap) = placement_overlap(
    Placement {
      left: 0,
      top: 0,
      width: dst.width(),
      height: dst.height(),
    },
    Placement {
      left: offset.x,
      top: offset.y,
      width: src.width(),
      height: src.height(),
    },
  ) else {
    return;
  };
  let dst_left = overlap.lhs_offset.x as usize;
  let dst_top = overlap.lhs_offset.y as usize;
  let src_left = overlap.rhs_offset.x as usize;
  let src_top = overlap.rhs_offset.y as usize;
  let width = overlap.placement.width as usize;
  let height = overlap.placement.height as usize;

  let dst_width = dst.width() as usize;
  let src_width = src.width() as usize;
  let dst_pixels = dst.pixels_mut();
  let src_pixels = src.pixels();
  for row in 0..height {
    let dst_row = (dst_top + row) * dst_width;
    let src_row = (src_top + row) * src_width;
    for col in 0..width {
      let dst_pixel = &mut dst_pixels[dst_row + dst_left + col];
      let src_pixel = src_pixels[src_row + src_left + col];
      let s = src_pixel.demultiply();
      let d = dst_pixel.demultiply();
      let mut out = image::Rgba([d.red(), d.green(), d.blue(), d.alpha()]);
      let top = image::Rgba([
        s.red(),
        s.green(),
        s.blue(),
        ((s.alpha() as f32) * opacity).clamp(0.0, 255.0) as u8,
      ]);
      blend_pixel(&mut out, top, mode);
      *dst_pixel = color_to_premultiplied(out);
    }
  }
}

enum DeferredNodeRender {
  Deferred {
    path: Vec<usize>,
    finish: PendingFinish,
  },
  SkipRendering,
}

pub(crate) struct DeferredOutline {
  outline: OutlineGeometry,
  transform: Affine,
}

impl DeferredOutline {
  fn paint(&self, canvas: &mut Canvas) {
    draw_outline(&self.outline, self.transform, canvas);
  }
}

/// The state a painted node leaves open until its descendants are done: its
/// constraint mask, its isolation layer, and the bounds its filters cover.
struct PendingFinish {
  layout: Layout,
  has_constraint: bool,
  isolated_canvas: Option<Box<CanvasSubcanvas>>,
  filter_bounds: Option<SceneBounds>,
}

impl PendingFinish {
  /// Paints the outline and filters, pops the mask, and composites the layer.
  fn run(
    self,
    node: &mut RenderNode,
    canvas: &mut Canvas,
    outlines: Option<&mut Vec<DeferredOutline>>,
  ) -> Result<()> {
    // CSS 2.1 Appendix E paints the outline last, above the box's children, so a
    // node whose children follow it in the bucket hands its outline to the caller.
    if let Some((outline, transform)) = resolve_outline(&node.context, self.layout) {
      let deferred = DeferredOutline { outline, transform };

      match outlines {
        Some(outlines) => outlines.push(deferred),
        None => deferred.paint(canvas),
      }
    }

    if !node.context.style.filter.is_empty() {
      let viewport = canvas.viewport();
      let filter_padding = filter_padding(
        &node.context.style.filter,
        &node.context.sizing,
        node.context.transform,
      );
      let filter_region = self.filter_bounds.and_then(|bounds| {
        viewport
          .clamp_bounds(bounds, filter_padding)
          .map(|region| region.translate(-(viewport.origin.x as i32), -(viewport.origin.y as i32)))
      });

      if let Some(region) = filter_region
        && region != CanvasViewport::local(viewport.size).placement()
      {
        let mut region_raw = canvas.read_region(region);
        let Some(mut region_pixmap) =
          PixmapMut::from_bytes(&mut region_raw, region.width, region.height)
        else {
          return Ok(());
        };

        apply_filters_to_pixmap(
          &mut region_pixmap,
          &node.context.sizing,
          node.context.current_color,
          node.context.style.filter.iter(),
        )?;

        canvas.write_region(region, &region_raw);
      } else {
        canvas.with_pixmap(|pixmap| {
          let mut pixmap_mut = pixmap.as_mut();
          apply_filters_to_pixmap(
            &mut pixmap_mut,
            &node.context.sizing,
            node.context.current_color,
            node.context.style.filter.iter(),
          )
        })?;
      }
    }

    if self.has_constraint {
      canvas.pop_mask();
    }
    if let Some(isolated_canvas) = self.isolated_canvas {
      canvas.composite_subcanvas(
        *isolated_canvas,
        node.context.style.mix_blend_mode,
        node.context.style.opacity.0,
      );
    }

    Ok(())
  }
}

fn filter_padding(filters: &[Filter], sizing: &SizingContext, transform: Affine) -> i32 {
  let transform_scale = affine_max_scale(transform);
  filters
    .iter()
    .map(|filter| match filter {
      Filter::Blur(radius) => {
        (radius.to_px(sizing, 1.0) * BlurType::Filter.extent_multiplier() * transform_scale).ceil()
          as i32
      }
      Filter::DropShadow(shadow) => {
        let blur_spread = shadow.blur_radius.to_px(sizing, 1.0)
          * BlurType::Shadow.extent_multiplier()
          * transform_scale;
        let offset_x = shadow.offset_x.to_px(sizing, 1.0).abs() * transform_scale;
        let offset_y = shadow.offset_y.to_px(sizing, 1.0).abs() * transform_scale;
        (blur_spread + offset_x.max(offset_y)).ceil() as i32
      }
      _ => 0,
    })
    .sum()
}

fn affine_max_scale(transform: Affine) -> f32 {
  let s1 = transform.a * transform.a + transform.b * transform.b;
  let s2 = transform.c * transform.c + transform.d * transform.d;
  let off = transform.a * transform.c + transform.b * transform.d;
  let trace = s1 + s2;
  let half_trace = trace * 0.5;
  let det = s1 * s2 - off * off;
  let discriminant = (half_trace * half_trace - det).max(0.0);
  let sigma_max = (half_trace + discriminant.sqrt()).sqrt();
  if sigma_max.is_finite() {
    sigma_max.max(1.0)
  } else {
    1.0
  }
}

/// Paints a scene's stacking contexts, in paint order, onto one canvas.
pub(crate) struct ScenePainter<'a> {
  pub(crate) root: &'a mut RenderNode,
  pub(crate) contexts: &'a [StackingContextNode],
  pub(crate) layout_results: &'a LayoutResults,
  pub(crate) canvas: &'a mut Canvas,
}

impl<'a> ScenePainter<'a> {
  pub(crate) fn new(scene: &'a mut Scene, canvas: &'a mut Canvas) -> Self {
    Self {
      root: &mut scene.root,
      contexts: &scene.contexts,
      layout_results: &scene.results,
      canvas,
    }
  }

  pub(crate) fn paint_context(&mut self, context_id: usize) -> Result<()> {
    let contexts = self.contexts;
    let Some(context) = contexts.get(context_id) else {
      return Err(Error::InvalidLayoutNode(context_id as u64));
    };

    if let Some(bounds) = context.paint_bounds()
      && !self.canvas.viewport().intersects(bounds)
    {
      return Ok(());
    }

    let mut deferred_root = None;
    let mut outlines = Vec::new();

    if let Some(root_paint) = context.root() {
      match self.begin_node(root_paint, true, context.paint_bounds(), &mut outlines)? {
        Some(DeferredNodeRender::SkipRendering) => return Ok(()),
        Some(deferred_root_render @ DeferredNodeRender::Deferred { .. }) => {
          deferred_root = Some(deferred_root_render);
        }
        None => {}
      }
    }

    for bucket in context.in_paint_order() {
      self.paint_bucket(bucket, &mut outlines)?;
    }

    for outline in &outlines {
      outline.paint(self.canvas);
    }

    if let Some(DeferredNodeRender::Deferred { path, finish }) = deferred_root {
      let Some(current) = self.root.node_at_path_mut(&path) else {
        let node_id = context.root().map_or(NodeId::ROOT, |node| node.node_id);
        return Err(Error::InvalidLayoutNode(node_id.into()));
      };

      PendingFinish {
        filter_bounds: context.paint_bounds().or(finish.filter_bounds),
        ..finish
      }
      .run(current, self.canvas, None)?;
    }

    Ok(())
  }

  fn paint_bucket(
    &mut self,
    items: &[PaintItem],
    outlines: &mut Vec<DeferredOutline>,
  ) -> Result<()> {
    for item in items {
      match &item.kind {
        PaintItemKind::Node(node_paint) => {
          self.begin_node(node_paint, false, None, outlines)?;
        }
        PaintItemKind::Context(context_id) => {
          self.paint_context(*context_id)?;
        }
      }
    }
    Ok(())
  }

  fn begin_node(
    &mut self,
    node_paint: &NodePaint,
    defer_finish: bool,
    isolation_bounds_hint: Option<SceneBounds>,
    outlines: &mut Vec<DeferredOutline>,
  ) -> Result<Option<DeferredNodeRender>> {
    let canvas = &mut *self.canvas;
    let Some(current) = self.root.node_at_path_mut(&node_paint.path) else {
      return Err(Error::InvalidLayoutNode(node_paint.node_id.into()));
    };
    let layout = self.layout_results.layout(node_paint.node_id)?;
    if current.context.style.is_invisible() || !node_paint.transform.is_invertible() {
      return Ok(None);
    }

    // Prefer the context's merged bounds: a zero-sized root can still have visible overflowing children.
    if let Some(bounds) = isolation_bounds_hint.or(node_paint.paint_bounds)
      && !canvas.viewport().intersects(bounds)
    {
      return Ok(Some(DeferredNodeRender::SkipRendering));
    }

    current.context.sizing.set_container_size(
      node_paint.container_size.width,
      node_paint.container_size.height,
    );
    current.context.transform = node_paint.transform;

    if !current.context.style.backdrop_filter.is_empty() {
      // Filtered backdrop is clipped by the node's clip-path and mask, like Chromium's
      // backdrop root: https://drafts.fxtf.org/filter-effects-2/#BackdropRoot
      let node_mask = if current.context.style.has_shape_mask() {
        match prepare_node_mask(
          &current.context,
          &current.context.style,
          layout,
          node_paint.transform,
          canvas.viewport(),
        )? {
          NodeMaskAction::Shell(mask) => Some(mask),
          NodeMaskAction::SkipRendering => return Ok(Some(DeferredNodeRender::SkipRendering)),
          _ => None,
        }
      } else {
        None
      };

      let border = BorderProperties::from_context(&current.context, layout.size, layout.border);
      apply_backdrop_filter(
        canvas,
        border,
        layout.size,
        node_paint.transform,
        &current.context,
        node_mask.as_ref(),
      )?;
    }

    let isolated_canvas = if current.context.style.needs_offscreen_compositing() {
      let viewport = canvas.viewport();
      let bounds = isolation_bounds_hint
        .and_then(|bounds| viewport.clamp_bounds(bounds, 2))
        .unwrap_or_else(|| viewport.placement());

      Some(Box::new(canvas.begin_subcanvas(bounds)?))
    } else {
      None
    };

    let mask_action = prepare_node_mask(
      &current.context,
      &current.context.style,
      layout,
      node_paint.transform,
      canvas.viewport(),
    )?;
    if matches!(mask_action, NodeMaskAction::SkipRendering) {
      if let Some(isolated_canvas) = isolated_canvas {
        canvas.composite_subcanvas(*isolated_canvas, BlendMode::Normal, 0.0);
      }
      return Ok(Some(DeferredNodeRender::SkipRendering));
    }

    let has_constraint = mask_action.is_some();

    match mask_action {
      NodeMaskAction::None => {
        draw_render_node_shell(current, canvas, layout)?;
      }
      NodeMaskAction::Shell(mask) => {
        canvas.push_mask(mask);
        draw_render_node_shell(current, canvas, layout)?;
      }
      NodeMaskAction::Content(mask) => {
        draw_render_node_shell(current, canvas, layout)?;
        canvas.push_mask(mask);
      }
      NodeMaskAction::SkipRendering => return Ok(Some(DeferredNodeRender::SkipRendering)),
    }

    let finish = PendingFinish {
      layout,
      has_constraint,
      isolated_canvas,
      filter_bounds: node_paint.paint_bounds,
    };

    draw_render_node_content(current, canvas, layout)?;

    if current.context.draw_debug_border() {
      draw_debug_border(canvas, layout, node_paint.transform);
    }

    if current.should_create_inline_layout() {
      draw_render_node_inline(current, canvas, layout)?;
    } else if defer_finish {
      return Ok(Some(DeferredNodeRender::Deferred {
        path: node_paint.path.clone(),
        finish,
      }));
    }

    finish.run(current, canvas, Some(outlines))?;

    Ok(None)
  }
}

fn draw_render_node_shell(node: &RenderNode, canvas: &mut Canvas, layout: Layout) -> Result<()> {
  if node.node.is_none() {
    return Ok(());
  }

  draw_box_shell(&node.context, canvas, layout)
}

fn draw_render_node_content(node: &RenderNode, canvas: &mut Canvas, layout: Layout) -> Result<()> {
  if node.should_create_inline_layout() || node.has_anonymous_text_item_child() {
    return Ok(());
  }

  if let Some(inner) = &node.node {
    draw_node_content(inner, &node.context, canvas, layout)?;
  }
  Ok(())
}

fn draw_render_node_inline(
  node: &mut RenderNode,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  if node.context.style.opacity.0 == 0.0 {
    return Ok(());
  }

  let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);

  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    collect_inline_items(node),
    layout.unsnapped_content,
    &font_style,
    &node.context,
    InlineLayoutMode::Draw,
  ));
  let boxes = built.spans.iter().filter_map(|span| match span {
    ProcessedInlineSpan::Box(item) => Some(item),
    _ => None,
  });

  let positioned_inline_boxes =
    draw_inline_layout(&node.context, canvas, layout, &built, &font_style)?;

  for (item, positioned) in boxes.zip(positioned_inline_boxes.iter()) {
    draw_inline_box(positioned, item, layout, canvas, node.context.transform)?;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::error::Error;

  use tiny_skia::Pixmap;

  use super::{BlendMode, Point, blend_pixmap_software};
  use crate::{Fonts, RenderOptions, layout::node::Node, render, viewport::Viewport};

  type TestResult = Result<(), Box<dyn Error>>;

  fn render_json(json: &str) -> Result<image::RgbaImage, Box<dyn Error>> {
    let fonts = Fonts::default();
    let node: Node = serde_json::from_str(json)?;
    let options = RenderOptions::builder()
      .viewport(Viewport::new((100, 100)))
      .node(node)
      .fonts(&fonts)
      .build();
    Ok(render(options)?.into_rgba())
  }

  #[test]
  fn zero_sized_opacity_node_does_not_change_output() -> TestResult {
    let bar = r##"{"type": "container", "style": {"width": "20px", "height": "50px", "backgroundColor": "#3b82f6", "opacity": 0.9}, "children": []}"##;
    let zero_bar = r##"{"type": "container", "style": {"width": "20px", "height": "0px", "backgroundColor": "#3b82f6", "opacity": 0.9}, "children": []}"##;
    let tree = |bars: &str| {
      format!(
        r##"{{"type": "container", "style": {{"display": "flex", "alignItems": "flex-end", "width": "100%", "height": "100%", "backgroundColor": "#ffffff"}}, "children": [{bars}]}}"##
      )
    };

    let with_zero = render_json(&tree(&format!("{bar}, {zero_bar}")))?;
    let without_zero = render_json(&tree(bar))?;

    assert_eq!(with_zero, without_zero);
    Ok(())
  }

  #[test]
  fn zero_sized_opacity_parent_still_paints_overflowing_child() -> TestResult {
    let image = render_json(
      r##"{
        "type": "container",
        "style": {"display": "flex", "width": "0px", "height": "0px", "opacity": 0.5},
        "children": [
          {"type": "container", "style": {"width": "50px", "height": "50px", "flexShrink": 0, "backgroundColor": "#ff0000"}, "children": []}
        ]
      }"##,
    )?;

    let pixel = image.get_pixel(10, 10);
    assert!(
      pixel.0[0] > 0 && pixel.0[3] > 0,
      "overflowing child of zero-sized opacity parent must still paint, got {pixel:?}"
    );
    Ok(())
  }

  #[test]
  fn blending_outside_the_destination_leaves_pixels_unchanged() {
    let mut dst = Pixmap::new(1, 1).unwrap();
    dst.data_mut().copy_from_slice(&[1, 2, 3, 4]);
    let mut src = Pixmap::new(1, 1).unwrap();
    src.data_mut().copy_from_slice(&[5, 6, 7, 8]);

    blend_pixmap_software(
      &mut dst,
      &src,
      BlendMode::Normal,
      Point::new(i32::MAX, i32::MAX),
      1.0,
    );

    assert_eq!(dst.data(), &[1, 2, 3, 4]);
  }
}
