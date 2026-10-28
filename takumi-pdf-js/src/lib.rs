//! WebAssembly bindings for takumi-pdf.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod date;
mod metadata;
mod options;

use std::{collections::HashMap, fmt::Display, sync::Arc};

use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{
  FontCacheOptions, FontStore, NeedsOutput, append_css,
  input::{Font, decode_images},
  parse_lang,
  source::{read_source, split_emojis},
  source_cache::SourceCache,
  stylesheet,
};
use takumi_core::{
  layout::node::Node,
  resources::image::{ImageSource, ResourceCache},
  style::{FontFamily, Lang},
  viewport::Viewport,
};
use takumi_pdf::{
  Attachment, Band, MeasureOptions, PageOptions, PageRange, PageRules, PdfError, PdfMetadata,
  PdfOptions, PdfStandard, Tagging, UncoveredText,
};
use wasm_bindgen::prelude::*;

use crate::options::{BandInput, PdfRenderOptions, page_background, resolve_geometry};

pub(crate) fn map_error(error: impl Display) -> js_sys::Error {
  js_sys::Error::new(&error.to_string())
}

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = include_str!("./dts-header.d.ts");

#[wasm_bindgen]
extern "C" {
  /// A layout node, the `{ json }` text of one, or HTML markup to parse in its place.
  #[wasm_bindgen(typescript_type = "Node | string | { json: string }")]
  pub type NodeType;

  /// JavaScript type for font input (details object or raw buffer).
  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;

  /// JavaScript object holding the font cache budget.
  #[wasm_bindgen(typescript_type = "FontCacheOptions")]
  pub type FontCacheOptionsType;

  /// JavaScript type for the families produced by `registerFont`.
  #[wasm_bindgen(typescript_type = "RegisteredFamily[]")]
  pub type RegisteredFamiliesType;

  /// JavaScript object representing render options.
  #[wasm_bindgen(typescript_type = "PdfRenderOptions")]
  pub type PdfRenderOptionsType;

  /// JavaScript object representing measure options.
  #[wasm_bindgen(typescript_type = "MeasureOptions")]
  pub type MeasureOptionsType;

  /// JavaScript object representing a measured size.
  #[wasm_bindgen(typescript_type = "MeasuredSize")]
  pub type MeasuredSizeType;

  /// JavaScript object listing what a node tree needs fetched.
  #[wasm_bindgen(typescript_type = "RenderNeeds")]
  pub type RenderNeedsType;
}

/// The size returned by [`PdfRenderer::measure`].
#[derive(serde::Serialize)]
struct MeasuredSizeOutput {
  width: f32,
  height: f32,
}

/// The layout inputs [`PdfRenderer::render`] and [`PdfRenderer::measure`] share.
struct Layout {
  viewport: Option<Viewport>,
  page: Option<PageOptions>,
  images: HashMap<Arc<str>, ImageSource>,
  lang: Option<Lang>,
}

/// A PDF renderer holding registered fonts and a decoded-resource cache.
///
/// Every method takes `&self` and the fonts are copy-on-write, mirroring the
/// other wasm bindings: a panic mid-call can't leave the wasm-bindgen borrow
/// flag or the font store held.
#[wasm_bindgen]
pub struct PdfRenderer {
  fonts: FontStore,
  resource_cache: ResourceCache,
  sources: SourceCache,
}

