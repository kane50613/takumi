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
      create_inline_layout, inline_background_fragments, inline_background_path,
      outline_island_contour, outline_islands, resolve_inline_runs, run_decorations,
    },
    node::TextData,
    tree::RenderNode,
  },
  painter::paint_run_decorations,
  resources::{glyph::ResolvedGlyph, image::to_data_url},
  style::{BackgroundClip, LineJoin, TextDecorationLines},
};

use crate::{
  Affine, Frame, IDENTITY, Num, Rgba, SvgDocument,
  box_model::path_data,
  gradient::LayerEmitter,
  render::{DocumentDevice, emit_inline_box},
};

/// Where a run of inline text sits: its container `layout` (border/padding and
/// content size) and the container's absolute border-box top-left
/// `(origin_x, origin_y)`. Bundles the geometry threaded through the run/glyph
/// emitters; the shadow pass shifts `origin_*` per shadow.
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
}

/// Emits a leaf [`TextData`] node. `origin_x`/`origin_y` are the element's
/// absolute border-box top-left; `layout` carries border/padding and content size.
pub(crate) fn emit_text(
  text: &TextData,
  context: &RenderContext,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let font_style = SizedFontStyle::from_style(&context.style, context);
  let content = layout.content_box_size();
  if font_style.sizing.font_size == 0.0 || content.width <= 0.0 || content.height <= 0.0 {
    return Ok(());
  }

  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    vec![InlineItem::Text {
      text: text.text.as_str().into(),
      context,
      link: None,
    }],
    content,
    &font_style,
    context,
    InlineLayoutMode::Draw,
  ));

  let runs = resolve_inline_runs(&built, context, layout).map_err(font_error)?;
  emit_runs(
    doc,
    &runs,
    &built.spans,
    &font_style,
    context,
    TextFrame::new(layout, origin_x, origin_y),
  )
}

