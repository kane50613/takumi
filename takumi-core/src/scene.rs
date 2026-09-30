//! Backend-agnostic paint scene: the stacking-context tree that decides paint order, grouping, and
//! bounds. Raster and SVG backends consume this instead of each walking the node tree
//! independently.

use std::convert::Infallible;

use skrifa::FontRef;

use crate::{
  error::{Error, Result},
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, ComputedLayout, NodeId, Point, Size, transformed_rect_extents},
  layout::{
    decoration::OutlineGeometry,
    inline::{
      InlineContentKind, InlineLayoutMode, InlineLayoutRequest, PlacedItem, ProcessedInlineSpan,
      collect_inline_items, create_inline_layout, glyph_run_rect, resolve_inline_max_height,
    },
    node::Node,
    tree::{ContainingBlocks, LayoutResults, RenderNode},
  },
  shadow::SizedShadow,
  style::{Affine, BlurType, ComputedStyle, Display, Float},
  viewport::Viewport,
};

/// A node's resolved paint inputs.
#[derive(Clone)]
pub struct NodePaint {
  /// Child-index path from the root to this node.
  pub path: Vec<usize>,
  /// Layout id of the node.
  pub node_id: NodeId,
  /// Accumulated transform applied when painting.
  pub transform: Affine,
  /// Containing-block size; `None` on an axis is indefinite.
  pub container_size: Size<Option<f32>>,
  /// Device-space bounds of the paint output, if any.
  pub paint_bounds: Option<SceneBounds>,
}

/// Device-space integer bounds of a node or stacking context's paint output.
#[derive(Clone, Copy)]
pub struct SceneBounds {
  /// Left edge, inclusive.
  pub left: usize,
  /// Top edge, inclusive.
  pub top: usize,
  /// Right edge, exclusive.
  pub right: usize,
  /// Bottom edge, exclusive.
  pub bottom: usize,
}

impl SceneBounds {
  /// Whether the bounds enclose zero area.
  pub fn is_empty(self) -> bool {
    self.left >= self.right || self.top >= self.bottom
  }
}

/// What a [`PaintItem`] paints.
#[derive(Clone)]
pub enum PaintItemKind {
  /// A single node's paint inputs.
  Node(NodePaint),
  /// Index of a nested stacking context.
  Context(usize),
}

/// A paint entry plus its z-index and source order, which together order it uniquely.
#[derive(Clone)]
pub struct PaintItem {
  /// The node or nested stacking context to paint.
  pub kind: PaintItemKind,
  z_index: i32,
  source_order: usize,
}

impl PaintItem {
  fn z_order(&self) -> (i32, usize) {
    (self.z_index, self.source_order)
  }
}

#[derive(Clone, Copy)]
/// The phases of [CSS 2.1 Appendix E](https://www.w3.org/TR/CSS21/zindex.html) a stacking context
/// paints its descendants in, after its own background.
enum PaintBucket {
  /// Negative `z-index`.
  Negative,
  /// In-flow, non-positioned boxes, each with its inline content.
  ///
  /// Approximate: a box paints its text right after its background, where Blink paints every
  /// block's background before any of their text.
  InFlow,
  /// Non-positioned floats.
  ///
  /// Approximate: a float inside inline content paints with that content, where Blink paints it
  /// in this phase.
  Float,
  /// Positioned boxes and stacking contexts at `z-index: auto` or `0`, in tree order.
  Positioned,
  /// Positive `z-index`.
  Positive,
}

#[derive(Default)]
struct StackingBuckets {
  negative: Vec<PaintItem>,
  in_flow: Vec<PaintItem>,
  floats: Vec<PaintItem>,
  positioned: Vec<PaintItem>,
  positive: Vec<PaintItem>,
}

impl StackingBuckets {
  fn push(&mut self, bucket: PaintBucket, item: PaintItem) {
    match bucket {
      PaintBucket::Negative => self.negative.push(item),
      PaintBucket::InFlow => self.in_flow.push(item),
      PaintBucket::Float => self.floats.push(item),
      PaintBucket::Positioned => self.positioned.push(item),
      PaintBucket::Positive => self.positive.push(item),
    }
  }

