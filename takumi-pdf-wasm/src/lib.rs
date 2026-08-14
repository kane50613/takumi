//! WebAssembly bindings for takumi-pdf.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod date;
mod font;
mod metadata;
mod options;

use std::sync::RwLock;

use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{build_font_resource, default_fonts, stylesheet};
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::{font::FontResource, image::ResourceCache},
  style::{FontFamily, Lang},
};
use takumi_pdf::{Attachment, MeasureOptions, PdfMetadata, PdfOptions, PdfStandard, Tagging};
use wasm_bindgen::prelude::*;

use crate::{
  font::Font,
  options::{PdfRenderOptions, decode_images, page_background, resolve_geometry},
};

pub(crate) fn map_error(error: impl core::fmt::Display) -> js_sys::Error {
  js_sys::Error::new(&error.to_string())
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
    let mut state = self
      .state
      .try_write()
      .map_err(|error| js_sys::Error::new(&format!("Renderer state is locked: {error}")))?;

    let registered = match font {
      Font::Buffer(buffer) => state
        .register(FontResource::new(buffer.into_vec()))
        .map_err(map_error)?,
      Font::Object(details) => {
        let data = details.data.into_vec();
        let resource = build_font_resource(
          &data,
          details.name,
          details.weight.map(|weight| weight as f32),
          details.style.map(|style| style.0),
          details.subset_of,
          details.subset_rank,
          details.generic,
        )
        .map_err(map_error)?;

        state.register(resource).map_err(map_error)?
      }
    };

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
    let mut options: PdfRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = decode_images(&self.resource_cache, options.images.take())?;
    let (viewport, page) = resolve_geometry(&options)?;
    let lang = options
      .lang
      .as_deref()
      .map(Lang::parse)
      .transpose()
      .map_err(map_error)?;
    let state = self
      .state
      .try_read()
      .map_err(|error| js_sys::Error::new(&format!("Renderer state is locked: {error}")))?;

    takumi_pdf::render(PdfOptions {
      viewport,
      fonts: &state,
      node,
      stylesheet: stylesheet(&self.resource_cache, options.stylesheets, Vec::new()),
      images,
      page,
      background_color: page_background(options.background_color.as_deref())?,
      header: options.header,
      footer: options.footer,
      font_families: options.font_families.map(FontFamily::from_names),
      lang,
      metadata: options.metadata.map(PdfMetadata::try_from).transpose()?,
      outline: options.outline.unwrap_or(false),
      standard: options.pdfa.map(PdfStandard::from).unwrap_or_default(),
      tagged: options.tagged.map(Tagging::from).unwrap_or_default(),
      attachments: options
        .attachments
        .unwrap_or_default()
        .into_iter()
        .map(Attachment::try_from)
        .collect::<Result<_, _>>()?,
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
    let mut options: PdfRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let images = decode_images(&self.resource_cache, options.images.take())?;
    let (viewport, page) = resolve_geometry(&options)?;
    let lang = options
      .lang
      .as_deref()
      .map(Lang::parse)
      .transpose()
      .map_err(map_error)?;
    let state = self
      .state
      .try_read()
      .map_err(|error| js_sys::Error::new(&format!("Renderer state is locked: {error}")))?;
    let measured = takumi_pdf::measure(MeasureOptions {
      viewport,
      fonts: &state,
      node,
      stylesheet: stylesheet(&self.resource_cache, options.stylesheets, Vec::new()),
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
