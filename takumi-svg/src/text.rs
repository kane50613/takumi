//! Text node → SVG glyph-outline `<path>` emission.
//!
//! Builds the inline layout with takumi-core's public API (the same call the
//! raster backend uses) and walks the resolved, positioned glyphs, emitting each
//! outline glyph as a real vector `<path>`. All `parley`/`skrifa`/`tiny_skia`
//! usage stays inside takumi-core; this crate only consumes content-box-relative
//! affine transforms and the helper that serializes an outline to path data.

use std::io;

use taffy::{AvailableSpace, Size};
use takumi_core::context::RenderContext;
use takumi_core::font_style::SizedFontStyle;
use takumi_core::layout::inline::{
  InlineItem, InlineLayoutMode, InlineLayoutRequest, create_inline_layout,
  resolve_inline_max_height, resolve_positioned_glyphs,
};
use takumi_core::layout::node::TextData;
use takumi_core::resources::font::ResolvedGlyph;

use crate::{Rgba, SvgDocument};

/// Emits a text node's glyphs into the given content-box rectangle.
pub(crate) fn emit_text(
  text: &TextData,
  context: &RenderContext,
  content_x: f32,
  content_y: f32,
  content_w: f32,
  content_h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let font_style = SizedFontStyle::from_style(&context.style, context);
  if font_style.sizing.font_size == 0.0 || content_w <= 0.0 || content_h <= 0.0 {
    return Ok(());
  }

  let max_height = resolve_inline_max_height(&font_style, content_h);
  let built = create_inline_layout(InlineLayoutRequest {
    items: vec![InlineItem::Text {
      text: text.text.as_str().into(),
      context,
    }],
    available_space: Size {
      width: AvailableSpace::Definite(content_w),
      height: AvailableSpace::Definite(content_h),
    },
    max_width: content_w,
    max_height,
    style: &font_style,
    global: context.global,
    mode: InlineLayoutMode::Draw,
  });

  let Ok(glyphs) = resolve_positioned_glyphs(&built, context, content_w) else {
    return Ok(());
  };

  for positioned in glyphs {
    match &positioned.glyph {
      ResolvedGlyph::Outline(outline) => {
        // Offset the content-box-relative transform to the absolute content origin.
        let [a, b, c, d, e, f] = positioned.transform;
        let data = outline.to_svg_path_data([a, b, c, d, e + content_x, f + content_y]);
        if !data.is_empty() {
          doc.path(&data, Rgba(positioned.color.0))?;
        }
      }
      // TODO: bitmap/emoji glyphs (embed the pixmap as a `data:image/png` <image>);
      // requires a core PNG helper to avoid pulling tiny_skia into takumi-svg.
      ResolvedGlyph::Bitmap(_) => {}
    }
  }

  // TODO: decorations (underline/strikethrough), text-shadow, and text stroke.
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
