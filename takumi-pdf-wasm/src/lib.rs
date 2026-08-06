//! WebAssembly bindings for takumi-pdf.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{
  collections::HashMap,
  sync::{Arc, RwLock},
};

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{build_font_resource, default_fonts, stylesheet};
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::{
    font::FontResource,
    image::{ImageCacheMode, ResourceCache},
  },
  style::{FontFamily, FontStyle as CssFontStyle, FromCssStr, Lang},
  viewport::Viewport,
};
use takumi_pdf::{PageMargins, PageOptions, PdfMetadata, PdfOptions, PdfStandard};
use wasm_bindgen::prelude::*;

fn map_error(error: impl core::fmt::Debug) -> js_sys::Error {
  js_sys::Error::new(&format!("{error:?}"))
}

/// Details for loading a custom font.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontDetails {
  name: Option<String>,
  data: ByteBuf,
  weight: Option<f64>,
  style: Option<FontStyle>,
  subset_of: Option<String>,
  generic: Option<String>,
}

/// Font input, either as detailed object or raw buffer.
#[derive(Deserialize)]
#[serde(untagged)]
enum Font {
  Object(FontDetails),
  Buffer(ByteBuf),
}

struct FontStyle(CssFontStyle);

impl<'de> Deserialize<'de> for FontStyle {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value = String::deserialize(deserializer)?;
    Ok(Self(
      CssFontStyle::from_css_str(&value).map_err(D::Error::custom)?,
    ))
  }
}

/// An image source with its URL and raw data.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageSource {
  src: Arc<str>,
  data: ByteBuf,
  cache: Option<ImageCacheMode>,
}

/// Explicit page or viewport dimensions in CSS px.
#[derive(Deserialize, Clone, Copy)]
struct Dimensions {
  width: f32,
  height: f32,
}

/// Viewport for single-page output. A missing height sizes the page to the
/// laid-out content.
#[derive(Deserialize, Clone, Copy)]
struct ViewportInput {
  width: f32,
  height: Option<f32>,
}

/// A page size: a preset name (`"a4"`, `"letter"`) or explicit dimensions.
#[derive(Deserialize)]
#[serde(untagged)]
enum SizeInput {
  Named(String),
  Dimensions(Dimensions),
}

/// A page margin: one number for all sides, or per-side values (missing
/// sides are zero).
#[derive(Deserialize)]
#[serde(untagged)]
enum MarginInput {
  Uniform(f32),
  Sides {
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    left: Option<f32>,
  },
}

fn resolve_page(
  size: Option<&SizeInput>,
  landscape: bool,
  margin: Option<&MarginInput>,
) -> Result<PageOptions, js_sys::Error> {
  let mut page = match size {
    None => PageOptions::A4,
    Some(SizeInput::Dimensions(dimensions)) => PageOptions {
      width: dimensions.width,
      height: dimensions.height,
      ..PageOptions::A4
    },
    Some(SizeInput::Named(name)) => match name.to_ascii_lowercase().as_str() {
      "a4" => PageOptions::A4,
      "letter" => PageOptions::LETTER,
      other => {
        return Err(js_sys::Error::new(&format!("unknown page size: {other}")));
      }
    },
  };

  if landscape {
    page = page.landscape();
  }
  match margin {
    None => {}
    Some(MarginInput::Uniform(value)) => page.margin = PageMargins::uniform(*value),
    Some(MarginInput::Sides {
      top,
      right,
      bottom,
      left,
    }) => {
      page.margin = PageMargins {
        top: top.unwrap_or(0.0),
        right: right.unwrap_or(0.0),
        bottom: bottom.unwrap_or(0.0),
        left: left.unwrap_or(0.0),
      };
    }
  }
  Ok(page)
}

