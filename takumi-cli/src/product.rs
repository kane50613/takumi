//! What every product shares: loading the document, its fonts and images, then rendering it.

use std::{collections::HashMap, error::Error, path::Path, sync::Arc};

use takumi::prelude::{Fonts, ImageSource, RenderNeeds};

use crate::{
  document::Document,
  input::{InputArgs, OutputArgs, read_html_file},
  sandbox::ResourceError,
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
  let mut warnings = Vec::new();
  let mut document = Document::load(
    &loaded.html,
    &loaded.location,
    product.default_css(),
    sandbox,
    &mut warnings,
  )?;
  let mut bands = product
    .band_files()
    .into_iter()
    .map(|path| {
      let (html, location) = read_html_file(path)?;

      Document::load(&html, &location, None, sandbox, &mut warnings)
    })
    .collect::<Result<Vec<_>, _>>()?;

  document.share_style_sheet(&mut bands);

  let mut fonts = input.fonts()?;

  if output.list_resources {
    for each in std::iter::once(&document).chain(&bands) {
      each.list_resources(product, &fonts, sandbox)?;
    }

    return Ok(());
  }

  let mut images = Images::new();

  for each in std::iter::once(&document).chain(&bands) {
    // A band's image keyed as the page writes another wins, as the band renders after the page.
    images.extend(each.load_resources(product, &mut fonts, sandbox, &mut warnings)?);
  }

  if !quiet {
    print_warnings(&warnings);
  }

  output.write(
    &product.render(document, bands, &fonts, images)?,
    product.is_text(),
  )
}

fn print_warnings(warnings: &[ResourceError]) {
  for warning in warnings {
    eprintln!("warning: {warning}");
  }
}
