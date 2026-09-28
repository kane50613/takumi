//! Walks the stacking-context scene and records what each box paints.

use std::{borrow::Cow, ops::Range};

use crate::{
  context::RenderContext,
  error::Result,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point, Size},
  layout::{
    background::{BackgroundLayersInput, background_origin_box},
    border::BorderProperties,
    decoration::ClipBox,
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, PositionedInlineRun, ProcessedInlineSpan,
      collect_inline_items, create_inline_layout,
    },
    inline_box::{InlineBoxPaint, resolve_inline_box},
    node::{ImageData, ImageSourceInput, NodeKind},
    replaced::place_replaced,
    tree::{LayoutResults, RenderNode},
  },
  painter::BoxPainter,
  resources::font::FontsSnapshot,
  scene::{NodePaint, PaintItemKind, StackingContextNode},
  style::{
    Affine, BackgroundClip, BackgroundImage, BlendMode, Isolation, Overflow, ResolvedGradientStop,
    TextDecorationLines, ToCss,
  },
};

use super::{
  fonts::FontTable,
  tree::{
    PaintBackground, PaintBackgroundLayer, PaintBorder, PaintBoxShadows, PaintClip,
    PaintDecoration, PaintFill, PaintGlyph, PaintGradientStop, PaintImage, PaintInlineBackground,
    PaintNode, PaintOutline, PaintRect, PaintShadow, PaintSource, PaintStroke, PaintTextRun,
    PaintTiles, PaintUnresolvedEffects, Radii,
  },
};

/// Builds paint nodes from a laid-out render tree.
pub(super) struct Walker {
  pub(super) fonts: FontTable,
}

impl Walker {
  /// The nodes a whole scene paints, in paint order.
  pub(super) fn scene(
    &mut self,
    root: &RenderNode,
    results: &LayoutResults,
    contexts: &[StackingContextNode],
  ) -> Result<Vec<PaintNode>> {
    self.context(root, results, contexts, 0)
  }

