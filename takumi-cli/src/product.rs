//! What every product shares: loading the document, its fonts and images, then rendering it.

use std::{collections::HashMap, error::Error, path::Path, sync::Arc};

use takumi::prelude::{Fonts, ImageSource, RenderNeeds};

use crate::{
  document::{Document, ImageCache},
  input::{InputArgs, OutputArgs, read_html_file},
  logger::Logger,
  sandbox::ResourceError,
  stylesheet::StyleSheetLoader,
};

/// The decoded images a render looks up, keyed by URL as the document writes it.
pub(crate) type Images = HashMap<Arc<str>, ImageSource>;

/// One thing `takumi` writes from a document.
pub(crate) trait Product {
  /// CSS this product's flags set, which the document's own style sheets override.
  fn default_css(&self) -> Option<String> {
    None
  }

  /// HTML files the command line names besides the input, such as a PDF's header, each loaded
  /// as a document of its own and handed to [`Product::render`] in this order.
  fn band_files(&self) -> Vec<&Path> {
    Vec::new()
  }

  /// What the document needs loaded before this product renders it.
  fn needs(
    &self,
    document: &Document,
    fonts: &Fonts,
    images: &Images,
  ) -> Result<RenderNeeds, Box<dyn Error>>;

  /// What the render would get wrong without failing, each as a warning to print before it.
  fn warnings(&self, _document: &Document, _fonts: &Fonts, _images: &Images) -> Vec<String> {
    Vec::new()
  }

  /// The bytes of the document rendered as this product, with the documents of its
  /// [band files](Product::band_files).
  fn render(
    &self,
    document: Document,
    bands: Vec<Document>,
    fonts: &Fonts,
    images: Images,
  ) -> Result<Vec<u8>, Box<dyn Error>>;

  /// Whether the bytes are text a terminal can show.
  fn is_text(&self) -> bool;
}

/// Renders the input as `product` to the output, or lists what it would load.
pub(crate) fn run(
  product: &impl Product,
  input: &InputArgs,
  output: &OutputArgs,
  deny_warnings: bool,
) -> Result<(), Box<dyn Error>> {
  let loaded = input.read()?;

  log::info!(
    target: "takumi::input",
    "read {}, {} bytes",
    loaded.location,
    loaded.html.len()
  );

  let sandbox = &loaded.sandbox;
  let listing = output.list_resources;
  let mut loader = if listing {
    StyleSheetLoader::lenient(sandbox)
  } else {
    StyleSheetLoader::new(sandbox)
  };
  let mut document = Document::load(
    &loaded.html,
    &loaded.location,
    product.default_css(),
    &mut loader,
  )?;
  let mut bands = product
    .band_files()
    .into_iter()
    .map(|path| {
      let (html, location) = read_html_file(path)?;

      Document::load_band(&html, &location, &mut loader)
    })
    .collect::<Result<Vec<_>, _>>()?;
  let report = loader.into_report();
  let mut warnings = report.skipped;

  for url in &report.loaded {
    log::debug!(target: "takumi::stylesheet", "loaded {url}");
  }
  log::info!(target: "takumi::stylesheet", "style sheets loaded: {}", report.loaded.len());

  document.share_style_sheet(&mut bands);

  let mut fonts = input.fonts()?;

  if listing {
    let mut lines: Vec<String> = report
      .loaded
      .iter()
      .map(|url| format!("stylesheet\tallowed\t{url}"))
      .collect();

    // A stylesheet that would not load is a listing line like any other resource.
    warnings.retain(|error| match error.url() {
      Some(url) => {
        lines.push(format!(
          "stylesheet\t{}\t{url}\t{error}",
          error.listing_status()
        ));
        false
      }
      None => true,
    });

    for each in std::iter::once(&document).chain(&bands) {
      each.list_resources(product, &fonts, sandbox, &mut lines)?;
    }
    warn(&warnings);
    // `--deny-warnings` writes nothing on a warning, the listing included.
    Logger::deny_warnings(deny_warnings)?;

    for line in lines {
      println!("{line}");
    }

    return Ok(());
  }

  let mut images = ImageCache::default();

  for each in std::iter::once(&document).chain(&bands) {
    each.load_resources(product, &mut fonts, &mut images, sandbox, &mut warnings)?;
  }

  warn(&warnings);

  let images = images.into_images();

  log::info!(target: "takumi::resource", "images loaded: {}", images.len());

  for warning in product.warnings(&document, &fonts, &images) {
    log::warn!(target: "takumi::render", "{warning}");
  }

  let rendered = product.render(document, bands, &fonts, images)?;

  log::info!(target: "takumi::render", "rendered {} bytes", rendered.len());
  Logger::deny_warnings(deny_warnings)?;
  output.write(&rendered, product.is_text())
}

/// Logs each resource the render went on without.
fn warn(skipped: &[ResourceError]) {
  for error in skipped {
    log::warn!(target: "takumi::resource", "{error}");
  }
}