  /// Orders the z-indexed buckets; the others are pushed in tree order already.
  fn sort(&mut self) {
    self.negative.sort_unstable_by_key(PaintItem::z_order);
    self.positive.sort_unstable_by_key(PaintItem::z_order);
  }

  fn in_paint_order(&self) -> [&[PaintItem]; 5] {
    [
      &self.negative,
      &self.in_flow,
      &self.floats,
      &self.positioned,
      &self.positive,
    ]
  }
}

/// One stacking context: an optional root node and its descendants bucketed into CSS paint order.
pub struct StackingContextNode {
  root: Option<NodePaint>,
  buckets: StackingBuckets,
  paint_bounds: Option<SceneBounds>,
}

impl StackingContextNode {
  /// The node that owns this context, if any (the synthetic root has none).
  pub fn root(&self) -> Option<&NodePaint> {
    self.root.as_ref()
  }

  /// The context's device-space paint bounds, once computed.
  pub fn paint_bounds(&self) -> Option<SceneBounds> {
    self.paint_bounds
  }

  /// Paint items grouped by stacking layer in paint order.
  pub fn in_paint_order(&self) -> [&[PaintItem]; 5] {
    self.buckets.in_paint_order()
  }

  fn with_root(root: Option<NodePaint>) -> Self {
    Self {
      root,
      buckets: StackingBuckets::default(),
      paint_bounds: None,
    }
  }

  fn push_item(
    &mut self,
    bucket: PaintBucket,
    kind: PaintItemKind,
    z_index: i32,
    source_order: usize,
  ) {
    self.buckets.push(
      bucket,
      PaintItem {
        kind,
        z_index,
        source_order,
      },
    );
  }
}

struct StackingContextBuildVisit {
  path: Vec<usize>,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
  /// The context in-flow boxes and floats paint in.
  context_id: usize,
  /// The context positioned boxes and stacking contexts paint in: the nearest stacking context,
  /// or the nearest box that clips its overflow, which keeps what it clips.
  stacking_id: usize,
  parent_display: Option<Display>,
  is_root: bool,
}

impl PaintBucket {
  /// The phase a child with `style` paints in, and its z-index there. A stacking context at
  /// `z-index: auto` paints with the positioned boxes.
  fn of(
    style: &ComputedStyle,
    is_flex_or_grid_item: bool,
    creates_stacking_context: bool,
  ) -> (Self, i32) {
    let z = style.paint_order_z(is_flex_or_grid_item);

    if z < 0 {
      (Self::Negative, z)
    } else if z > 0 {
      (Self::Positive, z)
    } else if creates_stacking_context
      || style.participates_in_positioned_paint_bucket(is_flex_or_grid_item)
    {
      (Self::Positioned, 0)
    } else if style.float != Float::None && !is_flex_or_grid_item {
      (Self::Float, 0)
    } else {
      (Self::InFlow, 0)
    }
  }

  /// Whether the phase belongs to the nearest stacking context rather than the nearest box that
  /// paints its descendants atomically.
  fn lifts(&self) -> bool {
    matches!(self, Self::Negative | Self::Positioned | Self::Positive)
  }
}

/// What a scene is built from.
#[derive(Clone, Copy)]
pub struct SceneRequest<'a> {
  /// The tree to paint.
  pub root: &'a RenderNode,
  /// Its layout.
  pub layout_results: &'a LayoutResults,
  /// The transform the root paints under.
  pub transform: Affine,
  /// The size percentages of the root resolve against.
  pub container_size: Size<Option<f32>>,
  /// Whether to compute each node's paint bounds, which the raster and SVG backends clip and cull by.
  pub paint_bounds: bool,
}