impl PdfRenderer {
  /// The document `node` describes and the options it renders with, its HTML bands parsed, its
  /// emoji split out, bands included, and the CSS of every `<style>` element appended.
  fn read_document(
    &self,
    node: NodeType,
    options: Option<JsValue>,
  ) -> Result<(Node, PdfRenderOptions), js_sys::Error> {
    let mut options: PdfRenderOptions = options
      .map(|options| from_value(options).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let (mut node, mut document_css) = read_source(node.into(), &self.sources)?;
    let emoji = options.emoji.take();

    split_emojis(&mut node, emoji.as_deref())?;

    for band in options.bands_mut() {
      if let BandInput::Html(html) = band {
        let (parsed, css) = read_source(JsValue::from_str(html), &SourceCache::default())?;

        document_css.extend(css);
        *band = BandInput::Node(Box::new(parsed));
      }

      if let BandInput::Node(node) = band {
        split_emojis(node, emoji.as_deref())?;
      }
    }

    options.css = append_css(options.css.take(), document_css);

    Ok((node, options))
  }

  fn layout(&self, options: &PdfRenderOptions) -> Result<Layout, js_sys::Error> {
    let images = decode_images(
      &self.resource_cache,
      options.images.as_deref().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let (viewport, page) = resolve_geometry(options)?;
    let lang = parse_lang(options.lang.as_deref()).map_err(map_error)?;

    Ok(Layout {
      viewport,
      page,
      images,
      lang,
    })
  }

  /// Calls `run` with the measure options `node` and `options` describe.
  fn with_measure_options<T>(
    &self,
    node: NodeType,
    options: Option<MeasureOptionsType>,
    run: impl FnOnce(MeasureOptions<'_>) -> Result<T, PdfError>,
  ) -> Result<T, js_sys::Error> {
    let (node, options) = self.read_document(node, options.map(Into::into))?;
    let Layout {
      viewport,
      page,
      images,
      lang,
    } = self.layout(&options)?;
    let fonts = self.fonts.for_render(options.font_families.as_deref());

    run(MeasureOptions {
      viewport,
      fonts: &fonts,
      node,
      stylesheet: stylesheet(&self.resource_cache, options.css, Vec::new()).map_err(map_error)?,
      images,
      page,
      font_families: options.font_families.map(FontFamily::from_names),
      lang,
    })
    .map_err(map_error)
  }

  fn load_font(
    &self,
    font: FontType,
    key: Option<&str>,
    pinned: bool,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    let font: Font = from_value(font.into()).map_err(map_error)?;
    let registered = self.fonts.register(font, key, pinned).map_err(map_error)?;

    Ok(to_value(&registered).map_err(map_error)?.unchecked_into())
  }
}

#[wasm_bindgen]
impl PdfRenderer {
  /// Creates a renderer with the bundled last-resort fonts.
  #[wasm_bindgen(constructor)]
  pub fn new(options: Option<FontCacheOptionsType>) -> Result<PdfRenderer, js_sys::Error> {
    let options: FontCacheOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    Ok(PdfRenderer {
      fonts: FontStore::new(options.font_cache_max_bytes).map_err(map_error)?,
      resource_cache: ResourceCache::default(),
      sources: SourceCache::default(),
    })
  }

  /// Registers a font (raw bytes or a details object), returning the families
  /// it produced.
  #[wasm_bindgen(js_name = registerFont)]
  pub fn register_font(
    &self,
    font: FontType,
    key: Option<String>,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    self.load_font(font, key.as_deref(), true)
  }

  /// Registers a font a render's `fonts` list loaded under `key`, released for space once the
  /// renderer's font cache is past its budget.
  #[wasm_bindgen(js_name = cacheFont)]
  pub fn cache_font(
    &self,
    font: FontType,
    key: &str,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    self.load_font(font, Some(key), false)
  }

  /// The families a registration under `key` added, when the renderer still holds them; `pin`
  /// keeps them from being released for space.
  #[wasm_bindgen(js_name = cachedFonts)]
  pub fn cached_fonts(
    &self,
    key: &str,
    pin: bool,
  ) -> Result<Option<RegisteredFamiliesType>, js_sys::Error> {
    self
      .fonts
      .cached(key, pin)
      .map(|registered| Ok(to_value(&registered).map_err(map_error)?.unchecked_into()))
      .transpose()
  }

  /// Removes every face registered under `family`, each `unicodeRange` subset alike, returning
  /// whether it had any.
  #[wasm_bindgen(js_name = unregisterFont)]
  pub fn unregister_font(&self, family: &str) -> bool {
    self.fonts.unregister_family(family)
  }

  /// Removes every registered font, leaving the bundled fallback font.
  #[wasm_bindgen(js_name = clearFonts)]
  pub fn clear_fonts(&self) -> Result<(), js_sys::Error> {
    self.fonts.reset().map_err(map_error)
  }

  /// Renders a node tree to PDF bytes. Without options the output is paged A4;
  /// `viewport` renders a single fixed page instead.
  #[wasm_bindgen(unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render(
    &self,
    node: NodeType,
    options: Option<PdfRenderOptionsType>,
  ) -> Result<Vec<u8>, js_sys::Error> {
    let (node, options) = self.read_document(node, options.map(Into::into))?;
    let Layout {
      viewport,
      page,
      images,
      lang,
    } = self.layout(&options)?;
    let fonts = self.fonts.for_render(options.font_families.as_deref());

    takumi_pdf::render(PdfOptions {
      viewport,
      fonts: &fonts,
      node,
      stylesheet: stylesheet(&self.resource_cache, options.css, Vec::new()).map_err(map_error)?,
      images,
      page,
      page_ranges: options
        .page_ranges
        .map(|ranges| ranges.into_iter().map(PageRange::from).collect()),
      background_color: page_background(options.background_color.as_deref())?,
      header: options.header.map(Band::from).and_then(Band::node),
      footer: options.footer.map(Band::from).and_then(Band::node),
      pages: options.pages.map(PageRules::from).unwrap_or_default(),
      font_families: options.font_families.map(FontFamily::from_names),
      lang,
      metadata: options.metadata.map(PdfMetadata::try_from).transpose()?,
      producer: None,
      outline: options.outline.unwrap_or(false),
      standard: options.pdfa.map(PdfStandard::from).unwrap_or_default(),
      tagged: options.tagged.map(Tagging::from).unwrap_or_default(),
      attachments: options
        .attachments
        .unwrap_or_default()
        .into_iter()
        .map(Attachment::try_from)
        .collect::<Result<_, _>>()?,
      uncovered_text: options
        .uncovered_text
        .map(UncoveredText::from)
        .unwrap_or_default(),
    })
    .map_err(map_error)
  }

  /// Lays out a node tree without rendering and returns its size in CSS px.
  /// Page options lay out at the full page width, like a header/footer band;
  /// `pageNumber` / `totalPages` hooks are filled with three-digit counters.
  #[wasm_bindgen]
  pub fn measure(
    &self,
    node: NodeType,
    options: Option<MeasureOptionsType>,
  ) -> Result<MeasuredSizeType, js_sys::Error> {
    let measured = self.with_measure_options(node, options, takumi_pdf::measure)?;

    Ok(
      to_value(&MeasuredSizeOutput {
        width: measured.width,
        height: measured.height,
      })
      .map_err(map_error)?
      .unchecked_into(),
    )
  }

  /// Reads the images a node tree looks up by URL and the characters its text shapes, after the
  /// cascade, laid out as `measure` lays it out.
  pub fn needs(
    &self,
    node: NodeType,
    options: Option<MeasureOptionsType>,
  ) -> Result<RenderNeedsType, js_sys::Error> {
    let needs = self.with_measure_options(node, options, takumi_pdf::needs)?;

    Ok(
      to_value(&NeedsOutput::from(needs))
        .map_err(map_error)?
        .unchecked_into(),
    )
  }
}
