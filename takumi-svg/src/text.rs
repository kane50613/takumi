//! Text node → SVG glyph-outline `<path>` emission.
//!
//! Builds the inline layout with takumi-core's public API (the same call the
//! raster backend uses) and walks the resolved, positioned glyphs, emitting each
//! outline glyph as a real vector `<path>`. All `parley`/`skrifa`/`tiny_skia`
//! usage stays inside takumi-core; this crate only consumes content-box-relative
//! affine transforms and the helper that serializes an outline to path data.

use std::io;

use taffy::{AvailableSpace, Layout, Size};
use takumi_core::context::RenderContext;
use takumi_core::font_style::SizedFontStyle;
use takumi_core::layout::inline::{
  InlineItem, InlineLayoutMode, InlineLayoutRequest, create_inline_layout,
  resolve_inline_max_height, resolve_positioned_glyphs,
};
use takumi_core::layout::node::TextData;
use takumi_core::resources::font::ResolvedGlyph;

use crate::image::encode;
use crate::{Affine, Rgba, SvgDocument};

/// Emits a text node's glyphs. `origin_x`/`origin_y` are the element's absolute
/// border-box top-left; `layout` provides border/padding and content size.
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

  let glyphs = resolve_positioned_glyphs(&built, context, layout).map_err(|error| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      format!("glyph resolution failed: {error}"),
    )
  })?;

  for positioned in glyphs {
    let [a, b, c, d, e, f] = positioned.transform;
    // Offset the border-box-relative transform to the absolute element origin.
    let placed = Affine {
      a,
      b,
      c,
      d,
      x: e + origin_x,
      y: f + origin_y,
    };
    match &positioned.glyph {
      ResolvedGlyph::Outline(outline) => {
        let data = outline.to_svg_path_data(placed.to_cols_array());
        if !data.is_empty() {
          doc.path(&data, Rgba(positioned.color.0))?;
        }
      }
      // Color/bitmap glyphs (emoji) have no vector form — embed the rasterized
      // pixmap as a `data:image/png` `<image>` placed by the glyph transform.
      ResolvedGlyph::Bitmap(bitmap) => {
        let Some(png) = bitmap.to_png() else {
          continue;
        };
        let (width, height) = bitmap.size();
        let matrix = placed
          * Affine::translation(bitmap.placement.left as f32, -(bitmap.placement.top as f32))
          * Affine::scale(bitmap.scale_x, bitmap.scale_y);
        let href = encode("image/png", &png);
        let group = doc.begin_group(matrix, 1.0, None)?;
        doc.image(0.0, 0.0, width as f32, height as f32, &href, None)?;
        doc.end_group(group)?;
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use takumi_core::GlobalContext;
  use takumi_core::layout::Viewport;
  use takumi_core::layout::node::Node;
  use takumi_core::resources::font::FontResource;

  use crate::render::render_svg;

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
    let svg = render_svg(node, Viewport::new((200, 80)), &global).unwrap();
    assert!(svg.contains("<path"), "expected glyph <path> elements");
    assert!(
      !svg.contains("base64"),
      "text must be vector, not embedded bitmap"
    );
  }
}
