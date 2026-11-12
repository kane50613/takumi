//! Walks the stacking-context scene in paint order, recording each node and the steps that
//! paint it.

use super::{
  document::{
    DrawPart, Drawable, Effects, ElementInfo, ImageSource, NodeKind, Paint, PaintFilter,
    PaintGlyph, PaintNode, PaintRect, PaintStep, Role, Sampling, Shape, TextRun, Unpainted,
    UnpaintedReason,
  },
  fonts::FontTable,
  outlines::Outlines,
  record::Recorder,
};
use crate::{
  context::RenderContext,
  error::{Error, Result},
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Size},
  layout::{
    background_image_geometry::{BoxBackgroundPaintContext, FillLayers},
    inline::{InlineBackgroundFragment, InlineBoxKind, ProcessedInlineSpan},
    inline_box::{ReplacedInlineBox, resolve_inline_box},
    node::{ImageData, ImageSourceInput},
    tree::RenderNode,
  },
  paint_chunk::{ChunkPart, ConversionContext, PaintChunk, PropertySink},
  paint_property::{ClipId, ClipKind, ClipNode, EffectId, EffectKind, EffectNode},
  painter::{
    BackgroundClipArea, BoxFrame, BoxPainter, CellBackground, FillShape, InlineLines, OverflowClip,
    OwnContent, PaintDevice, PaintedInline, TableDecorations, TextClip, paint_collapsed_borders,
  },
  resources::image::{ImageSource as LoadedImage, sniff_mime, to_data_url},
  scene::{NodePaint, Scene},
  sort_key::sort_by_key,
  style::{Affine, BackgroundImage, Display, Filter, Isolation, TextAlign, ToCss},
};
use rustc_hash::FxHashMap;

/// What one box paints of a block's inline text, as the walker records it: the block, the lines,
/// where the block sits, and the baselines that number its lines.
#[derive(Clone, Copy)]
struct InlineText<'t> {
  block: &'t RenderNode,
  block_path: &'t [usize],
  painted: &'t PaintedInline<'t>,
  lines: &'t InlineLines<'t>,
  font_style: &'t SizedFontStyle<'t>,
  /// Maps the block's border box onto the page.
  transform: Affine,
  /// The block, at its border box's origin.
  frame: BoxFrame,
  baselines: &'t [f32],
}

/// A render node placed in the document, inside the box `parent`.
#[derive(Clone, Copy)]
struct Placed<'n> {
  node: &'n RenderNode,
  layout: ComputedLayout,
  transform: Affine,
  path: &'n [usize],
  parent: usize,
}

/// Builds a document's nodes and steps from laid-out scenes.
pub(super) struct Walker {
  pub(super) fonts: FontTable,
  pub(super) nodes: Vec<PaintNode>,
  pub(super) steps: Vec<PaintStep>,
  pub(super) outlines: Outlines,
  /// Each node's element path, `None` for a node without one.
  paths: Vec<Option<Vec<usize>>>,
}

impl Walker {
  /// A walker with `root` as its first node.
  pub(super) fn new(root: Option<PaintNode>) -> Self {
    let mut walker = Self {
      fonts: FontTable::default(),
      nodes: Vec::new(),
      steps: Vec::new(),
      outlines: Outlines::default(),
      paths: Vec::new(),
    };

    if let Some(root) = root {
      walker.add(root, None);
    }
    walker
  }

  /// Records `scene`, its element paths under `prefix`, chunk by chunk under their clips and
  /// effects.
  pub(super) fn scene(&mut self, scene: &Scene, prefix: &[usize]) -> Result<()> {
    let chunks = PaintChunk::in_paint_order(&scene.contexts);
    let owners = PaintChunk::effect_owners(&chunks, &scene.properties);
    let paints: FxHashMap<&[usize], &NodePaint> = chunks
      .iter()
      .map(|chunk| (chunk.node.path.as_slice(), chunk.node))
      .collect();
    let mut conversion = ConversionContext::new(
      &scene.properties,
      StepWriter {
        walker: self,
        scene,
        prefix,
        owners: &owners,
        paints: &paints,
        boxes: FxHashMap::default(),
        open: Vec::new(),
        error: None,
      },
    );

    for chunk in &chunks {
      conversion.switch_to(chunk.state());
      conversion.sink().chunk(chunk);
    }

    conversion.finish().error.map_or(Ok(()), Err)
  }

