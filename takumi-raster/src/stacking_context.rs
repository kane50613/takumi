use takumi_core::{
  geometry::{ComputedLayout as Layout, Placement, Size},
  paint_chunk::{ChunkPart, ConversionContext, PaintChunk, PropertySink},
  paint_property::{ClipId, ClipNode, EffectId, EffectNode},
  painter::{TableDecorations, TextClip, paint_collapsed_borders},
  scene::{NodePaint, Scene, SceneBounds, SceneView},
};
use tiny_skia::PixmapMut;

use crate::{
  BorderProperties, Canvas, CanvasDevice, CanvasSubcanvas, CanvasViewport, DeferredOutline, Error,
  NodeMasks, PlacedMask, RenderContext, Result, apply_backdrop_filter, apply_filters_to_pixmap,
  draw_debug_border, draw_table_box_shell,
  inline_drawing::draw_own_content,
  layout::tree::RenderNode,
  style::{Affine, BlendMode, Filter, SizingContext, expand_filter_bounds},
  within_pixmap_pixel_budget,
};

/// Paints `scene` onto `canvas` chunk by chunk, entering each chunk's clips and effects.
pub(crate) fn paint_scene(scene: &Scene, canvas: &mut Canvas) -> Result<()> {
  ScenePaint::new(scene.view()).paint(canvas)
}

/// A scene's paint chunks in paint order and the boxes owning its effects.
pub(crate) struct ScenePaint<'s> {
  scene: SceneView<'s>,
  chunks: Vec<PaintChunk<'s>>,
  owners: Vec<Option<&'s NodePaint>>,
}

impl<'s> ScenePaint<'s> {
  pub(crate) fn new(scene: SceneView<'s>) -> Self {
    let chunks = PaintChunk::in_paint_order(scene.contexts);
    let owners = PaintChunk::effect_owners(&chunks, scene.properties);

    Self {
      scene,
      chunks,
      owners,
    }
  }

  /// Paints the chunks reaching `canvas`'s viewport.
  pub(crate) fn paint(&self, canvas: &mut Canvas) -> Result<()> {
    let mut conversion = ConversionContext::new(
      self.scene.properties,
      ScenePainter {
        scene: self.scene,
        canvas,
        owners: &self.owners,
        effects: Vec::new(),
        error: None,
      },
    );

    for chunk in &self.chunks {
      conversion.switch_to(chunk.state());
      conversion.sink().paint(chunk);
    }

    conversion.finish().error.map_or(Ok(()), Err)
  }
}

/// An effect group open on the canvas.
struct OpenEffect {
  /// The group's layer, when it has one.
  layer: Option<Box<CanvasSubcanvas>>,
  /// How many `clip-path` and `mask-image` masks it pushed.
  masks: usize,
  /// Whether one of those masks hides everything.
  hidden: bool,
  /// The context its owner paints under, which its filters and compositing read.
  context: Option<RenderContext>,
  filter_bounds: Option<SceneBounds>,
}