/// Emits a container's inline formatting context (anonymous text + inline
/// children), then recurses into each positioned inline box. Mirrors the raster
/// backend's container inline path. `origin_x`/`origin_y` are the container's
/// absolute border-box top-left.
pub(crate) fn emit_inline_content(
  node: &RenderNode,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let context = &node.context;
  let font_style = SizedFontStyle::from_style(&context.style, context);
  if font_style.sizing.font_size == 0.0 {
    return Ok(());
  }
  let content = layout.content_box_size();

  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    collect_inline_items(node),
    content,
    &font_style,
    context,
    InlineLayoutMode::Draw,
  ));

  let runs = resolve_inline_runs(&built, context, layout).map_err(font_error)?;
  emit_runs(
    doc,
    &runs,
    &built.spans,
    &font_style,
    context,
    TextFrame::new(layout, origin_x, origin_y),
  )?;

  for inline_box in &runs.inline_boxes {
    if let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(inline_box.id as usize) {
      emit_inline_box(inline_box, item, layout, origin_x, origin_y, doc)?;
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
  for fragment in inline_background_fragments(runs, spans) {
    let data = path_data(
      &inline_background_path(&fragment),
      [1.0, 0.0, 0.0, 1.0, frame.origin_x, frame.origin_y],
    );

    if data.is_empty() {
      continue;
    }
    let group = (fragment.opacity < 1.0)
      .then(|| doc.begin_group(IDENTITY, fragment.opacity, None, None))
      .transpose()?;

    doc.path(&data, Rgba(fragment.color.0), false)?;
    if let Some(group) = group {
      doc.end_group(group)?;
    }
  }

  // text-shadow paints below the glyphs; later-listed shadows paint lowest.
  for shadow in font_style.painted_text_shadows() {
    let color = Rgba(shadow.color.0);
    let filter = if shadow.blur_radius > 0.0 {
      Some(doc.blur_filter(shadow.blur_radius / 2.0)?)
    } else {
      None
    };
    let group = doc.begin_group(IDENTITY, 1.0, None, filter.as_deref())?;
    let shadow_frame = frame.shifted(shadow.offset_x, shadow.offset_y);
    for run in &runs.runs {
      emit_run_glyphs(doc, run, font_style, shadow_frame, Some(color), None, None)?;
    }
    doc.end_group(group)?;
  }

  let decorations: Vec<Vec<DecorationRect>> = runs
    .runs
    .iter()
    .map(|run| {
      run_decorations(
        &run.glyph_run,
        &run.resolved_glyphs,
        frame.layout,
        run.baseline_shift,
        run.transform(IDENTITY),
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

      emit_run_glyphs(doc, run, font_style, frame, None, stroke, None)?;
    }
  }

  // Text outlines stroke between the glyphs and the line-through, matching the
  // raster backend's painting order.
  emit_inline_outlines(doc, runs, spans, frame.origin_x, frame.origin_y)?;

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
  origin_x: f32,
  origin_y: f32,
) -> io::Result<()> {
  if runs.outline_rects.is_empty() {
    return Ok(());
  }
  for island in outline_islands(runs.outline_rects.clone()) {
    emit_outline_island(doc, &island, spans, origin_x, origin_y)?;
  }
  Ok(())
}

fn emit_outline_island(
  doc: &mut SvgDocument,
  island: &[InlineOutlineRect],
  spans: &[ProcessedInlineSpan<'_>],
  origin_x: f32,
  origin_y: f32,
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
  let width = stroke.width;
  let dasharray = stroke
    .dash
    .map(|[dash, gap]| format!("{} {}", Num(dash), Num(gap)));
  let linecap = stroke.round_cap.then_some("round");

  let contour = outline_island_contour(island, style.outline_offset + width / 2.0);
  let data = path_data(&contour, [1.0, 0.0, 0.0, 1.0, origin_x, origin_y]);
  if data.is_empty() {
    return Ok(());
  }

  let opacity = style.parent.opacity.0;
  let group = (opacity < 1.0)
    .then(|| doc.begin_group(IDENTITY, opacity, None, None))
    .transpose()?;
  doc.stroke_path(
    &data,
    Rgba(stroke.color.0),
    width,
    dasharray.as_deref(),
    linecap,
  )?;
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
  let white = Rgba([255, 255, 255, 255]);
  let join = line_join_str(font_style.parent.stroke_linejoin);

  let (mask_token, mask_ref) = doc.begin_mask()?;
  let mut any = false;
  for run in &runs.runs {
    any |= emit_clip_text_mask_glyphs(
      doc,
      run,
      frame,
      white,
      run.glyph_run.brush.stroke_width,
      join,
    )?;
  }
  doc.end_mask(mask_token)?;
  if !any {
    return Ok(());
  }

  let cc = context.current_color;
  let background = Rgba(context.style.background_color.resolve(cc).0);
  let (bx, by) = (frame.origin_x, frame.origin_y);
  let (bw, bh) = (frame.layout.size.width, frame.layout.size.height);

  let group = doc.begin_masked_group(&mask_ref)?;
  if background.0[3] != 0 {
    doc.rect(bx, by, bw, bh, background)?;
  }
  if let Some(images) = context.style.background_image.as_deref() {
    LayerEmitter::new(context, doc).background_images(
      images,
      Frame::new(bx, by, bw, bh),
      Frame::new(bx, by, bw, bh),
    )?;
  }
  doc.end_group(group)?;

  // The `color` (brush) fills the glyph interiors on top of the background, with
  // the real text stroke (a transparent stroke adds nothing visible).
  for run in &runs.runs {
    let stroke = run_stroke(&run.glyph_run, font_style);

    emit_run_glyphs(doc, run, font_style, frame, None, stroke, None)?;
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
  color: Rgba,
  stroke_width: f32,
  join: &str,
) -> io::Result<bool> {
  let run_transform = run.transform(IDENTITY);
  let glyph_offset = run.glyph_offset(frame.layout);
  let mut any = false;
  for glyph in &run.glyph_run.glyphs {
    let Some(ResolvedGlyph::Outline(outline)) = run.resolved_glyphs.get(&glyph.id).map(Arc::as_ref)
    else {
      continue;
    };
    let matrix =
      run_transform * Affine::translation(glyph_offset.x + glyph.x, glyph_offset.y + glyph.y);
    let cols = offset(matrix.to_cols_array(), frame.origin_x, frame.origin_y).to_cols_array();
    let data = path_data(outline.paths(), cols);
    if data.is_empty() {
      continue;
    }
    any = true;
    if let Some(embolden) = outline.embolden().filter(|embolden| *embolden > 0.0) {
      doc.glyph_path(&data, color, Some((color, embolden, join)))?;
    }
    if stroke_width > 0.0 {
      doc.glyph_path(&data, color, Some((color, stroke_width, join)))?;
    } else {
      doc.glyph_path(&data, color, None)?;
    }
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
    .then(|| doc.begin_group(IDENTITY, opacity, None, None))
    .transpose()?;
  let mut device = DocumentDevice { doc, error: None };

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
  if let Some(error) = device.error {
    return Err(error);
  }
  if let Some(group) = opacity_group {
    doc.end_group(group)?;
  }
  Ok(())
}

/// Emits a run's glyphs. `color_override` (for shadows) recolors every glyph and
/// suppresses bitmaps/COLR; `stroke` adds `-webkit-text-stroke` to outlines. When
/// `clip_data` is `Some`, outline glyph paths are appended to it (for a
/// `background-clip: text` `<clipPath>`) instead of being painted.
fn emit_run_glyphs(
  doc: &mut SvgDocument,
  run: &PositionedInlineRun,
  font_style: &SizedFontStyle,
  frame: TextFrame,
  color_override: Option<Rgba>,
  stroke: Option<(Rgba, f32, &str)>,
  mut clip_data: Option<&mut String>,
) -> io::Result<()> {
  let run_transform = run.transform(IDENTITY);
  let glyph_offset = run.glyph_offset(frame.layout);
  let fill_color = run.glyph_run.brush.color;
  let bold_join = line_join_str(font_style.parent.stroke_linejoin);

  // Per-run (inline span) opacity, matching the raster backend's
  // `draw_with_inline_opacity`. Skipped while building a clip path (geometry only).
  let opacity = run.glyph_run.brush.opacity;
  let opacity_group = (clip_data.is_none() && opacity < 1.0)
    .then(|| doc.begin_group(IDENTITY, opacity, None, None))
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
    let placed = offset(matrix.to_cols_array(), frame.origin_x, frame.origin_y);
    let cols = placed.to_cols_array();

    match resolved.as_ref() {
      ResolvedGlyph::Outline(outline) => {
        if let Some(clip) = clip_data.as_deref_mut() {
          clip.push_str(&path_data(outline.paths(), cols));
          continue;
        }
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
              flush_glyph_run(doc, &mut uses, fill, stroke)?;
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
          flush_glyph_run(doc, &mut uses, fill, stroke)?;
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
        if color_override.is_some() || clip_data.is_some() {
          continue;
        }
        let Some(png) = bitmap.image.encode_png() else {
          continue;
        };
        flush_glyph_run(doc, &mut uses, fill, stroke)?;
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
  flush_glyph_run(doc, &mut uses, fill, stroke)?;

  if let Some(group) = opacity_group {
    doc.end_group(group)?;
  }
  Ok(())
}

/// Emits the accumulated run of plain outline glyphs as `<use>` references and
/// clears the buffer. A no-op when nothing has accumulated.
fn flush_glyph_run(
  doc: &mut SvgDocument,
  uses: &mut Vec<(u32, f32, f32)>,
  fill: Rgba,
  stroke: Option<(Rgba, f32, &str)>,
) -> io::Result<()> {
  if !uses.is_empty() {
    doc.glyph_uses(uses, fill, stroke)?;
    uses.clear();
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

fn font_error(error: takumi_core::resources::font::FontError) -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    format!("glyph resolution failed: {error}"),
  )
}

/// Offsets a border-box-relative `[a,b,c,d,e,f]` transform to absolute space.
fn offset(transform: [f32; 6], origin_x: f32, origin_y: f32) -> Affine {
  let [a, b, c, d, e, f] = transform;
  Affine {
    a,
    b,
    c,
    d,
    x: e + origin_x,
    y: f + origin_y,
  }
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