impl SceneRequest<'_> {
  /// Flattens the node tree into CSS-ordered stacking contexts for painting.
  pub fn build(self) -> Result<Vec<StackingContextNode>> {
    let SceneRequest {
      root,
      layout_results,
      transform,
      container_size,
      paint_bounds: with_bounds,
    } = self;
    let mut contexts = vec![StackingContextNode::with_root(None)];
    let mut source_order = 0usize;
    let mut containing_blocks = ContainingBlocks::default();
    let mut visits = vec![StackingContextBuildVisit {
      path: Vec::new(),
      node_id: NodeId::ROOT,
      transform,
      container_size,
      context_id: 0,
      stacking_id: 0,
      parent_display: None,
      is_root: true,
    }];

    while let Some(visit) = visits.pop() {
      let Some(current) = root.node_at_path(&visit.path) else {
        return Err(Error::InvalidLayoutNode(visit.node_id.into()));
      };
      let layout = layout_results.layout(visit.node_id)?;
      if current.context.style.is_invisible() {
        continue;
      }

      let mut current_transform = visit.transform;
      current_transform *= Affine::translation(layout.location.x, layout.location.y);
      current_transform *= current.context.style.local_transform(
        layout.size.width,
        layout.size.height,
        &current.context.sizing,
      );
      if !current_transform.is_invertible() {
        continue;
      }
      containing_blocks.record_transform(visit.node_id, current_transform);

      let node_paint = NodePaint {
        path: visit.path.clone(),
        node_id: visit.node_id,
        transform: current_transform,
        container_size: visit.container_size,
        paint_bounds: with_bounds
          .then(|| compute_node_paint_bounds(current, layout, current_transform))
          .flatten(),
      };

      let is_flex_or_grid_item = visit.parent_display.is_some_and(|display| {
        matches!(
          display,
          Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid
        )
      });

      let creates_stacking_context = visit.is_root
        || current.context.style.creates_stacking_context(
          layout.size.width,
          layout.size.height,
          &current.context.sizing,
          is_flex_or_grid_item,
        );
      let clips = current
        .context
        .style
        .resolve_overflows()
        .should_clip_content();

      let mut context_id = visit.context_id;
      let mut stacking_id = visit.stacking_id;

      if visit.is_root {
        contexts[0].root = Some(node_paint);
      } else {
        let (bucket, z_index) = PaintBucket::of(
          &current.context.style,
          is_flex_or_grid_item,
          creates_stacking_context,
        );
        let parent = if bucket.lifts() {
          visit.stacking_id
        } else {
          visit.context_id
        };
        // A positioned box or a float paints its descendants atomically, as if it were a
        // stacking context, but its positioned and z-indexed descendants still paint in the
        // real one.
        let atomic = !matches!(bucket, PaintBucket::InFlow);

        if creates_stacking_context || clips || atomic {
          let child_context = contexts.len();

          contexts.push(StackingContextNode::with_root(Some(node_paint)));
          contexts[parent].push_item(
            bucket,
            PaintItemKind::Context(child_context),
            z_index,
            source_order,
          );
          context_id = child_context;
          if creates_stacking_context || clips {
            stacking_id = child_context;
          }
        } else {
          contexts[parent].push_item(
            bucket,
            PaintItemKind::Node(node_paint),
            z_index,
            source_order,
          );
        }
        source_order += 1;
      }

      if current.children.is_none() {
        continue;
      }

      if current.should_create_inline_layout() {
        continue;
      }

      let layout_children = layout_results.box_children(visit.node_id)?;
      let child_container_size = Size {
        width: Some(layout.content_box_width()),
        height: Some(layout.content_box_height()),
      };
      containing_blocks.record_content_box(visit.node_id, child_container_size);

      for child in layout_children.iter().rev() {
        let mut child_path = visit.path.clone();
        child_path.push(child.render_index);
        let (base_transform, base_container) =
          containing_blocks.base_for(child, current_transform, child_container_size);
        visits.push(StackingContextBuildVisit {
          path: child_path,
          node_id: child.node_id,
          transform: base_transform,
          container_size: base_container,
          context_id,
          stacking_id,
          parent_display: Some(current.context.style.display),
          is_root: false,
        });
      }
    }

    for context in &mut contexts {
      context.buckets.sort();
    }

    if !with_bounds {
      return Ok(contexts);
    }

    // `None` means "unknown extent" and poisons the union; dropping it would
    // under-report the context and clip or cull visible paint.
    for context_id in (0..contexts.len()).rev() {
      let mut paint_bounds = None;
      let mut unknown = false;
      if let Some(root) = &contexts[context_id].root {
        match root.paint_bounds {
          Some(bounds) => paint_bounds = Some(bounds),
          None => unknown = true,
        }
      }
      for bucket in contexts[context_id].buckets.in_paint_order() {
        for item in bucket {
          let item_bounds = match &item.kind {
            PaintItemKind::Node(node_paint) => node_paint.paint_bounds,
            PaintItemKind::Context(child_context_id) => contexts[*child_context_id].paint_bounds,
          };
          match item_bounds {
            Some(bounds) => paint_bounds = merge_bounds(paint_bounds, Some(bounds)),
            None => unknown = true,
          }
        }
      }
      if let Some(root_paint) = &contexts[context_id].root
        && let Some(root_node) = root.node_at_path(&root_paint.path)
      {
        paint_bounds = outset_bounds(paint_bounds, filter_reach(root_node), root_paint.transform);
      }
      contexts[context_id].paint_bounds = if unknown { None } else { paint_bounds };
    }

    Ok(contexts)
  }
}

