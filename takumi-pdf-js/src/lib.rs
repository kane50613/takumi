//! WebAssembly bindings for takumi-pdf.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod date;
mod metadata;
mod options;

use std::{collections::HashMap, fmt::Display, iter::once, rc::Rc, sync::Arc};

use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{
  FontCacheOptions, FontStore, NeedsOutput, append_css,
  input::{ListedFonts, decode_images, retain_svg},
  parse_lang,
  source::read_source,
  source_cache::SourceCache,
  stylesheet,
};
use takumi_core::{
  Fonts,
  layout::{needs::RenderNeeds, node::Node},
  resources::image::{ImageSource, ResourceCache},
  style::{FontFamily, Lang, StyleSheet},
  viewport::Viewport,
};
use takumi_pdf::{
  Attachment, Band, MeasureOptions, PageOptions, PageRange, PageRules, PdfMetadata, PdfOptions,
  PdfStandard, Tagging, UncoveredText,
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

  /// JavaScript object listing what a document needs fetched.
  #[wasm_bindgen(typescript_type = "RenderNeeds")]
  pub type RenderNeedsType;
  /// JavaScript list of a render's fonts, described before their bytes arrive.
  #[wasm_bindgen(typescript_type = "ListedFont[]")]
  pub type ListedFontsType;
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

/// What every tree of a document lays out with as [`PdfRenderer::measure`] lays it out.
struct MeasureInputs {
  layout: Layout,
  fonts: Rc<Fonts>,
  stylesheet: Arc<StyleSheet>,
  font_families: Option<FontFamily>,
}

impl MeasureInputs {
  /// The options measuring `node` takes.
  fn options(&self, node: Node) -> MeasureOptions<'_> {
    MeasureOptions {
      viewport: self.layout.viewport,
      fonts: &self.fonts,
      node,
      stylesheet: self.stylesheet.clone(),
      images: self.layout.images.clone(),
      page: self.layout.page,
      font_families: self.font_families.clone(),
      lang: self.layout.lang,
    }
  }
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
  bands: SourceCache,
}

impl PdfRenderer {
  /// The document `node` describes and its options, with HTML bands parsed and `<style>` CSS appended.
  fn read_document(
    &self,
    node: NodeType,
    options: Option<JsValue>,
  ) -> Result<(Node, PdfRenderOptions), js_sys::Error> {
    let mut options: PdfRenderOptions = options
      .map(|options| from_value(options).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let (node, mut document_css) = read_source(node.into(), &self.sources)?;

    for band in options.bands_mut() {
      if let BandInput::Html(html) = band {
        let (parsed, css) = read_source(JsValue::from_str(html), &self.bands)?;

        document_css.extend(css);
        *band = BandInput::Node(Box::new(parsed));
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

  /// What the document's trees lay out with as `measure` lays them out.
  fn measure_inputs(
    &self,
    options: &mut PdfRenderOptions,
    listed: Option<&ListedFonts>,
  ) -> Result<MeasureInputs, js_sys::Error> {
    Ok(MeasureInputs {
      layout: self.layout(options)?,
      fonts: self.fonts.for_needs(&mut options.font_families, listed),
      stylesheet: stylesheet(&self.resource_cache, options.css.take(), Vec::new())
        .map_err(map_error)?,
      font_families: options.font_families.take().map(FontFamily::from_names),
    })
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
      bands: SourceCache::default(),
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
    self.fonts.register_value(font, key.as_deref(), true)
  }

  /// Registers a font a render's `fonts` list loaded under `key`, released for space once the
  /// renderer's font cache is past its budget.
  #[wasm_bindgen(js_name = cacheFont)]
  pub fn cache_font(
    &self,
    font: FontType,
    key: &str,
  ) -> Result<RegisteredFamiliesType, js_sys::Error> {
    self.fonts.register_value(font, Some(key), false)
  }

  /// The families a registration under `key` added, when the renderer still holds them; `pin`
  /// keeps them from being released for space.
  #[wasm_bindgen(js_name = cachedFonts)]
  pub fn cached_fonts(
    &self,
    key: &str,
    pin: bool,
  ) -> Result<Option<RegisteredFamiliesType>, js_sys::Error> {
    self.fonts.cached_value(key, pin)
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
    let (node, mut options) = self.read_document(node, options.map(Into::into))?;
    let inputs = self.measure_inputs(&mut options, None)?;
    let measured = takumi_pdf::measure(inputs.options(node)).map_err(map_error)?;

    Ok(
      to_value(&MeasuredSizeOutput {
        width: measured.width,
        height: measured.height,
      })
      .map_err(map_error)?
      .unchecked_into(),
    )
  }

  /// Reads the images a document and its bands look up and the pending fonts their text loads.
  pub fn needs(
    &self,
    node: NodeType,
    options: Option<PdfRenderOptionsType>,
    fonts: Option<ListedFontsType>,
  ) -> Result<RenderNeedsType, js_sys::Error> {
    let listed = self.fonts.listed(fonts.map(Into::into))?;
    let (node, mut options) = self.read_document(node, options.map(Into::into))?;

    retain_svg(&mut options.images);

    let inputs = self.measure_inputs(&mut options, listed.as_ref())?;
    let bands = options
      .bands_mut()
      .filter_map(|band| band.node().cloned())
      .collect::<Vec<_>>();
    let needs = once(node)
      .chain(bands)
      .map(|tree| takumi_pdf::needs(inputs.options(tree)))
      .collect::<Result<RenderNeeds, _>>()
      .map_err(map_error)?;

    Ok(
      to_value(&NeedsOutput::new(needs, listed.as_ref()))
        .map_err(map_error)?
        .unchecked_into(),
    )
  }
}
