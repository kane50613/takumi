use takumi_core::{
  geometry::ComputedLayout as Layout,
  paint_chunk::{ChunkPart, ConversionContext, PaintChunk, PropertySink},
  paint_property::{ClipId, ClipNode, EffectId, EffectNode},
  scene::{NodePaint, Scene, SceneBounds},
};
use tiny_skia::PixmapMut;

use crate::{
  BorderProperties, Canvas, CanvasSubcanvas, CanvasViewport, DeferredOutline, Error, NodeMasks,
  Result, apply_backdrop_filter, apply_filters_to_pixmap, clip_node_mask, draw_box_shell,
  draw_debug_border,
  inline_drawing::draw_own_content,
  layout::tree::{LayoutResults, RenderNode},
  style::{Affine, BlendMode, Filter, SizingContext},
};

/// Paints `scene` onto `canvas` chunk by chunk, entering each chunk's clips and effects.
pub(crate) fn paint_scene(scene: &mut Scene, canvas: &mut Canvas) -> Result<()> {
  let Scene {
    root,
    results,
    contexts,
    properties,
    ..
  } = scene;
  let chunks = PaintChunk::in_paint_order(contexts);
  let owners = PaintChunk::effect_owners(&chunks, properties);
  let mut conversion = ConversionContext::new(
    properties,
    ScenePainter {
      root,
      results,
      canvas,
      owners,
      effects: Vec::new(),
      error: None,
    },
  );

  for chunk in &chunks {
    conversion.switch_to(chunk.state());
    conversion.sink().paint(chunk);
  }

  conversion.finish().error.map_or(Ok(()), Err)
}

/// An effect group open on the canvas.
struct OpenEffect {
  /// The group's layer, when it has one.
  layer: Option<Box<CanvasSubcanvas>>,
  /// How many `clip-path` and `mask-image` masks it pushed.
  masks: usize,
  /// Whether one of those masks hides everything.
  hidden: bool,
  owner: Option<Vec<usize>>,
  filter_bounds: Option<SceneBounds>,
}

/// Paints chunks and enters their clips and effects on one canvas.
struct ScenePainter<'s, 'c> {
  root: &'s mut RenderNode,
  results: &'s LayoutResults,
  canvas: &'c mut Canvas,
  owners: Vec<Option<&'s NodePaint>>,
  effects: Vec<OpenEffect>,
  error: Option<Error>,
}

impl ScenePainter<'_, '_> {
  /// Draws one chunk under the clips and effects already entered.
  fn paint(&mut self, chunk: &PaintChunk<'_>) {
    if self.error.is_some() || self.effects.iter().any(|effect| effect.hidden) {
      return;
    }
    if let Some(bounds) = chunk.node.paint_bounds
      && !self.canvas.viewport().intersects(bounds)
    {
      return;
    }

    let canvas = &mut *self.canvas;
    let result =
      placed(self.root, self.results, chunk.node).and_then(|(node, layout)| match chunk.part {
        ChunkPart::Decorations if node.paints_own_box() => {
          draw_box_shell(&node.context, canvas, layout)
        }
        ChunkPart::Decorations => Ok(()),
        ChunkPart::Content => draw_node_content(node, canvas, layout, chunk.node.transform),
        ChunkPart::Outline => {
          DeferredOutline::of(&node.context, layout).map_or(Ok(()), |outline| outline.paint(canvas))
        }
      });

    self.record(result);
  }

  fn record(&mut self, result: Result<()>) {
    if let Err(error) = result {
      self.error.get_or_insert(error);
    }
  }

  /// Opens the group of `effect` owned by `paint`: its filtered backdrop, its layer, then its
  /// `clip-path` and `mask-image` masks.
  fn open_effect(&mut self, paint: &NodePaint, effect: &EffectNode) -> Result<OpenEffect> {
    let (node, layout) = placed(self.root, self.results, paint)?;
    let canvas = &mut *self.canvas;
    let style = &node.context.style;

    if !style.backdrop_filter.is_empty() {
      let shell_mask = if style.has_shape_mask() {
        match NodeMasks::of(&node.context, layout, paint.transform, canvas.viewport())? {
          Some(masks) => masks.into_shell_mask(),
          None => return Ok(OpenEffect::hidden(None)),
        }
      } else {
        None
      };
      let border = BorderProperties::from_context(&node.context, layout.size, layout.border);

      apply_backdrop_filter(
        canvas,
        border,
        layout.size,
        paint.transform,
        &node.context,
        shell_mask.as_ref(),
      )?;
    }

    let viewport = canvas.viewport();
    let placement = effect
      .bounds
      .and_then(|bounds| viewport.clamp_bounds(bounds, 2))
      .unwrap_or_else(|| viewport.placement());
    let layer = Box::new(canvas.begin_subcanvas(placement)?);
    let Some(masks) = NodeMasks::of(&node.context, layout, paint.transform, canvas.viewport())?
    else {
      return Ok(OpenEffect::hidden(Some(layer)));
    };
    let count = masks.len();

    for mask in masks.shell {
      canvas.push_mask(mask);
    }

    Ok(OpenEffect {
      layer: Some(layer),
      masks: count,
      hidden: false,
      owner: Some(paint.path.clone()),
      filter_bounds: effect.bounds,
    })
  }