/// A render tree laid out, with the stacking contexts that paint it.
pub struct Scene {
  /// The tree.
  pub root: RenderNode,
  /// Its layout.
  pub results: LayoutResults,
  /// Its stacking contexts; the first is the synthetic root.
  pub contexts: Vec<StackingContextNode>,
  /// The size it paints at: the viewport on a definite axis, the root's border box otherwise.
  pub size: Size<f32>,
}

impl Scene {
  /// Lays `root` out in `viewport` and builds its scene at the origin.
  pub fn lay_out(root: RenderNode, viewport: Viewport, paint_bounds: bool) -> Result<Self> {
    let results = LayoutResults::compute(&root, viewport.into());
    let container_size = Size::from(viewport.size);
    let size = container_size.zip_map(results.layout(NodeId::ROOT)?.size, Option::unwrap_or);
    let contexts = SceneRequest {
      root: &root,
      layout_results: &results,
      transform: Affine::IDENTITY,
      container_size,
      paint_bounds,
    }
    .build()?;

    Ok(Self {
      root,
      results,
      contexts,
      size,
    })
  }
}

/// How far a shadow's ink reaches past the shape that casts it.
fn shadow_reach(shadow: &SizedShadow) -> f32 {
  shadow.offset_x.abs().max(shadow.offset_y.abs())
    + shadow.spread_radius.max(0.0)
    + BlurType::Shadow.extent(shadow.blur_radius)
}

/// How far the node's filters spread its layer, in local px.
fn filter_reach(node: &RenderNode) -> f32 {
  let sizing = &node.context.sizing;

  node
    .context
    .style
    .filter
    .iter()
    .map(|filter| filter.reach(sizing))
    .sum()
}

/// How far box shadows and the outline reach past the border box, in local px.
fn box_ink_reach(node: &RenderNode, size: Size<f32>) -> f32 {
  let context = &node.context;
  let shadows = context.style.box_shadow.iter().flatten();
  let shadow_reach = shadows
    .filter(|shadow| !shadow.inset)
    .map(|shadow| {
      shadow_reach(&SizedShadow::from_box_shadow(
        *shadow,
        &context.sizing,
        context.current_color,
        size,
      ))
    })
    .fold(0.0_f32, f32::max);
  let outline_reach =
    OutlineGeometry::painted(context, size).map_or(0.0, |outline| outline.grow.max(0.0));

  shadow_reach.max(outline_reach)
}

/// How far text shadows and the text stroke reach past glyph ink, in local px.
fn text_ink_reach(font_style: &SizedFontStyle) -> f32 {
  let shadow_reach = font_style
    .text_shadow
    .iter()
    .map(shadow_reach)
    .fold(0.0_f32, f32::max);

  shadow_reach.max(font_style.stroke_width)
}

/// Grows `bounds` by `reach` local px on every side, taking the transform's per-axis envelope.
fn outset_bounds(
  bounds: Option<SceneBounds>,
  reach: f32,
  transform: Affine,
) -> Option<SceneBounds> {
  let mut bounds = bounds?;
  if reach <= 0.0 {
    return Some(bounds);
  }

  let pad_x = (reach * (transform.a.abs() + transform.c.abs())).ceil() as usize;
  let pad_y = (reach * (transform.b.abs() + transform.d.abs())).ceil() as usize;
  bounds.left = bounds.left.saturating_sub(pad_x);
  bounds.top = bounds.top.saturating_sub(pad_y);
  bounds.right = bounds.right.saturating_add(pad_x);
  bounds.bottom = bounds.bottom.saturating_add(pad_y);
  Some(bounds)
}