/// Paints chunks and enters their clips and effects on one canvas.
struct ScenePainter<'s, 'c> {
  scene: SceneView<'s>,
  canvas: &'c mut Canvas,
  owners: &'c [Option<&'s NodePaint>],
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

    let result = placed(self.scene, chunk.node)
      .and_then(|(node, layout, context)| self.paint_part(chunk, node, &context, layout));

    self.record(result);
  }

  /// Draws `chunk`'s part of its box, laid out at `layout`.
  fn paint_part(
    &mut self,
    chunk: &PaintChunk<'_>,
    node: &RenderNode,
    context: &RenderContext,
    layout: Layout,
  ) -> Result<()> {
    let SceneView { root, results, .. } = self.scene;
    let canvas = &mut *self.canvas;

    match chunk.part {
      ChunkPart::Decorations if node.paints_own_box() => {
        let text_clip = TextClip::of(root, results, chunk.node, context)?;
        let table = TableDecorations::of(root, results, chunk.node)?;

        draw_table_box_shell(context, canvas, layout, text_clip.as_ref(), table)
      }
      ChunkPart::CollapsedBorders if node.paints_collapsed_borders() => {
        let mut result = Ok(());

        CanvasDevice::paint(canvas, context, |device| {
          result = paint_collapsed_borders(results, chunk.node, Affine::IDENTITY, None, device);
        })?;
        result
      }
      ChunkPart::Decorations | ChunkPart::CollapsedBorders => Ok(()),
      ChunkPart::Content => draw_node_content(node, context, canvas, layout, chunk.node.transform),
      ChunkPart::Outline => {
        DeferredOutline::of(context, layout).map_or(Ok(()), |outline| outline.paint(canvas))
      }
    }
  }

  fn record(&mut self, result: Result<()>) {
    if let Err(error) = result {
      self.error.get_or_insert(error);
    }
  }

  /// Opens the group of `effect` owned by `paint`: its filtered backdrop, its layer, then its
  /// `clip-path` and `mask-image` masks.
  fn open_effect(&mut self, paint: &NodePaint, effect: &EffectNode) -> Result<OpenEffect> {
    // The masks read the owner's paint offset, which differs between its fragments.
    let (_, layout, context) = placed(self.scene, paint)?;
    let canvas = &mut *self.canvas;
    let style = &context.style;

    if !style.misc1_data.backdrop_filter.is_empty() {
      let shell_mask = if style.has_shape_mask() {
        match NodeMasks::of(&context, layout, paint.transform, canvas.viewport())? {
          Some(masks) => masks.into_shell_mask(),
          None => return Ok(OpenEffect::hidden(None)),
        }
      } else {
        None
      };
      let border = BorderProperties::from_context(&context, layout.size, layout.border);

      apply_backdrop_filter(
        canvas,
        border,
        layout.size,
        paint.transform,
        &context,
        shell_mask.as_ref(),
      )?;
    }

    let viewport = canvas.viewport();
    let placement = effect
      .bounds
      .and_then(|bounds| layer_placement(viewport, bounds, &context, paint.transform))
      .unwrap_or_else(|| viewport.placement());
    let layer = Box::new(canvas.begin_subcanvas(placement)?);
    let Some(masks) = NodeMasks::of(&context, layout, paint.transform, canvas.viewport())? else {
      return Ok(OpenEffect::hidden(Some(layer)));
    };
    let count = masks.len();

    for mask in masks.shell {
      canvas.push_mask(PlacedMask::covering(canvas.viewport(), mask));
    }

    Ok(OpenEffect {
      layer: Some(layer),
      masks: count,
      hidden: false,
      context: Some(context),
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
    let Some(context) = effect.context.as_ref().filter(|_| !effect.hidden) else {
      canvas.composite_subcanvas(*layer, BlendMode::Normal, 0.0);
      return Ok(());
    };

    if !context.style.misc1_data.filter.is_empty() {
      apply_filters(canvas, context, effect.filter_bounds)?;
    }

    canvas.composite_subcanvas(
      *layer,
      context.style.misc_data.mix_blend_mode,
      context.style.svg_data.opacity.0,
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
      context: None,
      filter_bounds: None,
    }
  }
}

impl PropertySink for ScenePainter<'_, '_> {
  fn push_clip(&mut self, _id: ClipId, clip: &ClipNode) {
    let viewport = self.canvas.viewport();

    match PlacedMask::of_clip(clip, viewport) {
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

/// The box `paint` names in `scene`, its layout, and its context set to paint where the scene
/// placed it. The tree stays as it was, as pages still to lay out read it.
fn placed<'r>(
  scene: SceneView<'r>,
  paint: &NodePaint,
) -> Result<(&'r RenderNode, Layout, RenderContext)> {
  let Some(node) = scene.root.node_at_path(&paint.path) else {
    return Err(Error::InvalidLayoutNode(paint.node_id.into()));
  };
  let layout = scene.enter(paint)?;
  let mut context = node.context.clone();

  context
    .sizing
    .set_container_size(paint.container_size.width, paint.container_size.height);
  context.transform = paint.transform;

  Ok((node, layout, context))
}

/// Runs `context`'s filters over the pixels its group painted within `bounds`.
fn apply_filters(
  canvas: &mut Canvas,
  context: &RenderContext,
  bounds: Option<SceneBounds>,
) -> Result<()> {
  let viewport = canvas.viewport();
  let filter_padding = filter_padding(
    &context.style.misc1_data.filter,
    &context.sizing,
    context.transform,
  );
  let filter_region = bounds.and_then(|bounds| {
    viewport
      .clamp_bounds(bounds, filter_padding)
      .map(|region| region.translate(-(viewport.origin.x), -(viewport.origin.y)))
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
      &context.sizing,
      context.current_color,
      context.style.misc1_data.filter.iter(),
    )?;
    canvas.write_region(region, &region_raw);

    return Ok(());
  }

  canvas.with_pixmap(|pixmap| {
    apply_filters_to_pixmap(
      &mut pixmap.as_mut(),
      &context.sizing,
      context.current_color,
      context.style.misc1_data.filter.iter(),
    )
  })
}

/// cc's `RenderSurfaceImpl::CalculateClippedAccumulatedContentRect`: the group's `bounds`, with a
/// 2px margin, within the viewport grown by what its filters read and write past it
/// (`CalculateExpandedClipForFilters`), which only a filter that moves pixels grows. Approximate:
/// cc then clamps the surface to the max texture size, where a layer past takumi's pixmap budget
/// keeps to the viewport.
/// <https://source.chromium.org/chromium/chromium/src/+/main:cc/layers/render_surface_impl.cc>
fn layer_placement(
  viewport: CanvasViewport,
  bounds: SceneBounds,
  context: &RenderContext,
  transform: Affine,
) -> Option<Placement> {
  let expanded = expand_filter_bounds(
    &context.style.misc1_data.filter,
    viewport.bounds(),
    &context.sizing,
    transform,
  );

  CanvasViewport::covering(expanded)
    .and_then(|clip| clip.clamp_bounds(bounds, 2))
    .filter(|placement| {
      within_pixmap_pixel_budget(Size {
        width: placement.width,
        height: placement.height,
      })
    })
    .or_else(|| viewport.clamp_bounds(bounds, 2))
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
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  transform: Affine,
) -> Result<()> {
  let inline = node.should_create_inline_layout();

  if !inline {
    draw_own_content(node, context, canvas, layout)?;
  }
  if context.draw_debug_border() {
    draw_debug_border(canvas, layout, transform);
  }
  if inline {
    draw_own_content(node, context, canvas, layout)?;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::error::Error;

  use crate::{Bitmap, Fonts, RenderOptions, layout::node::Node, render, viewport::Viewport};

  type TestResult = Result<(), Box<dyn Error>>;

  fn render_json(json: &str) -> Result<Bitmap, Box<dyn Error>> {
    let fonts = Fonts::default();
    let node: Node = serde_json::from_str(json)?;
    let options = RenderOptions::builder()
      .viewport(Viewport::new((100, 100)))
      .node(node)
      .fonts(&fonts)
      .build();
    Ok(render(options)?)
  }

  #[test]
  fn a_filter_reaching_past_the_pixmap_budget_keeps_to_the_canvas() -> TestResult {
    let image = render_json(
      r##"{
        "type": "container",
        "style": {"display": "flex", "position": "relative", "width": "100%", "height": "100%"},
        "children": [{
          "type": "container",
          "style": {
            "position": "absolute", "left": "-50px", "top": "0px", "width": "100px",
            "height": "100px", "backgroundColor": "#ffffff", "filter": "blur(100000px)"
          },
          "children": []
        }]
      }"##,
    )?;

    assert_eq!(image.dimensions(), (100, 100));
    Ok(())
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

    assert_eq!(with_zero.as_raw(), without_zero.as_raw());
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
      pixel[0] > 0 && pixel[3] > 0,
      "overflowing child of zero-sized opacity parent must still paint, got {pixel:?}"
    );
    Ok(())
  }
}
