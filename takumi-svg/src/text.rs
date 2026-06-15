//! Text node → SVG emission.
//!
//! Builds the inline layout and the backend-agnostic [`TextScene`] with
//! takumi-core's public API (the same layout/positioning the raster backend
//! uses), then emits: text-shadows, under/overline decorations, glyph fills
//! (outline `<path>` or bitmap `<image>` for emoji) with optional
//! `-webkit-text-stroke`, and line-through. All `parley`/`skrifa`/`tiny_skia`
//! usage stays inside takumi-core.

use std::io;

use taffy::{AvailableSpace, Layout, Size};
use takumi_core::context::RenderContext;
use takumi_core::font_style::SizedFontStyle;
use takumi_core::layout::inline::{
  DecorationRect, InlineItem, InlineLayoutMode, InlineLayoutRequest, TextScene,
  create_inline_layout, resolve_inline_max_height, resolve_text_scene,
};
use takumi_core::layout::node::TextData;
use takumi_core::resources::font::ResolvedGlyph;
use tiny_skia::{PathSegment, Point};

use crate::image::encode;
use crate::{Affine, IDENTITY, Rgba, SvgDocument};

/// Emits a text node. `origin_x`/`origin_y` are the element's absolute border-box
/// top-left; `layout` provides border/padding and content size.
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

  let max_height = resolve_inline_max_height(&font_style, content.height);
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
    max_height,
    style: &font_style,
    global: context.global,
    mode: InlineLayoutMode::Draw,
  });

  let scene = resolve_text_scene(&built, context, layout).map_err(|error| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      format!("glyph resolution failed: {error}"),
    )
  })?;

  let stroke =
    (font_style.stroke_width > 0.0 && font_style.text_stroke_color.0[3] != 0).then_some((
      Rgba(font_style.text_stroke_color.0),
      font_style.stroke_width,
    ));

  // text-shadow paints below the text; later-listed shadows paint lowest.
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
    emit_scene(
      doc,
      &scene,
      origin_x + shadow.offset_x,
      origin_y + shadow.offset_y,
      Some(color),
      None,
    )?;
    doc.end_group(group)?;
  }

  emit_scene(doc, &scene, origin_x, origin_y, None, stroke)
}

/// Emits the scene at the given origin. `color_override` (for shadows) recolors
/// everything; `stroke` adds `-webkit-text-stroke` to glyph outlines.
fn emit_scene(
  doc: &mut SvgDocument,
  scene: &TextScene,
  origin_x: f32,
  origin_y: f32,
  color_override: Option<Rgba>,
  stroke: Option<(Rgba, f32)>,
) -> io::Result<()> {
  for decoration in scene.decorations.iter().filter(|d| !d.over) {
    emit_decoration(doc, decoration, origin_x, origin_y, color_override)?;
  }

  for positioned in &scene.glyphs {
    let placed = offset(positioned.transform, origin_x, origin_y);
    match &positioned.glyph {
      ResolvedGlyph::Outline(outline) => {
        let data = outline_to_path_data(outline.paths(), placed.to_cols_array());
        if !data.is_empty() {
          let fill = color_override.unwrap_or(Rgba(positioned.color.0));
          doc.glyph_path(&data, fill, stroke)?;
        }
      }
      // Color/bitmap glyphs (emoji) have no vector form — embed the rasterized
      // pixmap as a `data:image/png` `<image>`. Skipped in the shadow pass.
      ResolvedGlyph::Bitmap(bitmap) => {
        if color_override.is_some() {
          continue;
        }
        let Ok(png) = bitmap.pixmap.encode_png() else {
          continue;
        };
        let (width, height) = (bitmap.pixmap.width(), bitmap.pixmap.height());
        let matrix = placed
          * Affine::translation(bitmap.placement.left as f32, -(bitmap.placement.top as f32))
          * Affine::scale(bitmap.scale_x, bitmap.scale_y);
        let href = encode("image/png", &png);
        let group = doc.begin_group(matrix, 1.0, None, None)?;
        doc.image(0.0, 0.0, width as f32, height as f32, &href, None)?;
        doc.end_group(group)?;
      }
    }
  }

  for decoration in scene.decorations.iter().filter(|d| d.over) {
    emit_decoration(doc, decoration, origin_x, origin_y, color_override)?;
  }

  Ok(())
}

fn emit_decoration(
  doc: &mut SvgDocument,
  decoration: &DecorationRect,
  origin_x: f32,
  origin_y: f32,
  color_override: Option<Rgba>,
) -> io::Result<()> {
  let matrix = offset(decoration.transform, origin_x, origin_y);
  let color = color_override.unwrap_or(Rgba(decoration.color.0));
  let group = doc.begin_group(matrix, 1.0, None, None)?;
  doc.rect(0.0, 0.0, decoration.width, decoration.height, color)?;
  doc.end_group(group)
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

/// Serializes a glyph outline as SVG path `d` data, applying `transform`
/// (`[a, b, c, d, e, f]`, SVG `matrix` order) to every point. Points are already
/// in pixel space, y-down.
fn outline_to_path_data(paths: &[PathSegment], [a, b, c, d, e, f]: [f32; 6]) -> String {
  use std::fmt::Write as _;

  let mut out = String::new();
  let map = |p: Point| (a * p.x + c * p.y + e, b * p.x + d * p.y + f);
  for command in paths {
    match command {
      PathSegment::MoveTo(p) => {
        let (x, y) = map(*p);
        let _ = write!(out, "M{x} {y}");
      }
      PathSegment::LineTo(p) => {
        let (x, y) = map(*p);
        let _ = write!(out, "L{x} {y}");
      }
      PathSegment::QuadTo(c0, p) => {
        let (x0, y0) = map(*c0);
        let (x, y) = map(*p);
        let _ = write!(out, "Q{x0} {y0} {x} {y}");
      }
      PathSegment::CubicTo(c0, c1, p) => {
        let (x0, y0) = map(*c0);
        let (x1, y1) = map(*c1);
        let (x, y) = map(*p);
        let _ = write!(out, "C{x0} {y0} {x1} {y1} {x} {y}");
      }
      PathSegment::Close => out.push('Z'),
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use takumi_core::GlobalContext;
  use takumi_core::layout::Viewport;
  use takumi_core::layout::node::Node;
  use takumi_core::resources::font::FontResource;

  use crate::render::{SvgOptions, render};

  /// Registers the raw-TTF test font as a fallback for all scripts so the
  /// default font-family resolves to it (no `woff2` feature required).
  fn global_with_font() -> GlobalContext {
    let mut global = GlobalContext::default();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");
    let data = std::fs::read(&path).expect("read test font");
    global
      .font_context
      .load_and_store(FontResource::new(data))
      .expect("load test font");
    global
  }

  #[test]
  fn text_renders_glyph_paths_not_bitmap() {
    let global = global_with_font();
    let node = Node::text("Hi".to_string());
    let svg = render(
      SvgOptions::builder()
        .node(node)
        .viewport(Viewport::new((200, 80)))
        .global(&global)
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