fn compute_node_paint_bounds(
  node: &RenderNode,
  layout: ComputedLayout,
  transform: Affine,
) -> Option<SceneBounds> {
  let mut bounds = outset_bounds(
    bounds_for_rect(layout.size, transform),
    box_ink_reach(node, layout.size),
    transform,
  );
  if !has_inline_paint_content(node) {
    return bounds;
  }

  let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);
  if font_style.sizing.font_size == 0.0 {
    return bounds;
  }

  let content = layout.unsnapped_content;
  let available_space = Size {
    width: AvailableSpace::Definite(content.width),
    height: AvailableSpace::Definite(content.height),
  };
  let max_height = resolve_inline_max_height(&font_style, content.height);

  let built = create_inline_layout(InlineLayoutRequest {
    items: collect_inline_items(node),
    available_space,
    max_width: content.width,
    max_height,
    style: &font_style,
    context: &node.context,
    mode: InlineLayoutMode::Measure,
    shape_cacheable: true,
  });
  let content_offset = layout.content_box_offset();
  let inline_transform = Affine::translation(content_offset.x, content_offset.y) * transform;
  let Ok(()) = built.walk_items::<Infallible>(layout, |line, item| {
    let setup = &line.setup;

    match item {
      PlacedItem::Run {
        glyph_run,
        static_inline_prefix,
        ..
      } => {
        let (glyph_origin, glyph_size) = glyph_run_rect(&glyph_run, setup.baseline_shift);
        let (glyph_origin, glyph_size) =
          setup.scale_rect(glyph_origin, glyph_size, static_inline_prefix);

        bounds = merge_bounds(
          bounds,
          bounds_for_placed_rect(glyph_origin, glyph_size, inline_transform),
        );

        // The metrics box above misses ink outside advance × (ascent+descent):
        // synthetic-italic skew, faux-bold outset, negative bearings, and
        // glyphs taller than the font's metrics. Merge per-glyph ink extents
        // so isolation surfaces sized from these bounds never clip text.
        let Ok(font) = FontRef::from_index(
          glyph_run.run().font().data.as_ref(),
          glyph_run.run().font().index,
        ) else {
          return Ok(());
        };
        let glyph_ids = glyph_run.positioned_glyphs().map(|glyph| glyph.id);
        let resolved_glyphs = node
          .context
          .fonts()
          .with_context(|fonts| fonts.resolve_glyphs(&glyph_run, font, glyph_ids));

        for glyph in glyph_run.positioned_glyphs() {
          let Some((min_x, min_y, max_x, max_y)) = resolved_glyphs
            .get(&glyph.id)
            .and_then(|glyph| glyph.ink_extents())
          else {
            continue;
          };
          let (ink_origin, ink_size) = setup.scale_rect(
            Point {
              x: glyph.x + min_x,
              y: glyph.y + setup.baseline_shift + min_y,
            },
            Size {
              width: max_x - min_x,
              height: max_y - min_y,
            },
            static_inline_prefix,
          );

          bounds = merge_bounds(
            bounds,
            bounds_for_placed_rect(ink_origin, ink_size, inline_transform),
          );
        }
      }
      PlacedItem::Box(inline_box) => {
        bounds = merge_bounds(
          bounds,
          bounds_for_placed_rect(
            Point::new(inline_box.x, inline_box.y),
            Size::new(inline_box.width, inline_box.height),
            inline_transform,
          ),
        );
      }
    }
    Ok(())
  });

  for inline_box in built.positioned_floats {
    bounds = merge_bounds(
      bounds,
      bounds_for_placed_rect(
        Point::new(inline_box.x, inline_box.y),
        Size::new(inline_box.width, inline_box.height),
        inline_transform,
      ),
    );
  }

  let decoration_reach = built
    .spans
    .iter()
    .filter_map(|span| match span {
      ProcessedInlineSpan::Text { decorations, .. } => decorations.as_ref(),
      _ => None,
    })
    .fold(0.0_f32, |mut max, chain| {
      let mut next = Some(chain);

      while let Some(link) = next {
        max = max.max(link.decoration.reach());
        next = link.parent.as_ref();
      }
      max
    });

  let text_reach = built
    .spans
    .iter()
    .filter_map(|span| match span {
      ProcessedInlineSpan::Text { style, .. } => Some(text_ink_reach(style)),
      _ => None,
    })
    .fold(text_ink_reach(&font_style), f32::max);

  outset_bounds(bounds, decoration_reach.max(text_reach), inline_transform)
}

