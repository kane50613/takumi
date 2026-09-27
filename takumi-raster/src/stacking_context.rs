use takumi_core::{
  geometry::{ComputedLayout as Layout, NodeId, Point},
  scene::{NodePaint, PaintItem, PaintItemKind, Scene, SceneBounds, StackingContextNode},
};
use tiny_skia::{Pixmap, PixmapMut};

use crate::{
  BorderProperties, Canvas, CanvasSubcanvas, CanvasViewport, DeferredOutline, Error, NodeMasks,
  Placement, Result, apply_backdrop_filter, apply_filters_to_pixmap, blend_pixel, draw_box_shell,
  draw_debug_border,
  inline_drawing::draw_own_content,
  layout::tree::{LayoutResults, RenderNode},
  placement_overlap,
  style::{Affine, BlendMode, Color, Filter, SizingContext},
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
      *dst_pixel = Color(out.0).premultiplied();
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

/// The state a painted node leaves open until its descendants are done: its
/// constraint mask, its isolation layer, and the bounds its filters cover.
struct PendingFinish {
  layout: Layout,
  /// How many masks the node pushed.
  constraints: usize,
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
    if let Some(deferred) = DeferredOutline::of(&node.context, self.layout) {
      match outlines {
        Some(outlines) => outlines.push(deferred),
        None => deferred.paint(canvas)?,
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

    for _ in 0..self.constraints {
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
    .map(|filter| (filter.reach(sizing) * transform_scale).ceil() as i32)
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
      outline.paint(self.canvas)?;
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
      let node_masks = if current.context.style.has_shape_mask() {
        let Some(masks) = NodeMasks::of(
          &current.context,
          layout,
          node_paint.transform,
          canvas.viewport(),
        )?
        else {
          return Ok(Some(DeferredNodeRender::SkipRendering));
        };

        masks.into_shell_mask()
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
        node_masks.as_ref(),
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

    let Some(masks) = NodeMasks::of(
      &current.context,
      layout,
      node_paint.transform,
      canvas.viewport(),
    )?
    else {
      if let Some(isolated_canvas) = isolated_canvas {
        canvas.composite_subcanvas(*isolated_canvas, BlendMode::Normal, 0.0);
      }
      return Ok(Some(DeferredNodeRender::SkipRendering));
    };
    let constraints = masks.len();

    for mask in masks.shell {
      canvas.push_mask(mask);
    }
    draw_render_node_shell(current, canvas, layout)?;
    if let Some(mask) = masks.content {
      canvas.push_mask(mask);
    }

    let finish = PendingFinish {
      layout,
      constraints,
      isolated_canvas,
      filter_bounds: node_paint.paint_bounds,
    };

    // An inline formatting context paints over the debug border, text and images under it.
    let inline = current.should_create_inline_layout();

    if !inline {
      draw_own_content(current, &current.context, canvas, layout)?;
    }
    if current.context.draw_debug_border() {
      draw_debug_border(canvas, layout, node_paint.transform);
    }

    if inline {
      draw_own_content(current, &current.context, canvas, layout)?;
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
  if !node.paints_own_box() {
    return Ok(());
  }

  draw_box_shell(&node.context, canvas, layout)
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
