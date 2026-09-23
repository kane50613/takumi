//! Text / inline-content emission.
//!
//! Builds the one shared inline enumeration ([`resolve_inline_runs`], the same
//! producer the raster backend consumes) and emits each run's glyphs (outline
//! `<path>`, COLR color layers, or bitmap `<image>`), text decorations,
//! text-shadows, and `-webkit-text-stroke`. The layout/positioning is shared with
//! raster; only the painting differs.

use std::{io, sync::Arc};

use takumi_core::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, Point},
  layout::{
    inline::{
      DecorationRect, InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineOutlineRect,
      InlineRunLayout, PositionedInlineRun, ProcessedInlineSpan, ShapedRun, collect_inline_items,
      create_inline_layout, outline_island_contour, outline_islands,
    },
    node::TextData,
    tree::RenderNode,
  },
  painter::paint_run_decorations,
  resources::{font::FontError, glyph::ResolvedGlyph, image::to_data_url},
  style::{Affine, BackgroundClip, LineJoin, TextDecorationLines},
};

use crate::{
  Frame, Rgba, SvgDocument,
  box_model::path_data,
  gradient::LayerEmitter,
  render::{DocumentDevice, emit_inline_box},
};

const WHITE: Rgba = Rgba([255, 255, 255, 255]);

/// Where inline text sits: its container `layout` and the container's absolute
/// border-box top-left `(origin_x, origin_y)`.
#[derive(Clone, Copy)]
struct TextFrame {
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
}

impl TextFrame {
  fn new(layout: Layout, origin_x: f32, origin_y: f32) -> Self {
    Self {
      layout,
      origin_x,
      origin_y,
    }
  }

  /// Shifts the origin by `(dx, dy)` (for the text-shadow pass).
  fn shifted(self, dx: f32, dy: f32) -> Self {
    Self {
      origin_x: self.origin_x + dx,
      origin_y: self.origin_y + dy,
      ..self
    }
  }

  /// The `path_data` matrix translating to the origin.
  fn translation(self) -> [f32; 6] {
    [1.0, 0.0, 0.0, 1.0, self.origin_x, self.origin_y]
  }

  /// Moves a border-box-relative transform to absolute space.
  fn place(self, transform: Affine) -> Affine {
    Affine {
      x: transform.x + self.origin_x,
      y: transform.y + self.origin_y,
      ..transform
    }
  }
}

/// Emits a leaf [`TextData`] node at its absolute border-box top-left.
pub(crate) fn emit_text(
  text: &TextData,
  context: &RenderContext,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  emit_inline_items(
    context,
    || {
      vec![InlineItem::Text {
        text: text.text.as_str().into(),
        context,
        link: None,
        decorations: None,
      }]
    },
    TextFrame::new(layout, origin_x, origin_y),
    doc,
  )
}

/// Emits a container's inline formatting context (anonymous text + inline
/// children) at its absolute border-box top-left. Mirrors the raster backend's
/// container inline path.
pub(crate) fn emit_inline_content(
  node: &RenderNode,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  emit_inline_items(
    &node.context,
    || collect_inline_items(node),
    TextFrame::new(layout, origin_x, origin_y),
    doc,
  )
}

/// Lays out the inline items in the content box, paints the runs, then recurses
/// into each positioned inline box.
fn emit_inline_items<'c>(
  context: &'c RenderContext,
  items: impl FnOnce() -> Vec<InlineItem<'c>>,
  frame: TextFrame,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let font_style = SizedFontStyle::from_style(&context.style, context);
  if font_style.sizing.font_size == 0.0 {
    return Ok(());
  }

  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    items(),
    frame.layout.unsnapped_content,
    &font_style,
    context,
    InlineLayoutMode::Draw,
  ));

  let runs = built
    .resolve_runs(context, frame.layout)
    .map_err(font_error)?;
  emit_runs(doc, &runs, &built.spans, &font_style, context, frame)?;

  for inline_box in &runs.inline_boxes {
    if let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(inline_box.id as usize) {
      emit_inline_box(
        inline_box,
        item,
        frame.layout,
        frame.origin_x,
        frame.origin_y,
        doc,
      )?;
    }
  }
  Ok(())
}

