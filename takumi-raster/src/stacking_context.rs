use takumi_core::{
  geometry::{ComputedLayout as Layout, NodeId, Point, Size, transformed_rect_extents},
  layout::decoration::OutlineGeometry,
  scene::{NodePaint, PaintItem, PaintItemKind, SceneBounds, StackingContextNode},
};
use tiny_skia::{Pixmap, PixmapMut};

use crate::{
  BlurType, BorderProperties, Canvas, CanvasSubcanvas, CanvasViewport, Error, NodeMaskAction,
  Placement, Result, SizedFontStyle, apply_backdrop_filter, apply_filters_to_pixmap, blend_pixel,
  color_to_premultiplied, draw_background, draw_border, draw_debug_border, draw_inset_box_shadow,
  draw_node_content, draw_outline, draw_outset_box_shadow,
  inline_drawing::{draw_inline_box, draw_inline_layout},
  layout::{
    inline::{
      InlineLayoutMode, InlineLayoutRequest, ProcessedInlineSpan, collect_inline_items,
      create_inline_layout,
    },
    tree::{LayoutResults, RenderNode},
  },
  prepare_node_mask, resolve_outline,
  style::{Affine, BackgroundImage, BlendMode, Filter, SizingContext},
};

fn bounds_intersects_viewport(bounds: SceneBounds, viewport: CanvasViewport) -> bool {
  if bounds.is_empty() {
    return false;
  }

  let viewport_left = viewport.origin.x as i32;
  let viewport_top = viewport.origin.y as i32;
  let viewport_right = viewport.right();
  let viewport_bottom = viewport.bottom();

  bounds.right as i32 > viewport_left
    && bounds.bottom as i32 > viewport_top
    && (bounds.left as i32) < viewport_right
    && (bounds.top as i32) < viewport_bottom
}

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

  let Some(OverlapRegion {
    dst_left,
    dst_top,
    src_left,
    src_top,
    width,
    height,
  }) = overlapping_region(dst, src, offset)
  else {
    return;
  };

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

fn overlapping_region(dst: &Pixmap, src: &Pixmap, offset: Point<i32>) -> Option<OverlapRegion> {
  let dst_left = offset.x.max(0) as usize;
  let dst_top = offset.y.max(0) as usize;
  let src_left = (-offset.x).max(0) as usize;
  let src_top = (-offset.y).max(0) as usize;
  let width = (dst.width() as i32 - offset.x.max(0))
    .min(src.width() as i32 - src_left as i32)
    .max(0) as usize;
  let height = (dst.height() as i32 - offset.y.max(0))
    .min(src.height() as i32 - src_top as i32)
    .max(0) as usize;

  if width == 0 || height == 0 {
    return None;
  }

  Some(OverlapRegion {
    dst_left,
    dst_top,
    src_left,
    src_top,
    width,
    height,
  })
}