  /// Records a box as a node without steps, its background shown through `text_clip` when it has
  /// one, and returns the node.
  fn record_box(
    &mut self,
    node: &RenderNode,
    (layout, inline_layer): (ComputedLayout, bool),
    transform: Affine,
    path: Vec<usize>,
    text_clip: Option<&TextClip>,
    table: &TableDecorations,
  ) -> Result<usize> {
    let context = &node.context;
    let painter = BoxPainter::new(context, layout);
    let size = layout.size;
    // An inline layer's fragments paint its backgrounds, borders, and outline as its content.
    let drawables = if node.paints_own_box() && !inline_layer {
      decorations(&painter, layout, transform, text_clip, table)?
    } else {
      Vec::new()
    };
    let outline = painter
      .pending_outline(Point::ZERO)
      .filter(|_| !inline_layer)
      .map(|pending| {
        let mut recorder = Recorder::new(transform);

        pending.paint(&mut recorder);
        recorder.finish()
      })
      .unwrap_or_default();
    let effects = context
      .style
      .needs_offscreen_compositing()
      .then(|| Box::new(effects(&painter, layout)));
    let overflow_clip =
      OverflowClip::of(context, layout, context.box_paint_offset(layout)).map(|clip| {
        let (shape, origin) = clip.shape();

        Shape::of(&shape, Affine::translation(origin.x, origin.y))
      });
    let id = self.nodes.len();

    self.add(
      PaintNode {
        id,
        parent: None,
        element: ElementInfo::of(node, path.clone()),
        transform: transform.to_cols_array(),
        width: size.width,
        height: size.height,
        bounds: PaintRect::bounding(size.width, size.height, transform),
        drawables,
        children: Vec::new(),
        kind: NodeKind::Box {
          content_box: PaintRect::sized(
            layout.content_box_offset(),
            Size {
              width: layout.content_box_width(),
              height: layout.content_box_height(),
            },
          ),
          outline,
          effects,
          overflow_clip,
        },
      },
      Some(path),
    );

    Ok(id)
  }

  /// Records `content`, the text or image the node at `placed` paints.
  fn own_content(&mut self, placed: Placed<'_>, content: OwnContent<'_>) -> Result<()> {
    match content {
      OwnContent::Inline(_) | OwnContent::InlineLayer(..) => self.inline(placed, content),
      OwnContent::Image(image) => {
        self.image(image, placed);
        Ok(())
      }
      OwnContent::None => Ok(()),
    }
  }

  /// Records a replaced image in the node's content box.
  fn image(&mut self, image: &ImageData, placed: Placed<'_>) {
    let Placed {
      node,
      layout,
      transform,
      parent,
      ..
    } = placed;
    let context = &node.context;
    let offset = layout.content_box_offset();
    let content = Size {
      width: layout.content_box_width(),
      height: layout.content_box_height(),
    };

    if content.width <= 0.0 || content.height <= 0.0 {
      return;
    }

    let src = match &image.src {
      ImageSourceInput::Url(url) => url.to_string(),
      ImageSourceInput::Buffer(bytes) => to_data_url(sniff_mime(bytes), bytes),
      #[cfg(feature = "svg-sizing")]
      ImageSourceInput::Loaded(LoadedImage::Svg(svg)) => {
        to_data_url("image/svg+xml", svg.source().as_bytes())
      }
      _ => return,
    };
    let intrinsic = image
      .src
      .resolve(context)
      .ok()
      .map(|source| source.size(&context.sizing))
      .filter(|(width, height)| *width > 0.0 && *height > 0.0);
    let painter = BoxPainter::new(context, layout);
    let content_rect = PaintRect::sized(Point::ZERO, content);
    let (source, rect, clip) = match intrinsic {
      Some((width, height)) => {
        let replaced = painter.replaced_content(Size { width, height });
        // The clip is a border-box `ClipBox`; the content box it falls back to is already local.
        let clip = replaced
          .clip
          .map_or(Shape::Rect { rect: content_rect }, |clip| {
            Shape::of(
              &FillShape::from(clip),
              Affine::translation(-offset.x, -offset.y),
            )
          });

        (
          ImageSource { src, width, height },
          PaintRect::sized(replaced.placement.offset, replaced.placement.size),
          clip,
        )
      }
      None => (
        ImageSource {
          src,
          width: content.width,
          height: content.height,
        },
        content_rect,
        Shape::Rect { rect: content_rect },
      ),
    };
    let placed = transform * Affine::translation(offset.x, offset.y);
    let id = self.nodes.len();

    self.add(
      PaintNode {
        id,
        parent: Some(parent),
        element: self.nodes[parent].element.clone(),
        transform: placed.to_cols_array(),
        width: content.width,
        height: content.height,
        bounds: PaintRect::bounding(content.width, content.height, placed),
        drawables: vec![Drawable::Image {
          role: Role::Image,
          image: source.clone(),
          rect,
          clip,
          sampling: Sampling::of(context.style.misc_inherited_data.image_rendering),
        }],
        children: Vec::new(),
        kind: NodeKind::Image { image: source },
      },
      None,
    );
    self.draw(id);
  }

