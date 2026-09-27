//! Walks the stacking-context scene in paint order, recording each node and the steps that
//! paint it.

use std::{collections::HashMap, ptr};

use super::{
  document::{
    DrawPart, Drawable, Effects, ElementInfo, ImageSource, NodeKind, Paint, PaintFilter,
    PaintGlyph, PaintNode, PaintRect, PaintStep, Role, Sampling, Shape, TextRun,
  },
  fonts::FontTable,
  record::{RecordedText, Recorder},
};
use crate::{
  context::RenderContext,
  error::{Error, Result},
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Size},
  layout::{
    background_image_geometry::{FillLayers, OriginBox},
    inline::{
      BuiltInlineLayout, InlineLayoutMode, InlineLayoutRequest, InlinePass, InlineRunLayout,
      ProcessedInlineSpan, create_inline_layout,
    },
    inline_box::{InlineBoxPaint, resolve_inline_box},
    node::{ImageData, ImageSourceInput, NodeKind as InputKind},
    tree::RenderNode,
  },
  paint_chunk::{ChunkPart, ConversionContext, PaintChunk, PropertySink},
  paint_property::{ClipId, ClipNode, EffectId, EffectNode},
  painter::{
    BackgroundClipArea, BoxFrame, BoxPainter, FillShape, GlyphFill, OverflowClip, OwnContent,
    PaintDevice,
  },
  resources::image::{sniff_mime, to_data_url},
  scene::{NodePaint, Scene},
  style::{
    Affine, BackgroundClip, BackgroundImage, ComputedStyle, Direction, Filter, Isolation,
    TextAlign, ToCss,
  },
};

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
    let paints: HashMap<&[usize], &NodePaint> = chunks
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
        boxes: HashMap::new(),
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

  /// Records a box as a node without steps, and returns the node.
  fn record_box(
    &mut self,
    node: &RenderNode,
    layout: ComputedLayout,
    transform: Affine,
    path: Vec<usize>,
  ) -> usize {
    let context = &node.context;
    let painter = BoxPainter::new(context, layout);
    let size = layout.size;
    let drawables = if node.paints_own_box() {
      decorations(&painter, size, transform)
    } else {
      Vec::new()
    };
    let outline = painter
      .pending_outline(Point::ZERO)
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
    let overflow_clip = OverflowClip::of(context, layout).map(|clip| {
      let (shape, origin) = clip.shape(layout);

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

    id
  }

  /// Records the text or image the node lays out.
  fn own_content(&mut self, placed: Placed<'_>, pass: InlinePass) -> Result<()> {
    match OwnContent::of(placed.node) {
      OwnContent::Inline(_) => self.inline(placed, pass),
      OwnContent::Image(image) if pass == InlinePass::Content => {
        self.image(image, placed);
        Ok(())
      }
      OwnContent::Image(_) | OwnContent::None => Ok(()),
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
          sampling: Sampling::of(context.style.image_rendering),
        }],
        children: Vec::new(),
        kind: NodeKind::Image { image: source },
      },
      None,
    );
    self.draw(id);
  }

  /// Records the node's inline content: its text, then its inline boxes.
  fn inline(&mut self, placed: Placed<'_>, pass: InlinePass) -> Result<()> {
    let Placed {
      node,
      layout,
      transform,
      path,
      ..
    } = placed;
    let context = &node.context;
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some(items) = OwnContent::of(node).inline_items(&font_style) else {
      return Ok(());
    };
    let built = create_inline_layout(InlineLayoutRequest::in_content_box(
      items,
      layout.unsnapped_content,
      &font_style,
      context,
      InlineLayoutMode::Draw,
    ));
    let runs = built.resolve_runs(context, layout)?;

    if pass == InlinePass::Content {
      self.text(placed, &built, &runs, &font_style);
    }

    for inline_box in runs
      .inline_boxes
      .iter()
      .filter(|inline_box| pass.paints(inline_box))
    {
      let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(inline_box.id as usize) else {
        continue;
      };
      let Some((offset, paint)) = resolve_inline_box(inline_box, item, layout) else {
        continue;
      };
      let Some(relative) =
        node.path_where(or_marker(|candidate| ptr::eq(candidate, item.render_node)))
      else {
        continue;
      };
      let box_path = [path, &relative].concat();

      match paint {
        InlineBoxPaint::Container(subtree) => {
          let at = subtree.border_box_origin(offset);
          let scene = subtree.into_scene(transform * Affine::translation(at.x, at.y), false)?;

          self.scene(&scene, &box_path)?;
        }
        InlineBoxPaint::Replaced { node, layout } => {
          let local = node.context.style.local_transform(
            layout.size.width,
            layout.size.height,
            &node.context.sizing,
          );
          let placed = transform * Affine::translation(offset.x, offset.y) * local;
          let id = self.record_box(node, layout, placed, box_path.clone());
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
            InlinePass::Content,
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
      }
    }
    Ok(())
  }

  /// Records the node's text: the runs `built` lays out, painted in the style `font_style`.
  fn text(
    &mut self,
    placed: Placed<'_>,
    built: &BuiltInlineLayout<'_>,
    runs: &InlineRunLayout,
    font_style: &SizedFontStyle,
  ) {
    let Placed {
      node,
      layout,
      transform,
      path,
      parent,
    } = placed;
    let BuiltInlineLayout { spans, text, .. } = built;
    let context = &node.context;
    let painter = BoxPainter::new(context, layout);
    let fill = if context.style.background_clip == BackgroundClip::Text {
      GlyphFill::Background
    } else {
      GlyphFill::Text
    };
    let background = match fill {
      GlyphFill::Background => {
        let background = painter.background();

        background
          .color
          .map(|color| Paint::Color { color: color.0 })
          .into_iter()
          .chain(
            Paint::layers(&background.layers, layout.size, background.origin, context)
              .into_iter()
              .map(|(paint, _)| paint),
          )
          .collect()
      }
      GlyphFill::Text => Vec::new(),
    };
    let mut recorder = Recorder::text(
      RecordedText {
        runs: &runs.runs,
        background,
      },
      transform,
    );

    runs.paint(
      spans,
      font_style,
      fill,
      BoxFrame::new(layout, Point::ZERO),
      &mut recorder,
    );

    let drawables = recorder.finish();

    if drawables.is_empty() && runs.runs.is_empty() {
      return;
    }

    let mut baselines: Vec<f32> = runs.runs.iter().map(|run| run.glyph_run.baseline).collect();

    baselines.sort_by(f32::total_cmp);
    baselines.dedup_by(|a, b| (*a - *b).abs() < 0.01);

    let text_runs = runs
      .runs
      .iter()
      .map(|run| {
        let shaped = &run.glyph_run;
        let origin = run.origin(layout);
        let line_scale = run.transform(Affine::IDENTITY);
        let style = run.style(spans).unwrap_or(font_style);

        TextRun {
          text: run.text(text, spans),
          element: ElementInfo::styled(node, style.parent, path),
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
          line_height: shaped
            .brush
            .line_height_px
            .unwrap_or(shaped.metrics.ascent + shaped.metrics.descent),
          letter_spacing: style.letter_spacing,
          glyphs: shaped
            .glyphs
            .iter()
            .map(|glyph| PaintGlyph {
              id: glyph.id,
              x: glyph.x - shaped.offset,
              y: glyph.y - shaped.baseline,
            })
            .collect(),
          outline: run.outline(layout),
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

  fn draw(&mut self, node: usize) {
    self.steps.push(PaintStep::Draw {
      node,
      part: DrawPart::Drawables,
    });
  }

  /// Links every node to its parent and lists each box's children in document order. A box that
  /// shares its path with an earlier one, such as a list item's marker, belongs to that box.
  pub(super) fn link(&mut self) {
    let mut boxes: HashMap<&[usize], usize> = HashMap::new();

    for (id, path) in self.paths.iter().enumerate() {
      if let Some(path) = path {
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
  paints: &'w HashMap<&'s [usize], &'s NodePaint>,
  /// The node each box recorded as, by path.
  boxes: HashMap<Vec<usize>, Option<usize>>,
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

    let recorded = match recorded(self.scene, paint) {
      Ok(recorded) => recorded,
      Err(error) => {
        self.error.get_or_insert(error);
        None
      }
    };
    let node = recorded.map(|(node, layout)| {
      self.walker.record_box(
        node,
        layout,
        paint.transform,
        [self.prefix, &paint.path].concat(),
      )
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
      ChunkPart::Content | ChunkPart::Floats => {
        let pass = if chunk.part == ChunkPart::Floats {
          InlinePass::Floats
        } else {
          InlinePass::Content
        };
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
            pass,
          )
        });

        if let Err(error) = result {
          self.error.get_or_insert(error);
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
    let owner = self.paints.get(clip.owner.as_slice()).copied();

    self.open(owner, |node| PaintStep::BeginClip { node });
  }

  fn pop_clip(&mut self) {
    self.close(|node| PaintStep::EndClip { node });
  }

  fn begin_effect(&mut self, id: EffectId, _effect: &EffectNode) {
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
  let layout = scene.results.layout(paint.node_id)?;

  Ok(
    (!node.context.style.is_invisible() && paint.transform.is_invertible())
      .then_some((node, layout)),
  )
}

/// The shadows, background, and border the box `painter` paints, bottom first.
fn decorations(painter: &BoxPainter<'_>, size: Size<f32>, transform: Affine) -> Vec<Drawable> {
  let mut recorder = Recorder::new(transform);

  painter.paint_normal_box_shadows(Point::ZERO, &mut recorder);
  painter.background_color(Point::ZERO, &mut recorder);

  let background = painter.background();

  if let Some(clip) = background.clip.shape(size) {
    let mask = background.clip.border_mask();
    let shape = Shape::of(
      &mask.map_or(clip, |_| FillShape::Rect(size)),
      Affine::IDENTITY,
    );
    let layers = Paint::layers(
      &background.layers,
      size,
      background.origin,
      painter.context(),
    );
    let fill = |recorder: &mut Recorder<'_>| {
      for (paint, blend_mode) in layers {
        recorder.push(Drawable::Fill {
          role: Role::Background,
          shape: shape.clone(),
          paint,
          blend_mode,
          clips: Vec::new(),
        });
      }
    };

    match mask {
      Some(mask) => recorder.with_border_mask(&mask, size, Point::ZERO, fill),
      None => fill(&mut recorder),
    }
  }

  painter.paint_inset_box_shadows(Point::ZERO, &mut recorder);
  painter.paint_border(Point::ZERO, &mut recorder);
  recorder.finish()
}

/// How the box `painter` paints composites, as a group.
fn effects(painter: &BoxPainter<'_>, layout: ComputedLayout) -> Effects {
  let context = painter.context();
  let style = &context.style;
  let size = layout.size;
  let backdrop: Vec<Filter> = style
    .backdrop_filter
    .iter()
    .filter(|filter| !filter.is_drop_shadow())
    .cloned()
    .collect();
  let mask = style
    .mask_image
    .as_deref()
    .filter(|images| images.iter().any(BackgroundImage::paints))
    .map(|images| {
      let layers = FillLayers::mask(style).resolve(images, size, context);
      let area = OriginBox {
        offset: Point::ZERO,
        size,
      };

      Paint::layers(&layers, size, area, context)
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
    opacity: style.opacity.0,
    blend_mode: style.mix_blend_mode.to_css_string(),
    isolation: style.isolation == Isolation::Isolate,
    filters: PaintFilter::chain(&style.filter, size, context),
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
  let rtl = context.style.direction == Direction::Rtl;

  match context.style.text_align {
    TextAlign::Left => "left",
    TextAlign::Right => "right",
    TextAlign::Center => "center",
    TextAlign::Justify => "justify",
    TextAlign::Start if rtl => "right",
    TextAlign::End if !rtl => "right",
    TextAlign::Start | TextAlign::End => "left",
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

impl ElementInfo {
  /// The element under `root`, at `path`, whose text takes `style`: an anonymous text node's
  /// parent. `None` for `root` itself.
  fn styled(root: &RenderNode, style: &ComputedStyle, path: &[usize]) -> Option<Self> {
    let mut relative = root.path_where(or_marker(|candidate| {
      ptr::eq(&*candidate.context.style, style)
    }))?;

    if root
      .node_at_path(&relative)?
      .node
      .as_ref()
      .is_some_and(|input| matches!(input.kind, InputKind::Text(_)) && input.tag_name().is_none())
    {
      relative.pop();
    }
    if relative.is_empty() {
      return None;
    }

    Self::of(root.node_at_path(&relative)?, [path, &relative].concat())
  }
}

/// Accepts a node when `matches` accepts it or its marker, so a list item's marker sits at the
/// item's path.
fn or_marker(matches: impl Fn(&RenderNode) -> bool + Copy) -> impl Fn(&RenderNode) -> bool + Copy {
  move |node| matches(node) || node.marker.as_deref().is_some_and(matches)
}