struct OverlapRegion {
  dst_left: usize,
  dst_top: usize,
  src_left: usize,
  src_top: usize,
  width: usize,
  height: usize,
}
enum DeferredNodeRender {
  Deferred {
    path: Vec<usize>,
    layout: Layout,
    has_constraint: bool,
    isolated_canvas: Option<CanvasSubcanvas>,
    filter_bounds: Option<SceneBounds>,
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

fn finish_node_render(
  node: &mut RenderNode,
  canvas: &mut Canvas,
  layout: Layout,
  has_constraint: bool,
  isolated_canvas: Option<CanvasSubcanvas>,
  filter_bounds: Option<SceneBounds>,
  outlines: Option<&mut Vec<DeferredOutline>>,
) -> Result<()> {
  // CSS 2.1 Appendix E paints the outline last, above the box's children, so a
  // node whose children follow it in the bucket hands its outline to the caller.
  if let Some((outline, transform)) = resolve_outline(&node.context, layout) {
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
    let filter_region = filter_bounds.and_then(|bounds| {
      let left = (bounds.left as i32 - filter_padding).max(viewport.origin.x as i32);
      let top = (bounds.top as i32 - filter_padding).max(viewport.origin.y as i32);
      let right = (bounds.right as i32 + filter_padding).min(viewport.right());
      let bottom = (bounds.bottom as i32 + filter_padding).min(viewport.bottom());

      (left < right && top < bottom).then_some(Placement {
        left: left - viewport.origin.x as i32,
        top: top - viewport.origin.y as i32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
      })
    });

    let canvas_size = canvas.viewport().size;
    let region_is_full_canvas = filter_region.is_some_and(|r| {
      r.left == 0 && r.top == 0 && r.width == canvas_size.width && r.height == canvas_size.height
    });

    if let Some(region) = filter_region
      && !region_is_full_canvas
    {
      let row_bytes = region.width as usize * 4;
      let region_len = row_bytes * region.height as usize;
      let mut region_raw = vec![0; region_len];

      canvas.with_pixmap_ref(|pixmap| {
        let canvas_width = pixmap.width() as usize;
        let canvas_raw: &[u8] = bytemuck::cast_slice(pixmap.pixels());
        for (y, dest_row) in region_raw.chunks_exact_mut(row_bytes).enumerate() {
          let src_y = region.top as usize + y;
          let src_start = (src_y * canvas_width + region.left as usize) * 4;
          dest_row.copy_from_slice(&canvas_raw[src_start..src_start + row_bytes]);
        }
      });

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

      canvas.with_pixmap(|pixmap| {
        let canvas_width = pixmap.width() as usize;
        let canvas_raw: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
        for (y, src_row) in region_raw.chunks_exact(row_bytes).enumerate() {
          let dst_y = region.top as usize + y;
          let dst_start = (dst_y * canvas_width + region.left as usize) * 4;
          canvas_raw[dst_start..dst_start + row_bytes].copy_from_slice(src_row);
        }
      });
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

  if let Some(isolated_canvas) = isolated_canvas {
    if has_constraint {
      canvas.pop_mask();
    }
    canvas.composite_subcanvas(
      isolated_canvas,
      node.context.style.mix_blend_mode,
      node.context.style.opacity.0,
    );
  } else if has_constraint {
    canvas.pop_mask();
  }

  Ok(())
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

fn begin_node_render(
  root: &mut RenderNode,
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  node_paint: &NodePaint,
  defer_finish: bool,
  isolation_bounds_hint: Option<SceneBounds>,
  outlines: &mut Vec<DeferredOutline>,
) -> Result<Option<DeferredNodeRender>> {
  let Some(current) = root.node_at_path_mut(&node_paint.path) else {
    return Err(Error::InvalidLayoutNode(node_paint.node_id.into()));
  };
  let layout = layout_results.layout(node_paint.node_id)?;

  if current.context.style.is_invisible() || !node_paint.transform.is_invertible() {
    return Ok(None);
  }

  // Prefer the context's merged bounds: a zero-sized root can still have visible overflowing children.
  if let Some(bounds) = isolation_bounds_hint.or(node_paint.paint_bounds)
    && !bounds_intersects_viewport(bounds, canvas.viewport())
  {
    return Ok(Some(DeferredNodeRender::SkipRendering));
  }

  current
    .context
    .sizing
    .set_container_size(node_paint.container_size.0, node_paint.container_size.1);
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

  let should_isolate = current.context.style.needs_offscreen_compositing();
  let isolated_canvas = if should_isolate {
    Some(canvas.begin_subcanvas(compute_isolation_bounds(
      current,
      layout.size,
      node_paint.transform,
      canvas.viewport(),
      isolation_bounds_hint,
    ))?)
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
      canvas.composite_subcanvas(isolated_canvas, BlendMode::Normal, 0.0);
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

  draw_render_node_content(current, canvas, layout)?;

  if current.context.draw_debug_border() {
    draw_debug_border(canvas, layout, node_paint.transform);
  }

  if current.should_create_inline_layout() {
    draw_render_node_inline(current, canvas, layout)?;
    finish_node_render(
      current,
      canvas,
      layout,
      has_constraint,
      isolated_canvas,
      node_paint.paint_bounds,
      Some(outlines),
    )?;
  } else if !defer_finish {
    finish_node_render(
      current,
      canvas,
      layout,
      has_constraint,
      isolated_canvas,
      node_paint.paint_bounds,
      Some(outlines),
    )?;
  } else {
    return Ok(Some(DeferredNodeRender::Deferred {
      path: node_paint.path.clone(),
      layout,
      has_constraint,
      isolated_canvas,
      filter_bounds: node_paint.paint_bounds,
    }));
  }

  Ok(None)
}

fn paint_single_node(
  root: &mut RenderNode,
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  node_paint: &NodePaint,
  outlines: &mut Vec<DeferredOutline>,
) -> Result<()> {
  match begin_node_render(
    root,
    layout_results,
    canvas,
    node_paint,
    false,
    None,
    outlines,
  )? {
    Some(DeferredNodeRender::SkipRendering) | None => {}
    Some(DeferredNodeRender::Deferred {
      path,
      layout,
      has_constraint,
      isolated_canvas,
      filter_bounds,
    }) => {
      let Some(current) = root.node_at_path_mut(&path) else {
        return Err(Error::InvalidLayoutNode(node_paint.node_id.into()));
      };
      finish_node_render(
        current,
        canvas,
        layout,
        has_constraint,
        isolated_canvas,
        filter_bounds,
        None,
      )?;
    }
  }
  Ok(())
}

fn paint_bucket(
  root: &mut RenderNode,
  contexts: &[StackingContextNode],
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  items: &[PaintItem],
  outlines: &mut Vec<DeferredOutline>,
) -> Result<()> {
  for item in items {
    match &item.kind {
      PaintItemKind::Node(node_paint) => {
        paint_single_node(root, layout_results, canvas, node_paint, outlines)?;
      }
      PaintItemKind::Context(context_id) => {
        paint_context(root, contexts, layout_results, canvas, *context_id)?;
      }
    }
  }
  Ok(())
}

pub(crate) fn paint_context(
  root: &mut RenderNode,
  contexts: &[StackingContextNode],
  layout_results: &LayoutResults,
  canvas: &mut Canvas,
  context_id: usize,
) -> Result<()> {
  let Some(context) = contexts.get(context_id) else {
    return Err(Error::InvalidLayoutNode(context_id as u64));
  };

  if let Some(bounds) = context.paint_bounds()
    && !bounds_intersects_viewport(bounds, canvas.viewport())
  {
    return Ok(());
  }

  let mut deferred_root = None;
  let mut outlines = Vec::new();

  if let Some(root_paint) = context.root() {
    match begin_node_render(
      root,
      layout_results,
      canvas,
      root_paint,
      true,
      context.paint_bounds(),
      &mut outlines,
    )? {
      Some(DeferredNodeRender::SkipRendering) => return Ok(()),
      Some(deferred_root_render @ DeferredNodeRender::Deferred { .. }) => {
        deferred_root = Some(deferred_root_render);
      }
      None => {}
    }
  }

  for bucket in context.in_paint_order() {
    paint_bucket(
      root,
      contexts,
      layout_results,
      canvas,
      bucket,
      &mut outlines,
    )?;
  }

  for outline in &outlines {
    outline.paint(canvas);
  }

  if let Some(DeferredNodeRender::Deferred {
    path,
    layout,
    has_constraint,
    isolated_canvas,
    filter_bounds,
  }) = deferred_root
  {
    let Some(current) = root.node_at_path_mut(&path) else {
      let node_id = context.root().map_or(NodeId::ROOT, |node| node.node_id);
      return Err(Error::InvalidLayoutNode(node_id.into()));
    };
    finish_node_render(
      current,
      canvas,
      layout,
      has_constraint,
      isolated_canvas,
      context.paint_bounds().or(filter_bounds),
      None,
    )?;
  }

  Ok(())
}

fn supports_bounds_hint(node: &RenderNode, require_child_clipping: bool) -> bool {
  let style = &node.context.style;
  let has_children = node
    .children
    .as_ref()
    .is_some_and(|children| !children.is_empty());
  let clips_children = style.resolve_overflows().should_clip_content();
  let has_box_shadow = style
    .box_shadow
    .as_ref()
    .is_some_and(|shadows| !shadows.is_empty());
  let has_outline = style.outline_style.is_rendered();
  let has_text_shadow = style
    .text_shadow
    .as_ref()
    .is_some_and(|shadows| !shadows.is_empty());
  let has_text_stroke = style
    .webkit_text_stroke_width
    .is_some_and(|width| width != Default::default());
  let has_spread_background = style.background_image.as_ref().is_some_and(|images| {
    images.iter().any(|image| match image {
      BackgroundImage::Linear(gradient) => gradient.repeating,
      BackgroundImage::Radial(gradient) => gradient.repeating,
      BackgroundImage::Conic(gradient) => gradient.repeating,
      _ => false,
    })
  });

  style.filter.is_empty()
    && style.backdrop_filter.is_empty()
    && !style.has_shape_mask()
    && !has_box_shadow
    && !has_outline
    && !has_text_shadow
    && !has_text_stroke
    && !has_spread_background
    && (!require_child_clipping || !has_children || clips_children)
}

fn placement_from_bounds(
  bounds: SceneBounds,
  viewport: CanvasViewport,
  padding: i32,
) -> Option<Placement> {
  let left = (bounds.left as i32 - padding).max(viewport.origin.x as i32);
  let top = (bounds.top as i32 - padding).max(viewport.origin.y as i32);
  let right = (bounds.right as i32 + padding).min(viewport.right());
  let bottom = (bounds.bottom as i32 + padding).min(viewport.bottom());

  Placement::from_bounds(left, top, right, bottom)
}

fn full_viewport_placement(viewport: CanvasViewport) -> Placement {
  Placement {
    left: viewport.origin.x as i32,
    top: viewport.origin.y as i32,
    width: viewport.size.width,
    height: viewport.size.height,
  }
}

fn compute_isolation_bounds(
  node: &RenderNode,
  size: Size<f32>,
  transform: Affine,
  viewport: CanvasViewport,
  paint_bounds_hint: Option<SceneBounds>,
) -> Placement {
  let placement = if supports_bounds_hint(node, false) {
    paint_bounds_hint.and_then(|bounds| placement_from_bounds(bounds, viewport, 2))
  } else if supports_bounds_hint(node, true) {
    transformed_rect_extents(Point::ZERO, size, transform).and_then(
      |(min_x, min_y, max_x, max_y)| {
        let left = min_x.floor().max(viewport.origin.x as f32) as i32;
        let top = min_y.floor().max(viewport.origin.y as f32) as i32;
        let right = max_x.ceil().min(viewport.right() as f32) as i32;
        let bottom = max_y.ceil().min(viewport.bottom() as f32) as i32;

        Placement::from_bounds(left, top, right, bottom)
      },
    )
  } else {
    None
  };

  placement.unwrap_or_else(|| full_viewport_placement(viewport))
}

fn draw_render_node_shell(node: &RenderNode, canvas: &mut Canvas, layout: Layout) -> Result<()> {
  if node.node.is_none() {
    return Ok(());
  }

  draw_outset_box_shadow(&node.context, canvas, layout)?;
  draw_background(&node.context, canvas, layout)?;
  draw_inset_box_shadow(&node.context, canvas, layout)?;
  draw_border(&node.context, canvas, layout)?;
  Ok(())
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
    layout.content_box_size(),
    &font_style,
    &node.context,
    InlineLayoutMode::Draw,
  ));
  let inline_layout_box = layout;

  let boxes = built.spans.iter().filter_map(|span| match span {
    ProcessedInlineSpan::Box(item) => Some(item),
    _ => None,
  });

  let positioned_inline_boxes = draw_inline_layout(
    &node.context,
    canvas,
    inline_layout_box,
    &built,
    &font_style,
  )?;

  for (item, positioned) in boxes.zip(positioned_inline_boxes.iter()) {
    draw_inline_box(
      positioned,
      item,
      inline_layout_box,
      canvas,
      node.context.transform,
    )?;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::error::Error;

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
}
