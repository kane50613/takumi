//! What every product shares: loading the document, its fonts and images, then rendering it.

use std::{collections::HashMap, error::Error, path::Path, sync::Arc};

use takumi::prelude::{Fonts, ImageSource, RenderNeeds};

use crate::{
  document::{Document, ImageCache},
  input::{InputArgs, OutputArgs, read_html_file},
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
  quiet: bool,
) -> Result<(), Box<dyn Error>> {
  let loaded = input.read()?;
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

      Document::load(&html, &location, None, &mut loader)
    })
    .collect::<Result<Vec<_>, _>>()?;
  let report = loader.into_report();
  let mut warnings = report.skipped;

  document.share_style_sheet(&mut bands);

  let mut fonts = input.fonts()?;

  if listing {
    for url in &report.loaded {
      println!("stylesheet\tallowed\t{url}");
    }
    for each in std::iter::once(&document).chain(&bands) {
      each.list_resources(product, &fonts, sandbox)?;
    }
    print_warnings(&warnings, quiet);

    return Ok(());
  }

  let mut images = ImageCache::default();

  for each in std::iter::once(&document).chain(&bands) {
    each.load_resources(product, &mut fonts, &mut images, sandbox, &mut warnings)?;
  }

  print_warnings(&warnings, quiet);

  output.write(
    &product.render(document, bands, &fonts, images.into_images())?,
    product.is_text(),
  )
}

/// Prints each warning to standard error, unless `-q` hides them.
fn print_warnings(warnings: &[ResourceError], quiet: bool) {
  if quiet {
    return;
  }

  for warning in warnings {
    eprintln!("warning: {warning}");
  }
}