  /// Records the inline `content` the node at `placed` paints: its text, then the block's own
  /// inline boxes.
  fn inline(&mut self, placed: Placed<'_>, content: OwnContent<'_>) -> Result<()> {
    let Placed {
      node,
      layout,
      transform,
      path,
      ..
    } = placed;
    let block = match content {
      OwnContent::InlineLayer(block, layer) => (block, &path[..path.len() - layer.span_depth]),
      _ => (node, path),
    };
    let font_style = content.font_style(&node.context);
    let Some(painted) = content.lay_out_inline(&font_style, layout) else {
      return Ok(());
    };
    let painted = painted?;

    self.text(placed, block, &painted, &font_style);

    if !matches!(content, OwnContent::Inline(_)) {
      return Ok(());
    }

    for inline_box in painted
      .runs
      .inline_boxes
      .iter()
      .filter(|inline_box| inline_box.kind == InlineBoxKind::InFlow)
    {
      let Some(ProcessedInlineSpan::Box(item)) = painted.spans.get(inline_box.id as usize) else {
        continue;
      };
      let Some(relative) = node.path_to(item.render_node) else {
        continue;
      };
      let Some(ReplacedInlineBox { node, layout }) = resolve_inline_box(inline_box, item, layout)
      else {
        continue;
      };
      let box_path = [path, &relative].concat();

      let local = node.context.style.local_transform(
        layout.size.width,
        layout.size.height,
        &node.context.sizing,
      );
      let placed = transform * Affine::translation(layout.location.x, layout.location.y) * local;
      let id = self.record_box(
        node,
        (layout, false),
        placed,
        box_path.clone(),
        None,
        &TableDecorations::None,
      )?;
      let (group, clip, outline) = match &self.nodes[id].kind {
        NodeKind::Box {
          effects,
          overflow_clip,
          outline,
          ..
        } => (
          effects.is_some(),
          overflow_clip.is_some(),
          !outline.is_empty(),
        ),
        _ => (false, false, false),
      };

      if group {
        self.steps.push(PaintStep::BeginGroup { node: id });
      }
      if !self.nodes[id].drawables.is_empty() {
        self.draw(id);
      }
      if clip {
        self.steps.push(PaintStep::BeginClip { node: id });
      }
      self.own_content(
        Placed {
          node,
          layout,
          transform: placed,
          path: &box_path,
          parent: id,
        },
        OwnContent::of(node),
      )?;
      if clip {
        self.steps.push(PaintStep::EndClip { node: id });
      }
      if outline {
        self.steps.push(PaintStep::Draw {
          node: id,
          part: DrawPart::Outline,
        });
      }
      if group {
        self.steps.push(PaintStep::EndGroup { node: id });
      }
    }
    Ok(())
  }

  /// Records the inline content `painted` the box at `placed` paints of the block `block` at
  /// `block_path`: each span fragment with a background or border as a box, then the text, in the
  /// style `font_style`.
  fn text(
    &mut self,
    placed: Placed<'_>,
    (block, block_path): (&RenderNode, &[usize]),
    painted: &PaintedInline<'_>,
    font_style: &SizedFontStyle,
  ) {
    let lines = painted.lines(|_| true);
    let (transform, frame) = painted.block_placement(placed.transform);
    let mut baselines: Vec<f32> = painted
      .runs
      .runs
      .iter()
      .map(|run| run.glyph_run.baseline)
      .collect();

    sort_by_key(&mut baselines, |&baseline| baseline);
    baselines.dedup_by(|a, b| (*a - *b).abs() < 0.01);

    let inline = InlineText {
      block,
      block_path,
      painted,
      lines: &lines,
      font_style,
      transform,
      frame,
      baselines: &baselines,
    };
    let mut fragments = Vec::new();

    for &fragment in lines.background_fragments() {
      let parent = enclosing_fragment(fragment, &fragments).unwrap_or(placed.parent);

      if let Some(id) = self.inline_fragment(&inline, fragment, parent) {
        fragments.push((fragment, id));
      }
    }

    self.text_node(&inline, placed.parent);
  }