/// Paints a resolved run layout in CSS text-decoration order: shadows, under/over
/// decorations, glyphs, then line-through.
fn emit_runs(
  doc: &mut SvgDocument,
  runs: &InlineRunLayout,
  spans: &[ProcessedInlineSpan<'_>],
  font_style: &SizedFontStyle,
  context: &RenderContext,
  frame: TextFrame,
) -> io::Result<()> {
  // Inline-span backgrounds fill under every glyph of the formatting context.
  for fragment in &runs.background_fragments {
    let data = path_data(&fragment.path(), frame.translation());

    if data.is_empty() {
      continue;
    }
    let group = (fragment.opacity < 1.0)
      .then(|| doc.begin_group(Affine::IDENTITY, fragment.opacity, None, None))
      .transpose()?;

    doc.path(&data, Rgba(fragment.color.0), false)?;
    if let Some(group) = group {
      doc.end_group(group)?;
    }
  }

  // text-shadow paints below the glyphs; later-listed shadows paint lowest.
  for shadow in font_style.painted_text_shadows() {
    let color = Rgba(shadow.color.0);
    let filter = (shadow.blur_radius > 0.0)
      .then(|| doc.blur_filter(shadow.blur_radius / 2.0))
      .transpose()?;
    let group = doc.begin_group(Affine::IDENTITY, 1.0, None, filter.as_deref())?;
    let shadow_frame = frame.shifted(shadow.offset_x, shadow.offset_y);
    for run in &runs.runs {
      emit_run_glyphs(doc, run, font_style, shadow_frame, Some(color), None)?;
    }
    doc.end_group(group)?;
  }

  let decorations: Vec<Vec<DecorationRect>> = runs
    .runs
    .iter()
    .map(|run| {
      run.glyph_run.decorations(
        &run.resolved_glyphs,
        frame.layout,
        run.baseline_shift,
        run.transform(Affine::IDENTITY),
      )
    })
    .collect();

  for (run, decorations) in runs.runs.iter().zip(&decorations) {
    emit_run_decorations(doc, run, decorations, frame, false)?;
  }

  if context.style.background_clip == BackgroundClip::Text {
    emit_clip_text_glyphs(doc, runs, font_style, context, frame)?;
  } else {
    for run in &runs.runs {
      let stroke = run_stroke(&run.glyph_run, font_style);

      emit_run_glyphs(doc, run, font_style, frame, None, stroke)?;
    }
  }

  // Text outlines stroke between the glyphs and the line-through, matching the
  // raster backend's painting order.
  emit_inline_outlines(doc, runs, spans, frame)?;

  for (run, decorations) in runs.runs.iter().zip(&decorations) {
    emit_run_decorations(doc, run, decorations, frame, true)?;
  }
  Ok(())
}

/// Strokes the shared inline outline contours ([`outline_islands`]) for each
/// styled span, mirroring the raster backend's merged-island outlines.
fn emit_inline_outlines(
  doc: &mut SvgDocument,
  runs: &InlineRunLayout,
  spans: &[ProcessedInlineSpan<'_>],
  frame: TextFrame,
) -> io::Result<()> {
  if runs.outline_rects.is_empty() {
    return Ok(());
  }
  for island in outline_islands(runs.outline_rects.clone()) {
    emit_outline_island(doc, &island, spans, frame)?;
  }
  Ok(())
}

fn emit_outline_island(
  doc: &mut SvgDocument,
  island: &[InlineOutlineRect],
  spans: &[ProcessedInlineSpan<'_>],
  frame: TextFrame,
) -> io::Result<()> {
  let Some(first_rect) = island.first() else {
    return Ok(());
  };
  let Some(ProcessedInlineSpan::Text { style, .. }) = spans.get(first_rect.span_id as usize) else {
    return Ok(());
  };
  // The device skips a transparent stroke, but an inline outline never reaches
  // one, so the emptiness is checked here instead.
  let Some(stroke) = style.outline_stroke().filter(|s| s.color.0[3] != 0) else {
    return Ok(());
  };

  let contour = outline_island_contour(island, style.outline_offset + stroke.width / 2.0);
  let data = path_data(&contour, frame.translation());
  if data.is_empty() {
    return Ok(());
  }

  let opacity = style.parent.opacity.0;
  let group = (opacity < 1.0)
    .then(|| doc.begin_group(Affine::IDENTITY, opacity, None, None))
    .transpose()?;
  doc.stroke_path(&data, &stroke)?;
  if let Some(group) = group {
    doc.end_group(group)?;
  }
  Ok(())
}

/// The `-webkit-text-stroke` a run carries. A span may set it for itself, so it
/// comes off the run; the join is a box-level property and stays with the node.
fn run_stroke<'j>(run: &ShapedRun, font_style: &'j SizedFontStyle) -> Option<(Rgba, f32, &'j str)> {
  let brush = &run.brush;

  (brush.stroke_width > 0.0 && brush.stroke_color.0[3] != 0).then_some((
    Rgba(brush.stroke_color.0),
    brush.stroke_width,
    line_join_str(font_style.parent.stroke_linejoin),
  ))
}

/// Emits glyphs filled by the element's background (`background-clip: text`).
///
/// Mirrors the raster backend: the background (color + images) is painted into
/// the glyph coverage, widened by any `-webkit-text-stroke` (so a transparent
/// stroke reveals a background-colored outline ring); the `color` (brush) then
/// fills the un-widened glyph interiors on top, followed by the real text stroke.
///
/// The coverage is expressed as an SVG `<mask>` (white glyph fill ∪ stroke)
/// rather than a `<clipPath>`, because a clip path ignores stroke width and so
/// can't reach the stroke-widened ring, so the background would only fill the thin
/// glyph interior. A mask honors the stroke, so the background fills the full
/// fill+stroke coverage.
fn emit_clip_text_glyphs(
  doc: &mut SvgDocument,
  runs: &InlineRunLayout,
  font_style: &SizedFontStyle,
  context: &RenderContext,
  frame: TextFrame,
) -> io::Result<()> {
  let join = line_join_str(font_style.parent.stroke_linejoin);

  let (mask_token, mask_ref) = doc.begin_mask()?;
  let mut any = false;
  for run in &runs.runs {
    any |= emit_clip_text_mask_glyphs(doc, run, frame, join)?;
  }
  doc.end_mask(mask_token)?;
  if !any {
    return Ok(());
  }

  let background = Rgba(
    context
      .style
      .background_color
      .resolve(context.current_color)
      .0,
  );
  let area = Frame::new(
    frame.origin_x,
    frame.origin_y,
    frame.layout.size.width,
    frame.layout.size.height,
  );

  let group = doc.begin_masked_group(&mask_ref)?;
  if background.0[3] != 0 {
    doc.rect(area.x, area.y, area.w, area.h, background)?;
  }
  if let Some(images) = context.style.background_image.as_deref() {
    LayerEmitter::new(context, doc).background_images(images, area, area)?;
  }
  doc.end_group(group)?;

  // The `color` (brush) fills the glyph interiors on top of the background, with
  // the real text stroke (a transparent stroke adds nothing visible).
  for run in &runs.runs {
    let stroke = run_stroke(&run.glyph_run, font_style);

    emit_run_glyphs(doc, run, font_style, frame, None, stroke)?;
  }
  Ok(())
}

/// Paints a run's outline glyphs white into the active mask with both fill and
/// stroke (and any faux-bold embolden), so the mask covers the full fill+stroke
/// glyph coverage. Returns whether any glyph was emitted.
fn emit_clip_text_mask_glyphs(
  doc: &mut SvgDocument,
  run: &PositionedInlineRun,
  frame: TextFrame,
  join: &str,
) -> io::Result<bool> {
  let run_transform = run.transform(Affine::IDENTITY);
  let glyph_offset = run.glyph_offset(frame.layout);
  let stroke_width = run.glyph_run.brush.stroke_width;
  let mut any = false;
  for glyph in &run.glyph_run.glyphs {
    let Some(ResolvedGlyph::Outline(outline)) = run.resolved_glyphs.get(&glyph.id).map(Arc::as_ref)
    else {
      continue;
    };
    let matrix =
      run_transform * Affine::translation(glyph_offset.x + glyph.x, glyph_offset.y + glyph.y);
    let data = path_data(outline.paths(), frame.place(matrix).to_cols_array());
    if data.is_empty() {
      continue;
    }
    any = true;
    if let Some(embolden) = outline.embolden().filter(|embolden| *embolden > 0.0) {
      doc.glyph_path(&data, WHITE, Some((WHITE, embolden, join)))?;
    }
    doc.glyph_path(
      &data,
      WHITE,
      (stroke_width > 0.0).then_some((WHITE, stroke_width, join)),
    )?;
  }
  Ok(any)
}

/// Emits a run's under/overline (`over == false`) or line-through (`over == true`)
/// decoration rects.
fn emit_run_decorations(
  doc: &mut SvgDocument,
  run: &PositionedInlineRun,
  decorations: &[DecorationRect],
  frame: TextFrame,
  over: bool,
) -> io::Result<()> {
  let opacity = run.glyph_run.brush.opacity;
  let opacity_group = (opacity < 1.0)
    .then(|| doc.begin_group(Affine::IDENTITY, opacity, None, None))
    .transpose()?;
  let mut device = DocumentDevice::new(doc);

  paint_run_decorations(
    decorations,
    over,
    TextDecorationLines::empty(),
    Point {
      x: frame.origin_x,
      y: frame.origin_y,
    },
    &mut device,
  );
  device.finish()?;
  if let Some(group) = opacity_group {
    doc.end_group(group)?;
  }
  Ok(())
}

/// Emits a run's glyphs. `color_override` (for shadows) recolors every glyph and
/// suppresses bitmaps/COLR; `stroke` adds `-webkit-text-stroke` to outlines.
fn emit_run_glyphs(
  doc: &mut SvgDocument,
  run: &PositionedInlineRun,
  font_style: &SizedFontStyle,
  frame: TextFrame,
  color_override: Option<Rgba>,
  stroke: Option<(Rgba, f32, &str)>,
) -> io::Result<()> {
  let run_transform = run.transform(Affine::IDENTITY);
  let glyph_offset = run.glyph_offset(frame.layout);
  let fill_color = run.glyph_run.brush.color;
  let bold_join = line_join_str(font_style.parent.stroke_linejoin);

  // Per-run (inline span) opacity, matching the raster backend's
  // `draw_with_inline_opacity`.
  let opacity = run.glyph_run.brush.opacity;
  let opacity_group = (opacity < 1.0)
    .then(|| doc.begin_group(Affine::IDENTITY, opacity, None, None))
    .transpose()?;

  // Plain outline glyphs are interned in glyph space (translation stripped) and
  // emitted as `<use>` references, so repeated glyphs cost one outline plus a
  // `<use>` per occurrence. Faux-bold, COLR layers, and bitmap glyphs need their
  // own paint, so the accumulated run is flushed before each.
  let fill = color_override.unwrap_or(Rgba(fill_color.0));
  let mut uses: Vec<(u32, f32, f32)> = Vec::new();

  for glyph in &run.glyph_run.glyphs {
    let Some(resolved) = run.resolved_glyphs.get(&glyph.id) else {
      continue;
    };
    let matrix =
      run_transform * Affine::translation(glyph_offset.x + glyph.x, glyph_offset.y + glyph.y);
    let placed = frame.place(matrix);
    let cols = placed.to_cols_array();

    match resolved.as_ref() {
      ResolvedGlyph::Outline(outline) => {
        let color_layers = if color_override.is_some() {
          Vec::new()
        } else {
          run.resolve_color_layers(outline, fill_color)
        };
        if color_layers.is_empty() {
          // Synthesized (faux) bold: the raster backend strokes the glyph with
          // its own fill color (`outline.embolden()`); mirror that here.
          match outline.embolden().filter(|embolden| *embolden > 0.0) {
            Some(embolden) => {
              let data = path_data(outline.paths(), cols);
              if data.is_empty() {
                continue;
              }
              doc.flush_glyph_uses(&mut uses, fill, stroke)?;
              doc.glyph_path(&data, fill, Some((fill, embolden, bold_join)))?;
              if let Some(text_stroke) = stroke {
                doc.glyph_path(&data, Rgba([0, 0, 0, 0]), Some(text_stroke))?;
              }
            }
            None => {
              let [a, b, c, d, x, y] = cols;
              let data = path_data(outline.paths(), [a, b, c, d, 0.0, 0.0]);

              if !data.is_empty() {
                uses.push((doc.glyph_ref(data), x, y));
              }
            }
          }
        } else {
          doc.flush_glyph_uses(&mut uses, fill, stroke)?;
          for (color, paths) in color_layers {
            if color.0[3] == 0 {
              continue;
            }
            let data = path_data(paths, cols);
            if !data.is_empty() {
              doc.glyph_path(&data, Rgba(color.0), None)?;
            }
          }
        }
      }
      // Color/bitmap glyphs (emoji) have no vector form, so embed the rasterized
      // pixmap as a `data:image/png` `<image>`. Skipped in the shadow pass.
      ResolvedGlyph::Bitmap(bitmap) => {
        if color_override.is_some() {
          continue;
        }
        let Some(png) = bitmap.image.encode_png() else {
          continue;
        };
        doc.flush_glyph_uses(&mut uses, fill, stroke)?;
        let (width, height) = (bitmap.image.width(), bitmap.image.height());
        let bitmap_matrix = placed
          * Affine::translation(bitmap.placement.left as f32, -(bitmap.placement.top as f32))
          * Affine::scale(bitmap.scale_x, bitmap.scale_y);
        let href = to_data_url("image/png", &png);
        let group = doc.begin_group(bitmap_matrix, 1.0, None, None)?;
        doc.image(0.0, 0.0, width as f32, height as f32, &href, None)?;
        doc.end_group(group)?;
      }
    }
  }
  doc.flush_glyph_uses(&mut uses, fill, stroke)?;

  if let Some(group) = opacity_group {
    doc.end_group(group)?;
  }
  Ok(())
}

fn line_join_str(join: LineJoin) -> &'static str {
  match join {
    LineJoin::Round => "round",
    LineJoin::Bevel => "bevel",
    _ => "miter",
  }
}

fn font_error(error: FontError) -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    format!("glyph resolution failed: {error}"),
  )
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use takumi_core::{Fonts, layout::node::Node, resources::font::FontResource, viewport::Viewport};

  use crate::render::{SvgOptions, render};

  /// Registers the raw-TTF test font as a fallback for all scripts so the
  /// default font-family resolves to it (no `woff2` feature required).
  fn font_context_with_font() -> Fonts {
    let mut fonts = Fonts::default();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");
    let data = std::fs::read(&path).expect("read test font");
    fonts
      .register(FontResource::new(data))
      .expect("load test font");
    fonts
  }

  #[test]
  fn text_renders_glyph_paths_not_bitmap() {
    let fonts = font_context_with_font();
    let node = Node::text("Hi".to_string());
    let svg = render(
      SvgOptions::builder()
        .node(node)
        .viewport(Viewport::new((200, 80)))
        .fonts(&fonts)
        .build(),
    )
    .unwrap();
    assert!(svg.contains("<path"), "expected glyph <path> elements");
    assert!(
      !svg.contains("base64"),
      "text must be vector, not embedded bitmap"
    );
  }
}
