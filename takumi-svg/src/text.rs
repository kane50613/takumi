//! Text / inline-content emission: lays out the inline items, lets takumi-core's text painter
//! order the shadows, decorations, and outlines, and draws each run's glyphs as outline `<path>`s,
//! COLR colour layers, or bitmap `<image>`s.

use std::{io, sync::Arc};

use takumi_core::{
  context::RenderContext,
  font_style::SizedFontStyle,
  layout::{
    inline::{
      InlineLayoutMode, InlineLayoutRequest, InlinePass, PositionedInlineRun, ProcessedInlineSpan,
      ShapedRun, create_inline_layout,
    },
    tree::RenderNode,
  },
  painter::{BoxBackground, BoxFrame, GlyphFill, OwnContent},
  path_data::path_data,
  resources::{font::FontError, glyph::ResolvedGlyph, image::to_data_url},
  style::{Affine, BackgroundClip, LineJoin},
};

use crate::{
  Frame, GlyphStroke, Rgba, SvgDocument,
  gradient::LayerEmitter,
  render::{DocumentDevice, emit_inline_box},
};

/// Emits a node's inline content at its frame: lays out its inline items in the content box,
/// paints the runs, then recurses into each positioned inline box.
pub(crate) fn emit_inline_content(
  node: &RenderNode,
  frame: BoxFrame,
  pass: InlinePass,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let context = &node.context;
  let font_style = SizedFontStyle::from_style(&context.style, context);
  let Some(items) = OwnContent::of(node).inline_items(&font_style) else {
    return Ok(());
  };
  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    items,
    frame.layout.unsnapped_content,
    &font_style,
    context,
    InlineLayoutMode::Draw,
  ));

  let runs = built
    .resolve_runs(context, frame.layout)
    .map_err(font_error)?;
  let fill = if context.style.background_clip == BackgroundClip::Text {
    GlyphFill::Background
  } else {
    GlyphFill::Text
  };

  if pass == InlinePass::Content {
    DocumentDevice::paint_text(doc, context, |device| {
      runs.paint(&built.spans, &font_style, fill, frame, device);
    })?;
  }

  for inline_box in runs
    .inline_boxes
    .iter()
    .filter(|inline_box| pass.paints(inline_box))
  {
    if let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(inline_box.id as usize) {
      emit_inline_box(inline_box, item, frame, doc)?;
    }
  }
  Ok(())
}

/// The `-webkit-text-stroke` a run carries. A span may set it for itself, so it
/// comes off the run; the join is a box-level property and stays with the node.
pub(crate) fn run_stroke(run: &ShapedRun, font_style: &SizedFontStyle) -> Option<GlyphStroke> {
  let brush = &run.brush;

  (brush.stroke_width > 0.0 && brush.stroke_color.0[3] != 0).then_some(GlyphStroke {
    color: Rgba(brush.stroke_color.0),
    width: brush.stroke_width,
    join: font_style.parent.stroke_linejoin,
  })
}

/// A background `background-clip: text` shows through glyphs: a box's, laid over `area`.
pub(crate) struct ClipTextBackground<'b> {
  pub(crate) context: &'b RenderContext,
  pub(crate) background: &'b BoxBackground<'b>,
  pub(crate) area: BoxFrame,
}

/// Emits `fill` through one run's glyphs in the block at `frame` (`background-clip: text`),
/// widened by any `-webkit-text-stroke`, for the run's own paint to cover.
pub(crate) fn emit_clip_text_run(
  run: &PositionedInlineRun,
  font_style: &SizedFontStyle,
  frame: BoxFrame,
  fill: &ClipTextBackground<'_>,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let (mask_token, mask_ref) = doc.begin_mask()?;
  let any = emit_clip_text_mask_glyphs(run, frame, font_style.parent.stroke_linejoin, doc)?;

  doc.end_mask(mask_token)?;

  if !any {
    return Ok(());
  }

  let ClipTextBackground {
    context,
    background,
    area,
  } = *fill;
  let border_box = Frame::border_box(area);
  let group = doc.begin_masked_group(&mask_ref)?;

  if let Some(color) = background.color {
    doc.rect(border_box, Rgba(color.0))?;
  }
  LayerEmitter::new(context, doc).layers(
    &background.layers,
    Frame::origin_box(area, background.origin),
    border_box,
  )?;
  doc.end_group(group)
}