  /// Records an inline span's background and border on one line as a box node under the box
  /// `parent`, draws it, and returns it.
  ///
  /// Blink paints each line's fragment of an inline box on its own, in
  /// `InlineBoxFragmentPainter::Paint`, so each fragment is a box here.
  fn inline_fragment(
    &mut self,
    inline: &InlineText<'_>,
    fragment: &InlineBackgroundFragment<'_>,
    parent: usize,
  ) -> Option<usize> {
    let InlineText {
      block,
      block_path,
      painted,
      lines,
      font_style,
      transform,
      frame,
      ..
    } = *inline;
    let local = transform * Affine::translation(fragment.x, fragment.y);
    let mut recorder = Recorder::new(local);

    // The fragment draws at the block's origin, so shift the frame to put it at its own.
    lines.paint_background(
      fragment,
      &painted.spans,
      font_style,
      BoxFrame::new(frame.layout, Point::new(-fragment.x, -fragment.y)),
      &mut recorder,
    );

    let drawables = recorder.finish();

    if drawables.is_empty() {
      return None;
    }

    let element_path = block
      .path_to(fragment.owner())
      .map(|relative| [block_path, &relative].concat());
    let element = element_path
      .clone()
      .and_then(|element_path| ElementInfo::of(fragment.owner(), element_path));
    let size = Size {
      width: fragment.width,
      height: fragment.height,
    };
    let fragment_box =
      ComputedLayout::new(Point::ZERO, size, fragment.border.width, fragment.padding);
    let id = self.nodes.len();

    self.add(
      PaintNode {
        id,
        parent: Some(parent),
        element,
        transform: local.to_cols_array(),
        width: fragment.width,
        height: fragment.height,
        bounds: PaintRect::bounding(fragment.width, fragment.height, local),
        drawables,
        children: Vec::new(),
        kind: NodeKind::Box {
          content_box: PaintRect::sized(
            fragment_box.content_box_offset(),
            Size {
              width: fragment_box.content_box_width(),
              height: fragment_box.content_box_height(),
            },
          ),
          outline: Vec::new(),
          effects: None,
          overflow_clip: None,
        },
      },
      // The span's path orders the box among its siblings; its parent is already set.
      element_path,
    );
    self.draw(id);

    Some(id)
  }

  /// Records a text node of the runs `inline` paints under the box `parent`, with the outlines,
  /// and draws it.
  fn text_node(&mut self, inline: &InlineText<'_>, parent: usize) {
    let InlineText {
      block,
      block_path,
      painted,
      lines,
      font_style,
      transform,
      frame,
      baselines,
    } = *inline;
    let layout = frame.layout;
    let spans = &painted.spans;
    let text = painted.text();
    let context = &block.context;
    let runs = lines.runs();
    let mut recorder = Recorder::text(transform, runs.iter().map(|run| run.index).collect());

    lines.paint_glyphs(spans, font_style, frame, &mut recorder);

    let drawables = recorder.finish();

    if drawables.is_empty() && runs.is_empty() {
      return;
    }

    let text_runs = runs
      .iter()
      .map(|run| {
        let shaped = &run.glyph_run;
        let origin = run.origin(layout);
        let line_scale = run.transform(Affine::IDENTITY);
        let style = run.style(spans).unwrap_or(font_style);

        TextRun {
          text: run.text(text, spans),
          element: ElementInfo::authored(block, run.element(spans), block_path),
          x: origin.x,
          y: origin.y,
          width: shaped.advance,
          line: baselines
            .iter()
            .position(|baseline| (baseline - shaped.baseline).abs() < 0.01)
            .unwrap_or_default(),
          ascent: shaped.metrics.ascent,
          descent: shaped.metrics.descent,
          font: self.fonts.intern(context.fonts(), shaped),
          font_size: shaped.font_size,
          line_height: shaped.metrics.line_height,
          letter_spacing: style.letter_spacing,
          glyphs: shaped
            .glyphs
            .iter()
            .zip(run.glyph_clusters(text, spans))
            .map(|(glyph, cluster)| PaintGlyph {
              id: glyph.id,
              x: glyph.x - shaped.offset,
              y: glyph.y - shaped.baseline,
              cluster,
            })
            .collect(),
          transform: (!line_scale.is_identity()).then(|| {
            (Affine::translation(-origin.x, -origin.y)
              * line_scale
              * Affine::translation(origin.x, origin.y))
            .to_cols_array()
          }),
        }
      })
      .collect();
    let id = self.nodes.len();
    let size = layout.size;

    self
      .outlines
      .insert(id, runs.iter().map(|run| run.outline(layout)).collect());
    self.add(
      PaintNode {
        id,
        parent: Some(parent),
        element: self.nodes[parent].element.clone(),
        transform: transform.to_cols_array(),
        width: size.width,
        height: size.height,
        bounds: PaintRect::bounding(size.width, size.height, transform),
        drawables,
        children: Vec::new(),
        kind: NodeKind::Text {
          text_align: text_align(context),
          runs: text_runs,
        },
      },
      None,
    );
    self.draw(id);
  }