fn has_inline_paint_content(node: &RenderNode) -> bool {
  node.should_create_inline_layout()
    || node.anonymous_text_content.is_some()
    || matches!(
      node.node.as_ref().and_then(Node::inline_content),
      Some(InlineContentKind::Text(_))
    )
    || node.children.as_ref().is_some_and(|children| {
      children
        .iter()
        .any(|child| child.anonymous_text_content.is_some())
    })
}

fn bounds_for_rect(size: Size<f32>, transform: Affine) -> Option<SceneBounds> {
  let (min_x, min_y, max_x, max_y) = transformed_rect_extents(Point::ZERO, size, transform)?;
  let left = (min_x.floor() as i32).max(0) as usize;
  let top = (min_y.floor() as i32).max(0) as usize;
  let right = (max_x.ceil() as i32).max(0) as usize;
  let bottom = (max_y.ceil() as i32).max(0) as usize;

  // Empty bounds mean "paints nothing"; None means "unknown" and forces full-viewport isolation.
  Some(SceneBounds {
    left,
    top,
    right,
    bottom,
  })
}

/// [`bounds_for_rect`] for a rect at `origin` in `transform`'s space.
fn bounds_for_placed_rect(
  origin: Point<f32>,
  size: Size<f32>,
  transform: Affine,
) -> Option<SceneBounds> {
  bounds_for_rect(size, Affine::translation(origin.x, origin.y) * transform)
}

fn merge_bounds(left: Option<SceneBounds>, right: Option<SceneBounds>) -> Option<SceneBounds> {
  match (left, right) {
    // Empty bounds paint nothing and sit at clamped positions; don't let them expand the union.
    (Some(left), Some(right)) if left.is_empty() => Some(right),
    (Some(left), Some(right)) if right.is_empty() => Some(left),
    (Some(left), Some(right)) => Some(SceneBounds {
      left: left.left.min(right.left),
      top: left.top.min(right.top),
      right: left.right.max(right.right),
      bottom: left.bottom.max(right.bottom),
    }),
    (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
    (None, None) => None,
  }
}

#[cfg(test)]
mod tests {
  use super::{SceneBounds, bounds_for_rect, merge_bounds};
  use crate::{geometry::Size, style::Affine};

  #[test]
  fn zero_sized_rect_produces_empty_bounds() {
    let bounds = bounds_for_rect(
      Size {
        width: 0.0,
        height: 100.0,
      },
      Affine::translation(50.0, 50.0),
    );

    assert!(
      bounds.is_some_and(SceneBounds::is_empty),
      "zero-sized rect should produce empty bounds, got {:?}",
      bounds.map(|bounds| (bounds.left, bounds.top, bounds.right, bounds.bottom))
    );
  }

  #[test]
  fn merge_bounds_ignores_empty_bounds() {
    let empty = SceneBounds {
      left: 0,
      top: 5,
      right: 0,
      bottom: 10,
    };
    let real = SceneBounds {
      left: 1000,
      top: 0,
      right: 1200,
      bottom: 50,
    };

    for (left, right) in [(empty, real), (real, empty)] {
      let merged = merge_bounds(Some(left), Some(right));
      assert!(
        merged.is_some_and(
          |bounds| (bounds.left, bounds.top, bounds.right, bounds.bottom)
            == (real.left, real.top, real.right, real.bottom)
        ),
        "empty bounds must not expand the union"
      );
    }
  }
}
