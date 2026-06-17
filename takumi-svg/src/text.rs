//! Text / inline-content emission.
//!
//! Builds the one shared inline enumeration ([`resolve_inline_runs`], the same
//! producer the raster backend consumes) and emits each run's glyphs (outline
//! `<path>`, COLR color layers, or bitmap `<image>`), text decorations,
//! text-shadows, and `-webkit-text-stroke`. The layout/positioning is shared with
//! raster; only the painting differs.

use std::io;

use taffy::{AvailableSpace, Layout, Size};
use takumi_base::context::RenderContext;
use takumi_base::font_style::SizedFontStyle;
use takumi_base::layout::inline::{
  DecorationRect, InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineOutlineRect,
  InlineRunLayout, PositionedInlineRun, ProcessedInlineSpan, collect_inline_items,
  create_inline_layout, outline_island_contour, outline_islands, resolve_inline_max_height,
  resolve_inline_runs, run_decorations,
};
use takumi_base::layout::node::TextData;
use takumi_base::layout::style::{BackgroundClip, BorderStyle, LineJoin};
use takumi_base::layout::tree::RenderNode;
use takumi_base::resources::font::ResolvedGlyph;

use crate::box_model::path_data;
use crate::gradient::emit_background_images;
use crate::image::encode;
use crate::render::emit_inline_box;
use crate::{Affine, IDENTITY, Num, Rgba, SvgDocument};

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

  let built = create_inline_layout(InlineLayoutRequest {
    items: vec![InlineItem::Text {
      text: text.text.as_str().into(),
      context,
    }],
    available_space: Size {
      width: AvailableSpace::Definite(content.width),
      height: AvailableSpace::Definite(content.height),
    },
    max_width: content.width,
    max_height: resolve_inline_max_height(&font_style, content.height),
    style: &font_style,
    fonts: context.fonts,
    mode: InlineLayoutMode::Draw,
  });

  let runs = resolve_inline_runs(&built, context, layout).map_err(font_error)?;
  emit_runs(
    doc,
    &runs,
    &built.spans,
    &font_style,
    context,
    layout,
    origin_x,
    origin_y,
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

  let built = create_inline_layout(InlineLayoutRequest {
    items: collect_inline_items(node),
    available_space: Size {
      width: AvailableSpace::Definite(content.width),
      height: AvailableSpace::Definite(content.height),
    },
    max_width: content.width,
    max_height: resolve_inline_max_height(&font_style, content.height),
    style: &font_style,
    fonts: context.fonts,
    mode: InlineLayoutMode::Draw,
  });

  let runs = resolve_inline_runs(&built, context, layout).map_err(font_error)?;
  emit_runs(
    doc,
    &runs,
    &built.spans,
    &font_style,
    context,
    layout,
    origin_x,
    origin_y,
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
#[allow(clippy::too_many_arguments)]
fn emit_runs(
  doc: &mut SvgDocument,
  runs: &InlineRunLayout<'_>,
  spans: &[ProcessedInlineSpan<'_, '_>],
  font_style: &SizedFontStyle,
  context: &RenderContext,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
) -> io::Result<()> {
  let stroke =
    (font_style.stroke_width > 0.0 && font_style.text_stroke_color.0[3] != 0).then_some((
      Rgba(font_style.text_stroke_color.0),
      font_style.stroke_width,
      line_join_str(font_style.parent.stroke_linejoin),
    ));

  // text-shadow paints below the glyphs; later-listed shadows paint lowest.
  for shadow in font_style.text_shadow.iter().rev() {
    let color = Rgba(shadow.color.0);
    if color.0[3] == 0 {
      continue;
    }
    let filter = if shadow.blur_radius > 0.0 {
      Some(doc.blur_filter(shadow.blur_radius / 2.0)?)
    } else {
      None
    };
    let group = doc.begin_group(IDENTITY, 1.0, None, filter.as_deref())?;
    for run in &runs.runs {
      emit_run_glyphs(
        doc,
        run,
        font_style,
        layout,
        origin_x + shadow.offset_x,
        origin_y + shadow.offset_y,
        Some(color),
        None,
        None,
      )?;
    }
    doc.end_group(group)?;
  }

  for run in &runs.runs {
    emit_run_decorations(doc, run, layout, origin_x, origin_y, false)?;
  }

  if context.style.background_clip == BackgroundClip::Text {
    emit_clip_text_glyphs(
      doc, runs, font_style, context, layout, origin_x, origin_y, stroke,
    )?;
  } else {
    for run in &runs.runs {
      emit_run_glyphs(
        doc, run, font_style, layout, origin_x, origin_y, None, stroke, None,
      )?;
    }
  }

  // Text outlines stroke between the glyphs and the line-through, matching the
  // raster backend's painting order.
  emit_inline_outlines(doc, runs, spans, origin_x, origin_y)?;

  for run in &runs.runs {
    emit_run_decorations(doc, run, layout, origin_x, origin_y, true)?;
  }
  Ok(())
}

/// Strokes the shared inline outline contours ([`outline_islands`]) for each
/// styled span, mirroring the raster backend's merged-island outlines.
fn emit_inline_outlines(
  doc: &mut SvgDocument,
  runs: &InlineRunLayout<'_>,
  spans: &[ProcessedInlineSpan<'_, '_>],
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
  spans: &[ProcessedInlineSpan<'_, '_>],
  origin_x: f32,
  origin_y: f32,
) -> io::Result<()> {
  let Some(first_rect) = island.first() else {
    return Ok(());
  };
  let Some(ProcessedInlineSpan::Text { style, .. }) = spans.get(first_rect.span_id as usize) else {
    return Ok(());
  };
  let width = style.outline_width;
  if width <= 0.0 || !style.outline_style.is_rendered() || style.outline_color.0[3] == 0 {
    return Ok(());
  }

  // Dash geometry mirrors the raster stroke; non-stroked styles (double, the
  // 3D bevels) paint nothing here, as in the raster backend.
  let (dasharray, linecap) = match style.outline_style {
    BorderStyle::Dotted => (Some(format!("0 {}", Num(width * 2.0))), Some("round")),
    BorderStyle::Dashed => (
      Some(format!("{} {}", Num(width * 3.0), Num(width * 2.0))),
      None,
    ),
    BorderStyle::Hidden
    | BorderStyle::Double
    | BorderStyle::Groove
    | BorderStyle::Ridge
    | BorderStyle::Inset
    | BorderStyle::Outset => return Ok(()),
    _ => (None, None),
  };

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
    Rgba(style.outline_color.0),
    width,
    dasharray.as_deref(),
    linecap,
  )?;
  if let Some(group) = group {
    doc.end_group(group)?;
  }
  Ok(())
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
#[allow(clippy::too_many_arguments)]
fn emit_clip_text_glyphs(
  doc: &mut SvgDocument,
  runs: &InlineRunLayout<'_>,
  font_style: &SizedFontStyle,
  context: &RenderContext,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
  stroke: Option<(Rgba, f32, &str)>,
) -> io::Result<()> {
  let white = Rgba([255, 255, 255, 255]);
  let join = line_join_str(font_style.parent.stroke_linejoin);
  let stroke_width = font_style.stroke_width;

  let (mask_token, mask_ref) = doc.begin_mask()?;
  let mut any = false;
  for run in &runs.runs {
    any |= emit_clip_text_mask_glyphs(
      doc,
      run,
      layout,
      origin_x,
      origin_y,
      white,
      stroke_width,
      join,
    )?;
  }
  doc.end_mask(mask_token)?;
  if !any {
    return Ok(());
  }

  let cc = context.current_color;
  let background = Rgba(context.style.background_color.resolve(cc).0);
  let (bx, by) = (origin_x, origin_y);
  let (bw, bh) = (layout.size.width, layout.size.height);

  let group = doc.begin_masked_group(&mask_ref)?;
  if background.0[3] != 0 {
    doc.rect(bx, by, bw, bh, background)?;
  }
  if let Some(images) = context.style.background_image.as_deref() {
    emit_background_images(images, context, bx, by, bw, bh, doc)?;
  }
  doc.end_group(group)?;

  // The `color` (brush) fills the glyph interiors on top of the background, with
  // the real text stroke (a transparent stroke adds nothing visible).
  for run in &runs.runs {
    emit_run_glyphs(
      doc, run, font_style, layout, origin_x, origin_y, None, stroke, None,
    )?;
  }
  Ok(())
}

/// Paints a run's outline glyphs white into the active mask with both fill and
/// stroke (and any faux-bold embolden), so the mask covers the full fill+stroke
/// glyph coverage. Returns whether any glyph was emitted.
#[allow(clippy::too_many_arguments)]
fn emit_clip_text_mask_glyphs(
  doc: &mut SvgDocument,
  run: &PositionedInlineRun<'_>,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
  color: Rgba,
  stroke_width: f32,
  join: &str,
) -> io::Result<bool> {
  let run_transform = run.transform(IDENTITY);
  let glyph_offset = run.glyph_offset(layout);
  let mut any = false;
  for glyph in run.glyph_run.positioned_glyphs() {
    let Some(ResolvedGlyph::Outline(outline)) = run.resolved_glyphs.get(&glyph.id) else {
      continue;
    };
    let matrix =
      run_transform * Affine::translation(glyph_offset.x + glyph.x, glyph_offset.y + glyph.y);
    let cols = offset(matrix.to_cols_array(), origin_x, origin_y).to_cols_array();
    let data = path_data(outline.paths(), cols);
    if data.is_empty() {
      continue;
    }
    any = true;
    if let Some(embolden) = outline.embolden().filter(|embolden| *embolden > 0.0) {
      doc.glyph_path(&data, color, Some((color, embolden * 2.0, join)))?;
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
  run: &PositionedInlineRun<'_>,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
  over: bool,
) -> io::Result<()> {
  let transform = run.transform(IDENTITY);
  let opacity = run.glyph_run.style().brush.opacity;
  let opacity_group = (opacity < 1.0)
    .then(|| doc.begin_group(IDENTITY, opacity, None, None))
    .transpose()?;
  for decoration in run_decorations(&run.glyph_run, layout, run.baseline_shift, transform) {
    if decoration.over == over {
      emit_decoration(doc, &decoration, origin_x, origin_y)?;
    }
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
#[allow(clippy::too_many_arguments)]
fn emit_run_glyphs(
  doc: &mut SvgDocument,
  run: &PositionedInlineRun<'_>,
  font_style: &SizedFontStyle,
  layout: Layout,
  origin_x: f32,
  origin_y: f32,
  color_override: Option<Rgba>,
  stroke: Option<(Rgba, f32, &str)>,
  mut clip_data: Option<&mut String>,
) -> io::Result<()> {
  let run_transform = run.transform(IDENTITY);
  let glyph_offset = run.glyph_offset(layout);
  let fill_color = run.glyph_run.style().brush.color;
  let bold_join = line_join_str(font_style.parent.stroke_linejoin);

  // Per-run (inline span) opacity, matching the raster backend's
  // `draw_with_inline_opacity`. Skipped while building a clip path (geometry only).
  let opacity = run.glyph_run.style().brush.opacity;
  let opacity_group = (clip_data.is_none() && opacity < 1.0)
    .then(|| doc.begin_group(IDENTITY, opacity, None, None))
    .transpose()?;

  // Plain outline glyphs in a run share one fill (and one `-webkit-text-stroke`)
  // and use nonzero winding, so their `d` data is concatenated into a single
  // `<path>` instead of one element per glyph. Faux-bold, COLR layers, and bitmap
  // glyphs need their own paint, so the merged run is flushed before each.
  let fill = color_override.unwrap_or(Rgba(fill_color.0));
  let mut merged = String::new();

  for glyph in run.glyph_run.positioned_glyphs() {
    let Some(resolved) = run.resolved_glyphs.get(&glyph.id) else {
      continue;
    };
    let matrix =
      run_transform * Affine::translation(glyph_offset.x + glyph.x, glyph_offset.y + glyph.y);
    let placed = offset(matrix.to_cols_array(), origin_x, origin_y);
    let cols = placed.to_cols_array();

    match resolved {
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
          let data = path_data(outline.paths(), cols);
          if data.is_empty() {
            continue;
          }
          // Synthesized (faux) bold: the raster backend strokes the glyph with
          // its own fill color (`outline.embolden()`); mirror that here.
          match outline.embolden().filter(|embolden| *embolden > 0.0) {
            Some(embolden) => {
              flush_glyph_run(doc, &mut merged, fill, stroke)?;
              doc.glyph_path(&data, fill, Some((fill, embolden * 2.0, bold_join)))?;
              if let Some(text_stroke) = stroke {
                doc.glyph_path(&data, Rgba([0, 0, 0, 0]), Some(text_stroke))?;
              }
            }
            None => merged.push_str(&data),
          }
        } else {
          flush_glyph_run(doc, &mut merged, fill, stroke)?;
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
        let Ok(png) = bitmap.pixmap.encode_png() else {
          continue;
        };
        flush_glyph_run(doc, &mut merged, fill, stroke)?;
        let (width, height) = (bitmap.pixmap.width(), bitmap.pixmap.height());
        let bitmap_matrix = placed
          * Affine::translation(bitmap.placement.left as f32, -(bitmap.placement.top as f32))
          * Affine::scale(bitmap.scale_x, bitmap.scale_y);
        let href = encode("image/png", &png);
        let group = doc.begin_group(bitmap_matrix, 1.0, None, None)?;
        doc.image(0.0, 0.0, width as f32, height as f32, &href, None)?;
        doc.end_group(group)?;
      }
    }
  }
  flush_glyph_run(doc, &mut merged, fill, stroke)?;

  if let Some(group) = opacity_group {
    doc.end_group(group)?;
  }
  Ok(())
}

/// Emits the accumulated run of plain outline glyphs as one `<path>` and clears
/// the buffer. A no-op when nothing has accumulated.
fn flush_glyph_run(
  doc: &mut SvgDocument,
  merged: &mut String,
  fill: Rgba,
  stroke: Option<(Rgba, f32, &str)>,
) -> io::Result<()> {
  if !merged.is_empty() {
    doc.glyph_path(merged, fill, stroke)?;
    merged.clear();
  }
  Ok(())
}

fn emit_decoration(
  doc: &mut SvgDocument,
  decoration: &DecorationRect,
  origin_x: f32,
  origin_y: f32,
) -> io::Result<()> {
  let matrix = offset(decoration.transform, origin_x, origin_y);
  let group = doc.begin_group(matrix, 1.0, None, None)?;
  doc.rect(
    0.0,
    0.0,
    decoration.width,
    decoration.height,
    Rgba(decoration.color.0),
  )?;
  doc.end_group(group)
}

fn line_join_str(join: LineJoin) -> &'static str {
  match join {
    LineJoin::Miter => "miter",
    LineJoin::Round => "round",
    LineJoin::Bevel => "bevel",
  }
}

fn font_error(error: takumi_base::resources::font::FontError) -> io::Error {
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

  use takumi_base::Fonts;
  use takumi_base::layout::Viewport;
  use takumi_base::layout::node::Node;
  use takumi_base::resources::font::FontResource;

  use crate::render::{SvgOptions, render};

  /// Registers the raw-TTF test font as a fallback for all scripts so the
  /// default font-family resolves to it (no `woff2` feature required).
  fn font_context_with_font() -> Fonts {
    let mut fonts = Fonts::default();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");
    let data = std::fs::read(&path).expect("read test font");
    fonts
      .load_and_store(FontResource::new(data))
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