  /// Adds `node`, placed in the tree by its element `path` when its parent is not yet known.
  fn add(&mut self, node: PaintNode, path: Option<Vec<usize>>) {
    self.nodes.push(node);
    self.paths.push(path);
  }

  /// Records a table's collapsed borders as a node of their own inside the table `node`, and
  /// draws it.
  fn record_collapsed_borders(&mut self, table: usize, drawables: Vec<Drawable>, path: Vec<usize>) {
    let id = self.nodes.len();
    let PaintNode {
      transform,
      width,
      height,
      bounds,
      ..
    } = self.nodes[table].clone();

    self.add(
      PaintNode {
        id,
        parent: None,
        element: None,
        transform,
        width,
        height,
        bounds,
        drawables,
        children: Vec::new(),
        kind: NodeKind::Box {
          content_box: PaintRect::sized(Point::ZERO, Size::ZERO),
          outline: Vec::new(),
          effects: None,
          overflow_clip: None,
        },
      },
      Some(path),
    );
    self.draw(id);
  }

  fn draw(&mut self, node: usize) {
    self.steps.push(PaintStep::Draw {
      node,
      part: DrawPart::Drawables,
    });
  }

  /// Links every node to its parent and lists each box's children in document order. A box that
  /// shares its path with an earlier one, such as a list item's marker, belongs to that box.
  pub(super) fn link(&mut self) {
    let mut boxes: FxHashMap<&[usize], usize> = FxHashMap::default();

    // A node with its parent already set, such as an inline span's fragment, keeps its path only
    // for ordering, so it never becomes the parent of another node.
    for (id, (node, path)) in self.nodes.iter().zip(&self.paths).enumerate() {
      if let Some(path) = path
        && node.parent.is_none()
      {
        boxes.entry(path).or_insert(id);
      }
    }

    let parents: Vec<Option<usize>> = self
      .nodes
      .iter()
      .zip(&self.paths)
      .map(|(node, path)| {
        if node.id == 0 {
          return None;
        }
        if node.parent.is_some() {
          return node.parent;
        }

        let path = path.as_deref().unwrap_or_default();

        if let Some(&owner) = boxes.get(path)
          && owner != node.id
        {
          return Some(owner);
        }

        Some(
          (0..path.len())
            .rev()
            .find_map(|length| boxes.get(&path[..length]).copied())
            .unwrap_or_default(),
        )
      })
      .collect();

    for (id, parent) in parents.into_iter().enumerate() {
      self.nodes[id].parent = parent;
      if let Some(parent) = parent {
        self.nodes[parent].children.push(id);
      }
    }

    let order: Vec<Vec<usize>> = self
      .nodes
      .iter()
      .map(|node| {
        self.paths[node.id]
          .clone()
          .or_else(|| node.parent.and_then(|parent| self.paths[parent].clone()))
          .unwrap_or_default()
      })
      .collect();

    for node in &mut self.nodes {
      node.children.sort_by(|a, b| order[*a].cmp(&order[*b]));
    }
  }
}

/// A [`PropertySink`] turning a scene's chunks into steps.
struct StepWriter<'w, 's> {
  walker: &'w mut Walker,
  scene: &'s Scene,
  prefix: &'w [usize],
  owners: &'w [Option<&'s NodePaint>],
  /// Every box the chunks paint, by path.
  paints: &'w FxHashMap<&'s [usize], &'s NodePaint>,
  /// The node each box recorded as, by path.
  boxes: FxHashMap<Vec<usize>, Option<usize>>,
  /// The node each open clip or effect belongs to, when it was recorded.
  open: Vec<Option<usize>>,
  error: Option<Error>,
}

impl StepWriter<'_, '_> {
  /// The node `paint` records as, recording it the first time.
  fn node(&mut self, paint: &NodePaint) -> Option<usize> {
    if let Some(&node) = self.boxes.get(&paint.path) {
      return node;
    }

    let scene = self.scene;
    let node = recorded(scene, paint)
      .and_then(|recorded| {
        let Some((node, layout)) = recorded else {
          return Ok(None);
        };
        let text_clip = TextClip::of(&scene.root, scene.results.view(), paint, &node.context)?;
        let table = TableDecorations::of(&scene.root, scene.results.view(), paint)?;

        self
          .walker
          .record_box(
            node,
            (layout, paint.inline_layer.is_some()),
            paint.transform,
            [self.prefix, &paint.path].concat(),
            text_clip.as_ref(),
            &table,
          )
          .map(Some)
      })
      .unwrap_or_else(|error| {
        self.error.get_or_insert(error);
        None
      });

    self.boxes.insert(paint.path.clone(), node);
    node
  }