  /// Filters, unmasks and composites the most recent group.
  fn close_effect(&mut self, effect: OpenEffect) -> Result<()> {
    let canvas = &mut *self.canvas;

    for _ in 0..effect.masks {
      canvas.pop_mask();
    }

    let Some(layer) = effect.layer else {
      return Ok(());
    };
    let node = effect
      .owner
      .as_deref()
      .and_then(|path| self.root.node_at_path(path));
    let Some(node) = node.filter(|_| !effect.hidden) else {
      canvas.composite_subcanvas(*layer, BlendMode::Normal, 0.0);
      return Ok(());
    };

    if !node.context.style.filter.is_empty() {
      apply_filters(canvas, node, effect.filter_bounds)?;
    }

    canvas.composite_subcanvas(
      *layer,
      node.context.style.mix_blend_mode,
      node.context.style.opacity.0,
    );

    Ok(())
  }
}

impl OpenEffect {
  /// A group that shows nothing it paints, over `layer` when it opened one.
  fn hidden(layer: Option<Box<CanvasSubcanvas>>) -> Self {
    Self {
      layer,
      masks: 0,
      hidden: true,
      owner: None,
      filter_bounds: None,
    }
  }
}

impl PropertySink for ScenePainter<'_, '_> {
  fn push_clip(&mut self, _id: ClipId, clip: &ClipNode) {
    let viewport = self.canvas.viewport();

    match clip_node_mask(clip, viewport) {
      Some(mask) => self.canvas.push_mask(mask),
      None => self.record(Err(Error::InvalidViewport)),
    }
  }

  fn pop_clip(&mut self) {
    self.canvas.pop_mask();
  }

  fn begin_effect(&mut self, id: EffectId, effect: &EffectNode) {
    let opened = match self.owners[id.index()] {
      Some(owner) => self.open_effect(owner, effect),
      None => Err(Error::InvalidLayoutNode(0)),
    };

    match opened {
      Ok(open) => self.effects.push(open),
      Err(error) => {
        self.effects.push(OpenEffect::hidden(None));
        self.record(Err(error));
      }
    }
  }

  fn end_effect(&mut self) {
    if let Some(effect) = self.effects.pop() {
      let result = self.close_effect(effect);

      self.record(result);
    }
  }
}

/// The box `paint` names in `root` and its layout, set up to paint where the scene placed it.
fn placed<'r>(
  root: &'r mut RenderNode,
  results: &LayoutResults,
  paint: &NodePaint,
) -> Result<(&'r mut RenderNode, Layout)> {
  let Some(node) = root.node_at_path_mut(&paint.path) else {
    return Err(Error::InvalidLayoutNode(paint.node_id.into()));
  };
  let layout = results.layout(paint.node_id)?;

  node
    .context
    .sizing
    .set_container_size(paint.container_size.width, paint.container_size.height);
  node.context.transform = paint.transform;

  Ok((node, layout))
}

/// Runs `node`'s filters over the pixels its group painted within `bounds`.
fn apply_filters(
  canvas: &mut Canvas,
  node: &RenderNode,
  bounds: Option<SceneBounds>,
) -> Result<()> {
  let viewport = canvas.viewport();
  let filter_padding = filter_padding(
    &node.context.style.filter,
    &node.context.sizing,
    node.context.transform,
  );
  let filter_region = bounds.and_then(|bounds| {
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

    return Ok(());
  }

  canvas.with_pixmap(|pixmap| {
    apply_filters_to_pixmap(
      &mut pixmap.as_mut(),
      &node.context.sizing,
      node.context.current_color,
      node.context.style.filter.iter(),
    )
  })
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

/// Paints a node's own content and debug border, an inline formatting context over the border.
fn draw_node_content(
  node: &RenderNode,
  canvas: &mut Canvas,
  layout: Layout,
  transform: Affine,
) -> Result<()> {
  let inline = node.should_create_inline_layout();

  if !inline {
    draw_own_content(node, &node.context, canvas, layout)?;
  }
  if node.context.draw_debug_border() {
    draw_debug_border(canvas, layout, transform);
  }
  if inline {
    draw_own_content(node, &node.context, canvas, layout)?;
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
