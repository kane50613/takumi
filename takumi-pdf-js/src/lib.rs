//! WebAssembly bindings for takumi-pdf.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod date;
mod metadata;
mod options;

use std::{
  collections::HashMap,
  fmt::Display,
  sync::{Arc, RwLock, RwLockReadGuard},
};

use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{
  default_fonts,
  input::{Font, decode_images, register_font},
  stylesheet,
};
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::image::{ImageSource, ResourceCache},
  style::{FontFamily, Lang},
  viewport::Viewport,
};
use takumi_pdf::{
  Attachment, Band, MeasureOptions, PageOptions, PageRange, PageRules, PdfMetadata, PdfOptions,
  PdfStandard, Tagging, UncoveredText,
};
use wasm_bindgen::prelude::*;

use crate::options::{PdfRenderOptions, page_background, resolve_geometry};

pub(crate) fn map_error(error: impl Display) -> js_sys::Error {
  js_sys::Error::new(&error.to_string())
}

fn locked_error(error: impl Display) -> js_sys::Error {
  js_sys::Error::new(&format!("Renderer state is locked: {error}"))
}

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = include_str!("./dts-header.d.ts");

#[wasm_bindgen]
extern "C" {
  /// JavaScript object representing a layout node.
  #[wasm_bindgen(typescript_type = "Node")]
  pub type NodeType;

  /// JavaScript type for font input (details object or raw buffer).
  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;

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
}

/// The size returned by [`PdfRenderer::measure`].
#[derive(serde::Serialize)]
struct MeasuredSizeOutput {
  width: f32,
  height: f32,
}

/// The characters the page counters named by these class names can produce.
///
/// A counter's characters appear nowhere in the document, so a caller choosing
/// which faces to load has no other way to learn it needs, say, Thai digits.
#[wasm_bindgen(js_name = counterCharacters)]
pub fn counter_characters(classes: Vec<String>) -> String {
  takumi_pdf::counter_characters(classes.iter().map(String::as_str))
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
/// State lives behind a lock and every method takes `&self`, mirroring the
/// other wasm bindings: a panic mid-call can't leave the wasm-bindgen borrow
/// flag permanently set.
#[wasm_bindgen]
pub struct PdfRenderer {
  state: RwLock<Fonts>,
  resource_cache: ResourceCache,
}

impl PdfRenderer {
  fn layout(&self, options: &PdfRenderOptions) -> Result<Layout, js_sys::Error> {
    let images = decode_images(
      &self.resource_cache,
      options.images.as_deref().unwrap_or_default(),
    )
    .map_err(map_error)?;
    let (viewport, page) = resolve_geometry(options)?;
    let lang = options
      .lang
      .as_deref()
      .map(Lang::parse)
      .transpose()
      .map_err(map_error)?;

    Ok(Layout {
      viewport,
      page,
      images,
      lang,
    })
  }

  fn fonts(&self) -> Result<RwLockReadGuard<'_, Fonts>, js_sys::Error> {
    self.state.try_read().map_err(locked_error)
  }
}

#[wasm_bindgen]
impl PdfRenderer {
  /// Creates a renderer with the bundled last-resort fonts.
  #[wasm_bindgen(constructor)]
  pub fn new() -> Result<PdfRenderer, js_sys::Error> {
    Ok(PdfRenderer {
      state: RwLock::new(default_fonts().map_err(map_error)?),
      resource_cache: ResourceCache::default(),
    })
  }

  /// Registers a font (raw bytes or a details object), returning the families
  /// it produced.
  #[wasm_bindgen(js_name = registerFont)]
  pub fn register_font(&self, font: FontType) -> Result<RegisteredFamiliesType, js_sys::Error> {
    let font: Font = from_value(font.into()).map_err(map_error)?;
    let mut state = self.state.try_write().map_err(locked_error)?;

    let registered = register_font(&mut state, font).map_err(map_error)?;
    Ok(to_value(&registered).map_err(map_error)?.unchecked_into())
  }

  /// Renders a node tree to PDF bytes. Without options the output is paged A4;
  /// `viewport` renders a single fixed page instead.
  #[wasm_bindgen(unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render(
    &self,
    node: NodeType,
    options: Option<PdfRenderOptionsType>,
  ) -> Result<Vec<u8>, js_sys::Error> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: PdfRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let Layout {
      viewport,
      page,
      images,
      lang,
    } = self.layout(&options)?;
    let state = self.fonts()?;

    takumi_pdf::render(PdfOptions {
      viewport,
      fonts: &state,
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
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: PdfRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let Layout {
      viewport,
      page,
      images,
      lang,
    } = self.layout(&options)?;
    let state = self.fonts()?;
    let measured = takumi_pdf::measure(MeasureOptions {
      viewport,
      fonts: &state,
      node,
      stylesheet: stylesheet(&self.resource_cache, options.css, Vec::new()).map_err(map_error)?,
      images,
      page,
      font_families: options.font_families.map(FontFamily::from_names),
      lang,
    })
    .map_err(map_error)?;

    Ok(
      to_value(&MeasuredSizeOutput {
        width: measured.width,
        height: measured.height,
      })
      .map_err(map_error)?
      .unchecked_into(),
    )
  }
}