  /// Records one chunk's steps.
  fn chunk(&mut self, chunk: &PaintChunk<'_>) {
    if self.error.is_some() {
      return;
    }

    let Some(node) = self.node(chunk.node) else {
      return;
    };
    let walker = &mut *self.walker;

    match chunk.part {
      ChunkPart::Decorations => {
        if !walker.nodes[node].drawables.is_empty() {
          walker.draw(node);
        }
      }
      ChunkPart::Content => {
        let result = recorded(self.scene, chunk.node).and_then(|recorded| {
          let Some((render_node, layout)) = recorded else {
            return Ok(());
          };

          walker.own_content(
            Placed {
              node: render_node,
              layout,
              transform: chunk.node.transform,
              path: &[self.prefix, &chunk.node.path].concat(),
              parent: node,
            },
            OwnContent::of_paint(&self.scene.root, render_node, chunk.node),
          )
        });

        if let Err(error) = result {
          self.error.get_or_insert(error);
        }
      }
      ChunkPart::CollapsedBorders => {
        if !self
          .scene
          .root
          .node_at_path(&chunk.node.path)
          .is_some_and(RenderNode::paints_collapsed_borders)
        {
          return;
        }
        let mut recorder = Recorder::new(chunk.node.transform);
        let result = paint_collapsed_borders(
          self.scene.results.view(),
          chunk.node,
          Affine::IDENTITY,
          None,
          &mut recorder,
        );

        if let Err(error) = result {
          self.error.get_or_insert(error);
          return;
        }
        let drawables = recorder.finish();

        if !drawables.is_empty() {
          walker.record_collapsed_borders(
            node,
            drawables,
            [self.prefix, &chunk.node.path].concat(),
          );
        }
      }
      ChunkPart::Outline => {
        if let NodeKind::Box { outline, .. } = &walker.nodes[node].kind
          && !outline.is_empty()
        {
          walker.steps.push(PaintStep::Draw {
            node,
            part: DrawPart::Outline,
          });
        }
      }
    }
  }

  /// Opens the step `begin` makes for the box at `owner`.
  fn open(&mut self, owner: Option<&NodePaint>, begin: impl FnOnce(usize) -> PaintStep) {
    let node = owner.and_then(|owner| self.node(owner));

    if let Some(node) = node {
      self.walker.steps.push(begin(node));
    }
    self.open.push(node);
  }

  /// Closes the most recent step pair with what `end` makes.
  fn close(&mut self, end: impl FnOnce(usize) -> PaintStep) {
    if let Some(Some(node)) = self.open.pop() {
      self.walker.steps.push(end(node));
    }
  }
}

impl PropertySink for StepWriter<'_, '_> {
  fn push_clip(&mut self, _id: ClipId, clip: &ClipNode) {
    if matches!(
      clip.kind,
      ClipKind::PixelMovingFilter(_) | ClipKind::ClipPath(_)
    ) {
      return;
    }
    let owner = self.paints.get(clip.owner.as_slice()).copied();

    self.open(owner, |node| PaintStep::BeginClip { node });
  }

  fn pop_clip(&mut self, clip: &ClipNode) {
    if matches!(clip.kind, ClipKind::Shape(_)) {
      self.close(|node| PaintStep::EndClip { node });
    }
  }

  fn begin_effect(&mut self, id: EffectId, effect: &EffectNode) {
    if effect.kind == EffectKind::Filter
      && effect.parent.is_some_and(|parent| {
        let parent = self.scene.properties.effect(parent);

        parent.kind == EffectKind::Group && parent.owner == effect.owner
      })
    {
      self.open.push(None);
      return;
    }

    let owner = self.owners[id.index()];

    self.open(owner, |node| PaintStep::BeginGroup { node });
  }

  fn end_effect(&mut self) {
    self.close(|node| PaintStep::EndGroup { node });
  }
}

/// The render node `paint` places and its layout, or `None` when it paints nothing: it is gone,
/// invisible, or flattened by its transform.
pub(super) fn recorded<'s>(
  scene: &'s Scene,
  paint: &NodePaint,
) -> Result<Option<(&'s RenderNode, ComputedLayout)>> {
  let Some(node) = scene.root.node_at_path(&paint.path) else {
    return Ok(None);
  };
  let layout = paint.layout(scene.results.view())?;

  Ok(
    (!node.context.style.is_invisible() && paint.transform.is_invertible())
      .then_some((node, layout)),
  )
}