/// Options for rendering a PDF.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PdfRenderOptions {
  /// Fixed viewport for single-page output: percentage heights resolve
  /// against it and overflowing content is clipped. Mutually exclusive with
  /// the paged fields.
  viewport: Option<ViewportInput>,
  /// Page size for paged output. Defaults to A4.
  size: Option<SizeInput>,
  /// Swaps the page's width and height, including explicit sizes.
  landscape: Option<bool>,
  /// Page margin in CSS px: one number or per-side values.
  margin: Option<MarginInput>,
  /// Band repeated at the top of every page. Nodes classed `pageNumber` /
  /// `totalPages` receive the counters, optionally formatted by a
  /// supported `@counter-style` name in the same class list.
  header: Option<Node>,
  /// Band repeated at the bottom of every page; same class hooks as `header`.
  footer: Option<Node>,
  /// Pre-fetched images keyed by URL.
  images: Option<Vec<ImageSource>>,
  /// CSS stylesheets to apply before layout.
  stylesheets: Option<Vec<String>>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  font_families: Option<Vec<String>>,
  /// Default BCP-47 language tag applied to the root.
  lang: Option<String>,
  /// Document metadata written to the PDF's info dictionary.
  metadata: Option<MetadataInput>,
  /// Generates a PDF outline (bookmarks) from `h1`–`h6` headings.
  outline: Option<bool>,
  /// PDF/A conformance level: "2b", "2u", "3b", "3u" or "4".
  pdfa: Option<PdfaInput>,
}

/// PDF/A conformance level names accepted from JS.
#[derive(Deserialize, Clone, Copy)]
enum PdfaInput {
  #[serde(rename = "2b")]
  A2b,
  #[serde(rename = "2u")]
  A2u,
  #[serde(rename = "3b")]
  A3b,
  #[serde(rename = "3u")]
  A3u,
  #[serde(rename = "4")]
  A4,
}

impl From<PdfaInput> for PdfStandard {
  fn from(pdfa: PdfaInput) -> Self {
    match pdfa {
      PdfaInput::A2b => PdfStandard::A2b,
      PdfaInput::A2u => PdfStandard::A2u,
      PdfaInput::A3b => PdfStandard::A3b,
      PdfaInput::A3u => PdfStandard::A3u,
      PdfaInput::A4 => PdfStandard::A4,
    }
  }
}

/// Document metadata fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataInput {
  title: Option<String>,
  description: Option<String>,
  authors: Option<Vec<String>>,
  keywords: Option<Vec<String>>,
  creator: Option<String>,
}

impl From<MetadataInput> for PdfMetadata {
  fn from(input: MetadataInput) -> Self {
    Self {
      title: input.title,
      description: input.description,
      authors: input.authors.unwrap_or_default(),
      keywords: input.keywords.unwrap_or_default(),
      creator: input.creator,
    }
  }
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
  pub fn register_font(&self, font: JsValue) -> Result<JsValue, js_sys::Error> {
    let font: Font = from_value(font).map_err(map_error)?;
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
          details.generic,
        )
        .map_err(map_error)?;

        state.register(resource).map_err(map_error)?
      }
    };

    to_value(&registered).map_err(map_error)
  }

  /// Renders a node tree to PDF bytes. Without options the output is paged A4;
  /// `viewport` renders a single fixed page instead.
  #[wasm_bindgen]
  pub fn render(
    &self,
    node: JsValue,
    options: Option<js_sys::Object>,
  ) -> Result<Vec<u8>, js_sys::Error> {
    let node: Node = from_value(node).map_err(map_error)?;
    let options: PdfRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let mut images = HashMap::new();

    for source in options.images.unwrap_or_default() {
      let image = self
        .resource_cache
        .get_or_decode(&source.data, source.cache.unwrap_or_default())
        .map_err(map_error)?;

      images.insert(source.src, image);
    }

    let paged_field_set = options.size.is_some()
      || options.landscape.is_some()
      || options.margin.is_some()
      || options.header.is_some()
      || options.footer.is_some();
    let (viewport, page) = match options.viewport {
      Some(_) if paged_field_set => {
        return Err(js_sys::Error::new(
          "viewport is mutually exclusive with the paged options (size, landscape, margin, header, footer)",
        ));
      }
      Some(input) => (
        Some(Viewport::new((
          input.width as u32,
          input.height.map(|height| height as u32),
        ))),
        None,
      ),
      None => (
        None,
        Some(resolve_page(
          options.size.as_ref(),
          options.landscape.unwrap_or(false),
          options.margin.as_ref(),
        )?),
      ),
    };
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
      header: options.header,
      footer: options.footer,
      font_families: options.font_families.map(FontFamily::from_names),
      lang,
      metadata: options.metadata.map(PdfMetadata::from),
      outline: options.outline.unwrap_or(false),
      standard: options.pdfa.map(PdfStandard::from).unwrap_or_default(),
    })
    .map_err(map_error)
  }
}