  fn context(
    &mut self,
    root: &RenderNode,
    results: &LayoutResults,
    contexts: &[StackingContextNode],
    id: usize,
  ) -> Result<Vec<PaintNode>> {
    let Some(context) = contexts.get(id) else {
      return Ok(Vec::new());
    };
    let mut items = Vec::new();
    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(np) => items.extend(self.node(root, results, np)?),
          PaintItemKind::Context(child) => {
            items.extend(self.context(root, results, contexts, *child)?);
          }
        }
      }
    }
    match context.root() {
      Some(np) => Ok(match self.node(root, results, np)? {
        Some(mut node) => {
          node.children.extend(items);
          vec![node]
        }
        None => items,
      }),
      None => Ok(items),
    }
  }

  fn node(
    &mut self,
    root: &RenderNode,
    results: &LayoutResults,
    np: &NodePaint,
  ) -> Result<Option<PaintNode>> {
    let Some(node) = root.node_at_path(&np.path) else {
      return Ok(None);
    };
    let layout = results.layout(np.node_id)?;
    let mut painted = self.box_node(node, layout, np.transform, Some(np.path.clone()));
    painted.children = self.own_content(node, layout, np.transform, &mut painted)?;
    Ok(Some(painted))
  }

  /// A node's own box without its content.
  fn box_node(
    &mut self,
    node: &RenderNode,
    layout: ComputedLayout,
    transform: Affine,
    path: Option<Vec<usize>>,
  ) -> PaintNode {
    let style = &node.context.style;
    let painter = BoxPainter::new(&node.context, layout);
    let decorations = BoxDecorations::of(node, layout, &painter);
    let (x, y) = transform.transform_point(0.0, 0.0);
    PaintNode {
      source: path.map(|path| PaintSource::new(path, node.node.as_ref())),
      width: layout.size.width,
      height: layout.size.height,
      x,
      y,
      content_box: PaintRect::new(layout.content_box_offset(), layout.content_box_size()),
      transform: (!transform.only_translation()).then(|| transform.to_cols_array()),
      opacity: style.opacity.0,
      blend_mode: (style.mix_blend_mode != BlendMode::Normal)
        .then(|| style.mix_blend_mode.to_css_string()),
      isolate: style.isolation == Isolation::Isolate,
      clip: overflow_clip(node, layout, painter.border()),
      background: decorations.background,
      border: decorations.border,
      shadows: decorations.shadows,
      outline: decorations.outline,
      image: None,
      text_shadows: Vec::new(),
      inline_backgrounds: Vec::new(),
      text_runs: Vec::new(),
      text_align: None,
      unresolved_effects: unresolved(node),
      children: Vec::new(),
    }
  }

  /// Fills `painted` with the node's image or text and returns the boxes its inline content adds.
  fn own_content(
    &mut self,
    node: &RenderNode,
    layout: ComputedLayout,
    transform: Affine,
    painted: &mut PaintNode,
  ) -> Result<Vec<PaintNode>> {
    if node.should_create_inline_layout() {
      return self.inline_content(node, layout, transform, collect_inline_items(node), painted);
    }
    if node.has_anonymous_text_item_child() {
      return Ok(Vec::new());
    }
    match node.node.as_ref().map(|n| &n.kind) {
      Some(NodeKind::Image(image)) => {
        painted.image = image_content(image, node, layout);
        Ok(Vec::new())
      }
      Some(NodeKind::Text(text)) => {
        let items = vec![InlineItem::Text {
          text: Cow::Borrowed(text.text.as_str()),
          context: &node.context,
          link: None,
          decorations: None,
        }];
        self.inline_content(node, layout, transform, items, painted)
      }
      _ => Ok(Vec::new()),
    }
  }

  fn inline_content<'c>(
    &mut self,
    node: &'c RenderNode,
    layout: ComputedLayout,
    transform: Affine,
    items: Vec<InlineItem<'c>>,
    painted: &mut PaintNode,
  ) -> Result<Vec<PaintNode>> {
    let context = &node.context;
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let built = create_inline_layout(InlineLayoutRequest::in_content_box(
      items,
      layout.unsnapped_content,
      &font_style,
      context,
      InlineLayoutMode::Draw,
    ));
    let runs = built.resolve_runs(context, layout)?;

    painted.text_align = Some(
      context
        .style
        .text_align
        .resolve(context.style.direction)
        .to_css_string(),
    );
    painted.text_shadows = font_style
      .painted_text_shadows()
      .map(PaintShadow::from)
      .collect();
    painted.inline_backgrounds = runs
      .background_fragments
      .iter()
      .map(|fragment| PaintInlineBackground {
        rect: PaintRect {
          x: fragment.x,
          y: fragment.y,
          width: fragment.width,
          height: fragment.height,
        },
        radii: fragment.radii.map(|(x, y)| [x, y]),
        color: fragment.color.0,
        opacity: fragment.opacity,
      })
      .collect();
    painted.text_runs = runs
      .runs
      .iter()
      .map(|run| self.run(context.fonts(), &built.text, &built.spans, run, layout))
      .collect();

    let mut boxes = Vec::new();
    for inline_box in &runs.inline_boxes {
      let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(inline_box.id as usize) else {
        continue;
      };
      let Some((offset, paint)) = resolve_inline_box(inline_box, item, layout) else {
        continue;
      };
      match paint {
        InlineBoxPaint::Container(subtree) => {
          let at = subtree.border_box_origin(offset);
          let scene = subtree.into_scene(transform * Affine::translation(at.x, at.y), false)?;

          boxes.extend(self.scene(&scene.root, &scene.results, &scene.contexts)?);
        }
        InlineBoxPaint::Replaced { node, layout } => {
          let local = node.context.style.local_transform(
            layout.size.width,
            layout.size.height,
            &node.context.sizing,
          );
          let placed = transform * Affine::translation(offset.x, offset.y) * local;
          let mut painted = self.box_node(node, layout, placed, None);
          painted.children = self.own_content(node, layout, placed, &mut painted)?;
          boxes.push(painted);
        }
      }
    }
    Ok(boxes)
  }

  fn run(
    &mut self,
    fonts: &FontsSnapshot,
    text: &str,
    spans: &[ProcessedInlineSpan<'_>],
    run: &PositionedInlineRun,
    layout: ComputedLayout,
  ) -> PaintTextRun {
    let shaped = &run.glyph_run;
    let brush = &shaped.brush;
    let text_range = run_text_range(shaped.text_range.clone(), spans, brush.source_span_id);
    let glyph_offset = run.glyph_offset(layout);
    let origin = Point {
      x: glyph_offset.x + shaped.offset,
      y: glyph_offset.y + shaped.baseline,
    };
    let run_transform = run.transform(Affine::IDENTITY);
    let decorations = shaped
      .decorations(
        &run.resolved_glyphs,
        layout,
        run.baseline_shift,
        run_transform,
      )
      .into_iter()
      .map(|rect| PaintDecoration {
        line: decoration_line(rect.line),
        transform: rect.transform,
        width: rect.width,
        height: rect.height,
        color: rect.color.0,
      })
      .collect();

    PaintTextRun {
      text: run_text(text, text_range.clone()),
      x: origin.x,
      y: origin.y,
      width: shaped.advance,
      ascent: shaped.metrics.ascent,
      descent: shaped.metrics.descent,
      font_index: self.fonts.intern(fonts, shaped),
      font_size: shaped.font_size,
      line_height: shaped.metrics.line_height,
      letter_spacing: brush.letter_spacing,
      color: brush.color.0,
      opacity: brush.opacity,
      transform: (!run_transform.is_identity()).then(|| run_transform.to_cols_array()),
      glyphs: shaped
        .glyphs
        .iter()
        .map(|glyph| PaintGlyph {
          id: glyph.id,
          x: glyph.x - shaped.offset,
          y: glyph.y - shaped.baseline,
        })
        .collect(),
      decorations,
      stroke: (brush.stroke_width > 0.0 && brush.stroke_color.0[3] != 0).then_some(PaintStroke {
        color: brush.stroke_color.0,
        width: brush.stroke_width,
      }),
      text_byte_range: [text_range.start, text_range.end],
      span_id: brush.source_span_id,
    }
  }
}