/// Paints a run's outline glyphs white into the active mask with both fill and
/// stroke (and any faux-bold embolden), so the mask covers the full fill+stroke
/// glyph coverage. Returns whether any glyph was emitted.
fn emit_clip_text_mask_glyphs(
  run: &PositionedInlineRun,
  frame: BoxFrame,
  join: LineJoin,
  doc: &mut SvgDocument,
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
    let data = path_data(outline.paths(), frame.place(matrix));
    if data.is_empty() {
      continue;
    }
    any = true;
    if let Some(embolden) = outline.embolden().filter(|embolden| *embolden > 0.0) {
      let bold = GlyphStroke {
        color: Rgba::WHITE,
        width: embolden,
        join,
      };

      doc.glyph_path(&data, Rgba::WHITE, Some(bold))?;
    }
    let stroke = (stroke_width > 0.0).then_some(GlyphStroke {
      color: Rgba::WHITE,
      width: stroke_width,
      join,
    });

    doc.glyph_path(&data, Rgba::WHITE, stroke)?;
  }
  Ok(any)
}

/// Emits a run's glyphs. `color_override` (for shadows) recolors every glyph and
/// suppresses bitmaps/COLR; `stroke` adds `-webkit-text-stroke` to outlines.
pub(crate) fn emit_run_glyphs(
  run: &PositionedInlineRun,
  font_style: &SizedFontStyle,
  frame: BoxFrame,
  color_override: Option<Rgba>,
  stroke: Option<GlyphStroke>,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let run_transform = run.transform(Affine::IDENTITY);
  let glyph_offset = run.glyph_offset(frame.layout);
  let fill_color = run.glyph_run.brush.color;
  let bold_join = font_style.parent.stroke_linejoin;

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
              let data = path_data(outline.paths(), placed);
              if data.is_empty() {
                continue;
              }
              doc.flush_glyph_uses(&mut uses, fill, stroke)?;
              let bold = GlyphStroke {
                color: fill,
                width: embolden,
                join: bold_join,
              };

              doc.glyph_path(&data, fill, Some(bold))?;
              if let Some(text_stroke) = stroke {
                doc.glyph_path(&data, Rgba::TRANSPARENT, Some(text_stroke))?;
              }
            }
            None => {
              let data = path_data(
                outline.paths(),
                Affine {
                  x: 0.0,
                  y: 0.0,
                  ..placed
                },
              );

              if !data.is_empty() {
                uses.push((doc.glyph_ref(data), placed.x, placed.y));
              }
            }
          }
        } else {
          doc.flush_glyph_uses(&mut uses, fill, stroke)?;
          for (color, paths) in color_layers {
            if color.0[3] == 0 {
              continue;
            }
            let data = path_data(paths, placed);
            if !data.is_empty() {
              doc.glyph_path(&data, Rgba(color.0), None)?;
            }
          }
        }
      }
      // Color/bitmap glyphs (emoji) have no vector form, so embed the rasterized
      // pixmap as a `data:image/png` `<image>`, as a silhouette in the shadow pass.
      ResolvedGlyph::Bitmap(bitmap) => {
        let Some(png) = bitmap.image.encode_png() else {
          continue;
        };
        doc.flush_glyph_uses(&mut uses, fill, stroke)?;
        let (width, height) = (bitmap.image.width(), bitmap.image.height());
        let bitmap_matrix = placed * bitmap.image_transform();
        let href = to_data_url("image/png", &png);
        let silhouette = color_override
          .map(|color| doc.silhouette_filter(color))
          .transpose()?;
        let group = doc.begin_group(bitmap_matrix, 1.0, None, silhouette.as_deref())?;
        doc.image(
          Frame::new(0.0, 0.0, width as f32, height as f32),
          &href,
          None,
        )?;
        doc.end_group(group)?;
      }
    }
  }
  doc.flush_glyph_uses(&mut uses, fill, stroke)
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
