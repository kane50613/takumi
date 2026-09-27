//! Walks the stacking-context scene in paint order, recording each node and the steps that
//! paint it.

use std::{collections::HashMap, mem, ptr};

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
  error::Result,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Size},
  layout::{
    background_image_geometry::{FillLayers, OriginBox},
    inline::{
      BuiltInlineLayout, InlineLayoutMode, InlineLayoutRequest, InlineRunLayout,
      ProcessedInlineSpan, create_inline_layout,
    },
    inline_box::{InlineBoxPaint, resolve_inline_box},
    node::{ImageData, ImageSourceInput, NodeKind as InputKind},
    tree::RenderNode,
  },
  painter::{
    BackgroundClipArea, BoxFrame, BoxPainter, FillShape, GlyphFill, OverflowClip, OwnContent,
  },
  resources::image::{sniff_mime, to_data_url},
  scene::{BoxPart, NodePaint, PaintItemKind, PaintPhase, Scene},
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

/// A box whose steps are still open: its group, its overflow clip, and its outline.
struct OpenBox {
  node: usize,
  group: bool,
  clip: bool,
  outline: bool,
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

  /// Records `scene`, its element paths under `prefix`.
  pub(super) fn scene(&mut self, scene: &Scene, prefix: &[usize]) -> Result<()> {
    self.context(scene, 0, prefix)
  }

  fn context(&mut self, scene: &Scene, id: usize, prefix: &[usize]) -> Result<()> {
    let Some(context) = scene.contexts.get(id) else {
      return Ok(());
    };
    // The root's own content waits for its phase.
    let root = match context.root() {
      Some(paint) => self.open_node(scene, paint, prefix)?,
      None => None,
    };
    // A plain node owns no group, so its outline waits for the nodes that follow it, as Blink's
    // `kDescendantOutlinesOnly` pass paints them.
    let mut outlines = Vec::new();
    // The in-flow boxes the decorations phase opened, in order, each with whether it has an
    // outline, for the content phase to fill.
    let mut decorated: Vec<Option<(usize, bool)>> = Vec::new();

    for phase in context.paint_phases() {
      let (items, phase_part) = match phase {
        PaintPhase::RootContent => {
          if let (Some(paint), Some(open)) = (context.root(), &root) {
            self.node_content(scene, paint, prefix, open.node)?;
          }
          continue;
        }
        PaintPhase::Items(items, part) => (items, part),
      };
      let mut earlier = mem::take(&mut decorated).into_iter();

      for item in items {
        let Some(part) = item.part_in(phase_part) else {
          continue;
        };
        let paint = match &item.kind {
          PaintItemKind::Node(paint) => paint,
          PaintItemKind::Context(child) => {
            self.context(scene, *child, prefix)?;
            continue;
          }
        };

        match part {
          BoxPart::Decorations => {
            let open = self.open_node(scene, paint, prefix)?;

            decorated.push(open.as_ref().map(|open| (open.node, open.outline)));
            if let Some(mut open) = open {
              open.outline = false;
              self.close(open);
            }
          }
          BoxPart::Content => {
            if let Some(Some((node, outline))) = earlier.next() {
              self.node_content(scene, paint, prefix, node)?;
              if outline {
                outlines.push(node);
              }
            }
          }
          BoxPart::Whole => {
            if let Some(mut open) = self.open_node(scene, paint, prefix)? {
              self.node_content(scene, paint, prefix, open.node)?;
              if open.outline {
                outlines.push(open.node);
                open.outline = false;
              }
              self.close(open);
            }
          }
        }
      }
    }

    for node in outlines {
      self.steps.push(PaintStep::Draw {
        node,
        part: DrawPart::Outline,
      });
    }
    if let Some(open) = root {
      self.close(open);
    }
    Ok(())
  }

  /// Records a scene node's box, leaving its steps open.
  fn open_node(
    &mut self,
    scene: &Scene,
    paint: &NodePaint,
    prefix: &[usize],
  ) -> Result<Option<OpenBox>> {
    let Some((node, layout)) = recorded(scene, paint)? else {
      return Ok(None);
    };

    Ok(Some(self.open_box(
      node,
      layout,
      paint.transform,
      [prefix, &paint.path].concat(),
    )))
  }

