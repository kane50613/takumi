//! What every product shares: loading the document, its fonts and images, then rendering it.

use std::{collections::HashMap, error::Error, sync::Arc};

use takumi::prelude::{Fonts, ImageSource, RenderNeeds};

use crate::{
  document::Document,
  input::{InputArgs, OutputArgs},
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

  /// What the document needs loaded before this product renders it.
  fn needs(
    &self,
    document: &Document,
    fonts: &Fonts,
    images: &Images,
  ) -> Result<RenderNeeds, Box<dyn Error>>;

  /// The bytes of the document rendered as this product.
  fn render(
    &self,
    document: Document,
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
  let mut warnings = Vec::new();
  let document = Document::load(
    &loaded.html,
    &loaded.location,
    product.default_css(),
    &loaded.sandbox,
    &mut warnings,
  )?;
  let mut fonts = input.fonts()?;

  if output.list_resources {
    return document.list_resources(product, &fonts, &loaded.sandbox);
  }

  let images = document.load_resources(product, &mut fonts, &loaded.sandbox, &mut warnings)?;

  if !quiet {
    print_warnings(&warnings);
  }

  output.write(
    &product.render(document, &fonts, images)?,
    product.is_text(),
  )
}

fn print_warnings(warnings: &[ResourceError]) {
  for warning in warnings {
    eprintln!("warning: {warning}");
  }
}