/// Narrows a parley run's byte range to the span it was shaped for, since several spans can
/// share one run.
fn run_text_range(
  range: Range<usize>,
  spans: &[ProcessedInlineSpan<'_>],
  span_id: Option<u64>,
) -> Range<usize> {
  match span_id.and_then(|id| spans.get(id as usize)) {
    Some(ProcessedInlineSpan::Text { byte_range, .. }) => {
      range.start.max(byte_range.start)..range.end.min(byte_range.end)
    }
    _ => range,
  }
}

/// The run's text without the bidi marks layout inserts.
fn run_text(text: &str, range: Range<usize>) -> String {
  let end = range.end.min(text.len());
  let start = text.ceil_char_boundary(range.start.min(end));
  let end = text.floor_char_boundary(end);
  if start >= end {
    return String::new();
  }
  text[start..end]
    .chars()
    .filter(|c| !matches!(c, '\u{200E}' | '\u{200F}'))
    .collect()
}

fn decoration_line(line: TextDecorationLines) -> String {
  if line.contains(TextDecorationLines::UNDERLINE) {
    "underline"
  } else if line.contains(TextDecorationLines::OVERLINE) {
    "overline"
  } else {
    "line-through"
  }
  .to_string()
}

fn radii(border: &BorderProperties) -> Radii {
  border.radius.0.map(|pair| [pair.x, pair.y])
}

fn overflow_clip(
  node: &RenderNode,
  layout: ComputedLayout,
  border: &BorderProperties,
) -> Option<PaintClip> {
  let style = &node.context.style;
  let overflows = style.resolve_overflows();
  if !overflows.should_clip_content() {
    return None;
  }
  let clip = ClipBox::padding_box(*border, layout);
  Some(PaintClip {
    rect: PaintRect::new(clip.offset, clip.size),
    radii: radii(&clip.border),
    x: overflows.x != Overflow::Visible,
    y: overflows.y != Overflow::Visible,
  })
}

/// What a box paints around its content.
#[derive(Default)]
struct BoxDecorations {
  background: Option<PaintBackground>,
  border: Option<PaintBorder>,
  shadows: Option<PaintBoxShadows>,
  outline: Option<PaintOutline>,
}

impl BoxDecorations {
  fn of(node: &RenderNode, layout: ComputedLayout, painter: &BoxPainter<'_>) -> Self {
    let outline = painter.outline().map(|outline| {
      let width = outline.border.width.top;
      PaintOutline {
        width,
        color: outline.border.color.top.0,
        style: outline.border.style.top.to_css_string(),
        offset: outline.grow - width,
      }
    });
    if !painter.paints_decorations() {
      return Self {
        outline,
        ..Self::default()
      };
    }
    let context = &node.context;
    let style = &context.style;
    let border = painter.border();
    let shadows = painter.shadows();
    let background_color = style.background_color.resolve(context.current_color);
    let background = PaintBackground {
      color: (background_color.0[3] != 0).then_some(background_color.0),
      clip: style.background_clip.to_css_string(),
      layers: background_layers(node, layout),
    };

    Self {
      background: (background.color.is_some() || !background.layers.is_empty())
        .then_some(background),
      border: border.has_visible_sides().then(|| PaintBorder {
        widths: border.width.into_array(),
        colors: border.color.into_array().map(|color| color.0),
        styles: border.style.into_array().map(|style| style.to_css_string()),
        radii: radii(border),
      }),
      shadows: (!shadows.inset.is_empty() || !shadows.outer.is_empty()).then(|| PaintBoxShadows {
        inset: shadows.inset.iter().map(PaintShadow::from).collect(),
        outer: shadows.outer.iter().map(PaintShadow::from).collect(),
      }),
      outline,
    }
  }
}

fn background_layers(node: &RenderNode, layout: ComputedLayout) -> Vec<PaintBackgroundLayer> {
  let context = &node.context;
  let style = &context.style;
  if style.background_clip == BackgroundClip::Text {
    return Vec::new();
  }
  let images = style.background_image.as_deref().unwrap_or(&[]);
  if images.is_empty() {
    return Vec::new();
  }
  let origin = background_origin_box(style.background_origin, layout);
  let resolved = BackgroundLayersInput {
    images,
    positions: &style.background_position,
    sizes: &style.background_size,
    repeats: &style.background_repeat,
    blend_modes: &style.background_blend_mode,
    context,
    area: origin.size.map(|x| x.max(0.0) as u32),
    paint: layout.size.map(|x| x as u32),
    origin_offset: Point {
      x: origin.offset.x as i32,
      y: origin.offset.y as i32,
    },
  }
  .resolve();

  resolved
    .into_iter()
    .filter_map(|(index, geometry)| {
      let image = images.get(index)?;
      let fill = fill(image, geometry.tile_width, geometry.tile_height, context)?;
      Some(PaintBackgroundLayer {
        fill,
        tiles: PaintTiles {
          xs: geometry.xs.to_vec(),
          ys: geometry.ys.to_vec(),
          width: geometry.tile_width,
          height: geometry.tile_height,
        },
        blend_mode: geometry.blend_mode.to_css_string(),
      })
    })
    .collect()
}

fn fill(
  image: &BackgroundImage,
  width: u32,
  height: u32,
  context: &RenderContext,
) -> Option<PaintFill> {
  let stops = |stops: &[ResolvedGradientStop]| {
    stops
      .iter()
      .map(|stop| PaintGradientStop {
        color: stop.color.0,
        position: stop.position,
      })
      .collect()
  };
  Some(match image {
    BackgroundImage::None => return None,
    BackgroundImage::Linear(gradient) => {
      let geometry =
        gradient.resolve_geometry(width, height, &context.sizing, context.current_color);
      PaintFill::Linear {
        css: image.to_css_string(),
        repeating: gradient.repeating,
        dir_x: geometry.dir_x,
        dir_y: geometry.dir_y,
        axis_length: geometry.axis_length,
        stops: stops(geometry.stops()),
      }
    }
    BackgroundImage::Radial(gradient) => {
      let geometry =
        gradient.resolve_geometry(width, height, &context.sizing, context.current_color);
      let radius = |inverse: f32| if inverse > 0.0 { 1.0 / inverse } else { 0.0 };
      PaintFill::Radial {
        css: image.to_css_string(),
        repeating: gradient.repeating,
        cx: geometry.cx,
        cy: geometry.cy,
        radius_x: radius(geometry.inv_radius_x),
        radius_y: radius(geometry.inv_radius_y),
        stops: stops(geometry.stops()),
      }
    }
    BackgroundImage::Conic(_) => PaintFill::Conic {
      css: image.to_css_string(),
    },
    BackgroundImage::Url(src) => PaintFill::Image {
      src: Some(src.to_string()),
    },
  })
}

fn image_content(
  image: &ImageData,
  node: &RenderNode,
  layout: ComputedLayout,
) -> Option<PaintImage> {
  let context = &node.context;
  let content_size = layout.content_box_size();
  if content_size.width <= 0.0 || content_size.height <= 0.0 {
    return None;
  }
  let src = match &image.src {
    ImageSourceInput::Url(url) => Some(url.to_string()),
    _ => None,
  };
  let intrinsic = image
    .src
    .resolve(context)
    .ok()
    .map(|source| source.size(&context.sizing))
    .filter(|(width, height)| *width > 0.0 && *height > 0.0)
    .map(|(width, height)| Size { width, height });
  let placement = place_replaced(context, content_size, intrinsic.unwrap_or_default());

  Some(PaintImage {
    src,
    placement: PaintRect::new(
      layout.content_box_offset() + placement.offset,
      placement.size,
    ),
  })
}

fn unresolved(node: &RenderNode) -> Option<PaintUnresolvedEffects> {
  let style = &node.context.style;
  let css = |present: bool, css: String| present.then_some(css);
  let unresolved = PaintUnresolvedEffects {
    filter: css(!style.filter.is_empty(), style.filter.to_css_string()),
    backdrop_filter: css(
      !style.backdrop_filter.is_empty(),
      style.backdrop_filter.to_css_string(),
    ),
    mask_image: style
      .mask_image
      .as_ref()
      .filter(|images| !images.is_empty())
      .map(ToCss::to_css_string),
    clip_path: style.clip_path.as_ref().map(ToCss::to_css_string),
  };
  (unresolved.filter.is_some()
    || unresolved.backdrop_filter.is_some()
    || unresolved.mask_image.is_some()
    || unresolved.clip_path.is_some())
  .then_some(unresolved)
}