/// The shadows, background, and border the box `painter` paints at `layout`, bottom first, as
/// its part in a table, if any, paints them.
fn decorations(
  painter: &BoxPainter<'_>,
  layout: ComputedLayout,
  transform: Affine,
  text_clip: Option<&TextClip>,
  table: &TableDecorations,
) -> Result<Vec<Drawable>> {
  let mut recorder = Recorder::new(transform);
  let context = painter.context();

  match table {
    TableDecorations::None => {
      box_decorations(painter, Point::ZERO, text_clip, true, &mut recorder)?
    }
    TableDecorations::Cell {
      paints_border,
      clip,
    } => {
      if let Some(clip) = clip {
        recorder.push_clip(
          &FillShape::Rect(clip.size),
          Affine::translation(clip.offset.x, clip.offset.y),
        );
      }
      box_decorations(
        painter,
        Point::ZERO,
        text_clip,
        *paints_border,
        &mut recorder,
      )?;
      if clip.is_some() {
        recorder.pop_clip();
      }
    }
    TableDecorations::Table {
      grid,
      grid_offset,
      paints_border,
      columns,
    } => {
      let grid_painter = BoxPainter::new(context, *grid);

      box_decorations(
        &grid_painter,
        *grid_offset,
        text_clip,
        *paints_border,
        &mut recorder,
      )?;
      for column in columns {
        for cell in &column.cells {
          cell_background(&column.context, layout, cell, &mut recorder)?;
        }
      }
    }
    TableDecorations::Part { cells } => {
      painter.paint_normal_box_shadows(Point::ZERO, &mut recorder);
      for cell in cells {
        cell_background(context, layout, cell, &mut recorder)?;
      }
      painter.paint_inset_box_shadows(Point::ZERO, &mut recorder);
    }
  }

  Ok(recorder.finish())
}

/// The background of `context`, a table part, over `background.area` inside its cell, both
/// relative to `layout`, the box painting it.
fn cell_background(
  context: &RenderContext,
  layout: ComputedLayout,
  background: &CellBackground,
  recorder: &mut Recorder,
) -> Result<()> {
  let cell = background.cell;
  let at = background.area.offset;
  let painter = BoxPainter::new(
    context,
    ComputedLayout {
      location: layout.location + at,
      size: background.area.size,
      border: Default::default(),
      padding: Default::default(),
    },
  );

  recorder.push_clip(
    &FillShape::Rect(cell.size),
    Affine::translation(cell.offset.x, cell.offset.y),
  );
  let painted = self::background(&painter, at, None, recorder);

  recorder.pop_clip();
  painted
}

/// The shadows, background, and border the box `painter` paints with its border box at `origin`,
/// its border only when `paints_border`.
fn box_decorations(
  painter: &BoxPainter<'_>,
  origin: Point<f32>,
  text_clip: Option<&TextClip>,
  paints_border: bool,
  recorder: &mut Recorder,
) -> Result<()> {
  painter.paint_normal_box_shadows(origin, recorder);
  background(painter, origin, text_clip, recorder)?;
  painter.paint_inset_box_shadows(origin, recorder);
  if paints_border {
    painter.paint_border(origin, recorder);
  }
  Ok(())
}

/// The background the box `painter` paints with its border box at `origin`, shown through
/// `text_clip` when it has one.
fn background(
  painter: &BoxPainter<'_>,
  origin: Point<f32>,
  text_clip: Option<&TextClip>,
  recorder: &mut Recorder,
) -> Result<()> {
  let background = painter.background();
  let layers = Paint::layers(&background.layers, painter.context());
  // Blink's `BoxPainterBase::PaintFillLayers` paints a background that blends in a layer of its
  // own, so its layers blend only with its colour and one another.
  let isolated = layers.iter().any(|(_, blend_mode)| blend_mode.is_some());

  if isolated {
    recorder.begin_layer(1.0, None);
  }
  painter.background_color(origin, recorder);
  if let Some(clip) = background.clip.shape(background.size) {
    let mask = background.clip.border_mask();
    let offset = origin + background.offset;
    let shape = Shape::of(
      &mask.map_or(clip, |_| FillShape::Rect(background.size)),
      Affine::translation(offset.x, offset.y),
    );
    let fill = |recorder: &mut Recorder| {
      for (paint, blend_mode) in layers {
        let clips = recorder.clips();

        recorder.push(Drawable::Fill {
          role: Role::Background,
          shape: shape.clone(),
          paint,
          blend_mode,
          clips,
        });
      }
    };

    match mask {
      Some(mask) => recorder.with_border_mask(&mask, background.size, offset, fill),
      None => fill(recorder),
    }
  }
  if let Some(text_clip) = text_clip {
    text_clip.paint_background(origin, recorder)?;
  }
  if isolated {
    recorder.end_layer();
  }
  Ok(())
}