  /// Records the own content of a scene node whose box is `parent`.
  fn node_content(
    &mut self,
    scene: &Scene,
    paint: &NodePaint,
    prefix: &[usize],
    parent: usize,
  ) -> Result<()> {
    let Some((node, layout)) = recorded(scene, paint)? else {
      return Ok(());
    };

    self.own_content(Placed {
      node,
      layout,
      transform: paint.transform,
      path: &[prefix, &paint.path].concat(),
      parent,
    })
  }

  /// Records a box and opens its group and overflow clip.
  fn open_box(
    &mut self,
    node: &RenderNode,
    layout: ComputedLayout,
    transform: Affine,
    path: Vec<usize>,
  ) -> OpenBox {
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
    let overflow_clip = OverflowClip::of(context, layout).map(|clip| match clip {
      OverflowClip::Rounded(clip) => Shape::of(&clip.into(), Affine::IDENTITY),
      OverflowClip::Axes { x, y } => Shape::Rect {
        rect: BoxFrame::new(layout, Point::ZERO)
          .overflow_clip_edges(x, y)
          .into(),
      },
    });
    let open = OpenBox {
      node: self.nodes.len(),
      group: effects.is_some(),
      clip: overflow_clip.is_some(),
      outline: !outline.is_empty(),
    };
    let has_drawables = !drawables.is_empty();

    self.add(
      PaintNode {
        id: open.node,
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

    if open.group {
      self.steps.push(PaintStep::BeginGroup { node: open.node });
    }
    if has_drawables {
      self.draw(open.node);
    }
    if open.clip {
      self.steps.push(PaintStep::BeginClip { node: open.node });
    }
    open
  }

  /// Closes a box's clip, paints its outline, and composites its group.
  fn close(&mut self, open: OpenBox) {
    if open.clip {
      self.steps.push(PaintStep::EndClip { node: open.node });
    }
    if open.outline {
      self.steps.push(PaintStep::Draw {
        node: open.node,
        part: DrawPart::Outline,
      });
    }
    if open.group {
      self.steps.push(PaintStep::EndGroup { node: open.node });
    }
  }

  /// Records the text or image the node lays out.
  fn own_content(&mut self, placed: Placed<'_>) -> Result<()> {
    match OwnContent::of(placed.node) {
      OwnContent::Inline(_) => self.inline(placed),
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
      _ => return,
    };
    let intrinsic = image
      .src
      .resolve(context)
      .ok()
      .map(|source| source.size(&context.sizing))
      .filter(|(width, height)| *width > 0.0 && *height > 0.0);
    let painter = BoxPainter::new(context, layout);
    let (source, rect, clip) = match intrinsic {
      Some((width, height)) => {
        let replaced = painter.replaced_content(Size { width, height });
        let clip = replaced
          .clip
          .map_or(FillShape::Rect(content), FillShape::from);

        (
          ImageSource { src, width, height },
          PaintRect::sized(replaced.placement.offset, replaced.placement.size),
          Shape::of(&clip, Affine::translation(-offset.x, -offset.y)),
        )
      }
      None => (
        ImageSource {
          src,
          width: content.width,
          height: content.height,
        },
        PaintRect::sized(Point::ZERO, content),
        Shape::Rect {
          rect: PaintRect::sized(Point::ZERO, content),
        },
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
  fn inline(&mut self, placed: Placed<'_>) -> Result<()> {
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

    self.text(placed, &built, &runs, &font_style);

    for inline_box in &runs.inline_boxes {
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
          let open = self.open_box(node, layout, placed, box_path.clone());

          self.own_content(Placed {
            node,
            layout,
            transform: placed,
            path: &box_path,
            parent: open.node,
          })?;
          self.close(open);
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
    let shape = Shape::of(&clip, Affine::IDENTITY);

    for (paint, blend_mode) in Paint::layers(
      &background.layers,
      size,
      background.origin,
      painter.context(),
    ) {
      recorder.push(Drawable::Fill {
        role: Role::Background,
        shape: shape.clone(),
        paint,
        blend_mode,
        clips: Vec::new(),
      });
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