/// How the box `painter` paints composites, as a group.
fn effects(painter: &BoxPainter<'_>, layout: ComputedLayout) -> Effects {
  let context = painter.context();
  let style = &context.style;
  let size = layout.size;
  let backdrop: Vec<Filter> = style
    .misc1_data
    .backdrop_filter
    .iter()
    .filter(|filter| !filter.is_drop_shadow())
    .cloned()
    .collect();
  let mask = style
    .misc2_data
    .mask_image
    .as_deref()
    .filter(|images| images.iter().any(BackgroundImage::paints))
    .map(|images| {
      let layers = FillLayers::mask(style).resolve(
        images,
        &BoxBackgroundPaintContext::mask(size, context.box_paint_offset(layout)),
        context,
      );

      Paint::layers(&layers, context)
        .into_iter()
        .map(|(paint, blend_mode)| Drawable::Fill {
          role: Role::Background,
          shape: Shape::Rect {
            rect: PaintRect::sized(Point::ZERO, size),
          },
          paint,
          blend_mode,
          clips: Vec::new(),
        })
        .collect()
    });

  Effects {
    opacity: style.svg_data.opacity.0,
    blend_mode: style.misc_data.mix_blend_mode.to_css_string(),
    isolation: style.misc_data.isolation == Isolation::Isolate,
    filters: PaintFilter::chain(&style.misc1_data.filter, size, context),
    backdrop_clip: (!backdrop.is_empty()).then(|| {
      Shape::of(
        &BackgroundClipArea::BorderBox(*painter.border())
          .shape(size)
          .unwrap_or(FillShape::Rect(size)),
        Affine::IDENTITY,
      )
    }),
    backdrop_filters: PaintFilter::chain(&backdrop, size, context),
    clip: painter
      .clip_path()
      .map(|shape| Shape::of(&shape, Affine::IDENTITY)),
    mask,
  }
}

/// `text-align` with `start` and `end` resolved against the direction.
fn text_align(context: &RenderContext) -> &'static str {
  match context
    .style
    .inherited_data
    .text_align
    .resolve(context.style.inherited_data.direction)
  {
    TextAlign::Right => "right",
    TextAlign::Center => "center",
    TextAlign::Justify => "justify",
    TextAlign::Left | TextAlign::Start | TextAlign::End => "left",
  }
}

impl ElementInfo {
  /// The element `node` renders, at `path` from the input root, or `None` for an anonymous box.
  fn of(node: &RenderNode, path: Vec<usize>) -> Option<Self> {
    let input = node.node.as_ref()?;

    Some(Self {
      id: input.id().map(str::to_owned),
      tag_name: input.tag_name().map(str::to_owned),
      class_name: input.class_name().map(str::to_owned),
      path,
    })
  }
}

impl Unpainted {
  /// The outermost elements under `root` that painting skips, in document order.
  pub(super) fn under(root: &RenderNode) -> Vec<Self> {
    let mut unpainted = Vec::new();
    let mut pending = vec![(root, Vec::new())];

    while let Some((node, path)) = pending.pop() {
      let style = &node.context.style;
      let reason = if style.base_data.display == Display::None {
        Some(UnpaintedReason::DisplayNone)
      } else if style.is_invisible() {
        Some(UnpaintedReason::OpacityZero)
      } else {
        None
      };

      if let Some(reason) = reason {
        unpainted.extend(ElementInfo::of(node, path).map(|element| Self { element, reason }));
        continue;
      }

      let children = node.children.as_deref().unwrap_or_default();

      pending.extend(children.iter().enumerate().rev().map(|(index, child)| {
        let mut child_path = path.clone();

        child_path.push(index);
        (child, child_path)
      }));
    }

    unpainted
  }
}

impl ElementInfo {
  /// The element under `root` at `source_order`, at `path` from the input root. `None` for `root`
  /// itself.
  fn authored(root: &RenderNode, source_order: Option<usize>, path: &[usize]) -> Option<Self> {
    let source_order = source_order?;
    let relative = root.path_where(|candidate| candidate.source_order() == Some(source_order))?;

    if relative.is_empty() {
      return None;
    }

    Self::of(root.node_at_path(&relative)?, [path, &relative].concat())
  }
}

/// The node of the fragment, among `fragments`, of the innermost span around `fragment` on its
/// line, looking no further out than the inline layer `fragment` paints in.
fn enclosing_fragment(
  fragment: &InlineBackgroundFragment<'_>,
  fragments: &[(&InlineBackgroundFragment<'_>, usize)],
) -> Option<usize> {
  for link in fragment.link.ancestors() {
    let enclosing = fragments.iter().rev().find(|(outer, _)| {
      outer.link.decoration.id == link.decoration.id
        && (outer.y..=outer.y + outer.height).contains(&fragment.baseline)
    });

    if let Some(&(_, id)) = enclosing {
      return Some(id);
    }

    if link.decoration.layer {
      return None;
    }
  }

  None
}
