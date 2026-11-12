//! An HTML document and everything it loads through the sandbox: style sheets, images, and
//! `@font-face` files.

use std::{collections::HashMap, error::Error, sync::Arc};

use takumi::{
  parse_html,
  prelude::{
    FontSource, Fonts, FromHtmlOptions, ImageSource, ImageSourceInput, Node, NodeKind, StyleSheet,
  },
};
use url::Url;

use crate::{
  product::{Images, Product},
  sandbox::{ResourceError, Sandbox},
  stylesheet::StyleSheetLoader,
};

/// A parsed document, its CSS loaded, and the URL its relative references resolve against.
pub(crate) struct Document {
  pub(crate) node: Node,
  /// Its own CSS, in cascade order.
  pub(crate) css: Vec<String>,
  /// The style sheet it renders with, which [`Document::share_style_sheet`] can widen.
  pub(crate) stylesheet: Arc<StyleSheet>,
  pub(crate) base: Url,
}

impl Document {
  /// Parses the HTML file `html` found at `location` as a whole document, as a browser opens it,
  /// and loads the style sheets it links and imports through `loader`, after `default_css`,
  /// which they override.
  pub(crate) fn load(
    html: &str,
    location: &Url,
    default_css: Option<String>,
    loader: &mut StyleSheetLoader<'_>,
  ) -> Result<Self, Box<dyn Error>> {
    let options = FromHtmlOptions::builder().document(true).build();

    Self::parse(html, location, options, default_css, loader)
  }

  /// Parses a header or footer file as a fragment, as Chromium inserts a header or footer
  /// template into an element of its own page, and loads its style sheets through `loader`.
  pub(crate) fn load_band(
    html: &str,
    location: &Url,
    loader: &mut StyleSheetLoader<'_>,
  ) -> Result<Self, Box<dyn Error>> {
    let options = FromHtmlOptions::builder().fragment(true).build();

    Self::parse(html, location, options, None, loader)
  }

  /// Parses `html` with `options`, then loads the style sheets it links and imports.
  fn parse(
    html: &str,
    location: &Url,
    options: FromHtmlOptions,
    default_css: Option<String>,
    loader: &mut StyleSheetLoader<'_>,
  ) -> Result<Self, Box<dyn Error>> {
    let mut parsed = parse_html(html, options)?;
    let base = parsed
      .base_href
      .as_deref()
      .and_then(|href| location.join(href).ok())
      .unwrap_or_else(|| location.clone());

    for css in &mut parsed.css {
      *css = loader.rewrite(css, &base)?;
    }

    resolve_image_sources(&mut parsed.node, &base);

    let mut denied = None;
    let mut css: Vec<String> = default_css.into_iter().collect();

    css.extend(parsed.css_with_links(|link| {
      if denied.is_some() {
        return None;
      }

      let url = base.join(&link.href).ok()?;

      loader.load(&url).unwrap_or_else(|error| {
        denied = Some(error);
        None
      })
    }));

    if let Some(error) = denied {
      return Err(error.into());
    }

    Ok(Self {
      node: parsed.node,
      stylesheet: Arc::new(StyleSheet::parse_owned_list_loosy(css.clone())),
      css,
      base,
    })
  }

  /// Gives this document and `others` one style sheet: this document's CSS, then each of
  /// theirs in order, as a PDF's header and footer share the page's style sheet.
  ///
  /// Approximate: Chromium draws a header or footer template in a frame of its own, so its CSS,
  /// an `@page` rule included, never reaches the page. This follows takumi-pdf's JS API, which
  /// joins them.
  pub(crate) fn share_style_sheet(&mut self, others: &mut [Document]) {
    if others.is_empty() {
      return;
    }

    let css = self
      .css
      .iter()
      .chain(others.iter().flat_map(|other| &other.css))
      .cloned()
      .collect();
    let stylesheet = Arc::new(StyleSheet::parse_owned_list_loosy(css));

    for other in others.iter_mut() {
      other.stylesheet = Arc::clone(&stylesheet);
    }
    self.stylesheet = stylesheet;
  }

  /// Loads the `@font-face` files the document reaches into `fonts`, then its images into
  /// `images`. Fonts come first, since each font loaded can change which other faces the text
  /// reaches.
  pub(crate) fn load_resources(
    &self,
    product: &impl Product,
    fonts: &mut Fonts,
    images: &mut ImageCache,
    sandbox: &Sandbox,
    warnings: &mut Vec<ResourceError>,
  ) -> Result<(), Box<dyn Error>> {
    loop {
      let font_files = product.needs(self, fonts, &Images::new())?.font_files;

      if font_files.is_empty() {
        break;
      }

      let mut failed = Vec::new();

      for written in font_files {
        let Some(bytes) = self.read(&written, sandbox, warnings)? else {
          failed.push(written);
          continue;
        };

        if let Err(error) = fonts.load_font_file(written.as_str(), FontSource::from(bytes)) {
          warnings.push(ResourceError::Undecodable {
            url: written.clone(),
            reason: error.to_string(),
          });
          failed.push(written);
          continue;
        }

        log::debug!(target: "takumi::font", "loaded {written}");
      }

      *fonts = fonts.with_failed_font_files(&failed);
    }

    for written in product.needs(self, fonts, &Images::new())?.images {
      let Ok(url) = self.base.join(&written) else {
        continue;
      };

      images.add(written, url, sandbox, warnings)?;
    }

    Ok(())
  }

  /// Prints each image and font the document reaches, fallback `@font-face` sources included,
  /// with whether the sandbox lets it load, tab-separated.
  pub(crate) fn list_resources(
    &self,
    product: &impl Product,
    fonts: &Fonts,
    sandbox: &Sandbox,
    listing: &mut Vec<String>,
  ) -> Result<(), Box<dyn Error>> {
    let mut fonts = fonts.clone();
    let images = product.needs(self, &fonts, &Images::new())?.images;

    for written in &images {
      listing.push(self.listing_line("image", written, sandbox));
    }

    // Marking every source failed reaches the next one, as a font that fails to load would.
    loop {
      let font_files: Vec<String> = product
        .needs(self, &fonts, &Images::new())?
        .font_files
        .into_iter()
        .collect();

      if font_files.is_empty() {
        break;
      }

      for written in &font_files {
        listing.push(self.listing_line("font", written, sandbox));
      }

      fonts = fonts.with_failed_font_files(&font_files);
    }

    Ok(())
  }

  /// The listing line of the resource written as `written`.
  fn listing_line(&self, kind: &str, written: &str, sandbox: &Sandbox) -> String {
    match self.base.join(written) {
      Ok(url) => match sandbox.check(&url) {
        Ok(()) => format!("{kind}\tallowed\t{url}"),
        Err(error) => format!("{kind}\t{}\t{url}\t{error}", error.listing_status()),
      },
      Err(_) => format!("{kind}\tinvalid\t{written}"),
    }
  }

  /// The bytes of the resource written as `written`, none when a browser would skip it.
  fn read(
    &self,
    written: &str,
    sandbox: &Sandbox,
    warnings: &mut Vec<ResourceError>,
  ) -> Result<Option<Vec<u8>>, ResourceError> {
    let Ok(url) = self.base.join(written) else {
      return Ok(None);
    };

    read_or_skip(&url, sandbox, warnings)
  }
}

/// Resolves each image `src` under `node` against `base`, so the render tells apart two documents
/// that write the same `src` for different files, as a page and its header can.
fn resolve_image_sources(node: &mut Node, base: &Url) {
  match &mut node.kind {
    NodeKind::Container { children } => {
      for child in children {
        resolve_image_sources(child, base);
      }
    }
    // A `data:` URL is already absolute, and can be far too long to parse again.
    NodeKind::Image(image) => {
      if let ImageSourceInput::Url(src) = &mut image.src
        && !src.starts_with("data:")
        && let Ok(url) = base.join(src)
      {
        *src = Arc::from(url.as_str());
      }
    }
    _ => {}
  }
}

/// The bytes at `url`, none when a browser would skip it. Only a read that
/// [fails the render](ResourceError::fails_render) is an error.
fn read_or_skip(
  url: &Url,
  sandbox: &Sandbox,
  warnings: &mut Vec<ResourceError>,
) -> Result<Option<Vec<u8>>, ResourceError> {
  match sandbox.read(url) {
    Ok(bytes) => Ok(Some(bytes)),
    Err(error) if error.fails_render() => Err(error),
    Err(error) => {
      warnings.push(error);
      Ok(None)
    }
  }
}

/// The images of every document in one render, each file read and decoded once.
#[derive(Default)]
pub(crate) struct ImageCache {
  /// Each image by the key the render looks it up by, the URL as a document writes it.
  images: Images,
  /// The URL each key resolved to, so a second document writing the same key for another file
  /// is caught.
  resolved: HashMap<Arc<str>, Url>,
  decoded: HashMap<Url, ImageSource>,
}

impl ImageCache {
  /// Adds the image a document writes as `written` and resolves to `url`.
  fn add(
    &mut self,
    written: String,
    url: Url,
    sandbox: &Sandbox,
    warnings: &mut Vec<ResourceError>,
  ) -> Result<(), ResourceError> {
    let key: Arc<str> = Arc::from(written);

    if let Some(kept) = self.resolved.get(&key) {
      if *kept != url {
        warnings.push(ResourceError::SharedKey {
          written: key.to_string(),
          kept: kept.to_string(),
          other: url.to_string(),
        });
      }

      return Ok(());
    }

    let image = match self.decoded.get(&url) {
      Some(image) => image.clone(),
      None => {
        let Some(bytes) = read_or_skip(&url, sandbox, warnings)? else {
          return Ok(());
        };

        match ImageSource::from_bytes(&bytes) {
          Ok(image) => {
            self.decoded.insert(url.clone(), image.clone());
            image
          }
          Err(error) => {
            warnings.push(ResourceError::Undecodable {
              url: url.to_string(),
              reason: error.to_string(),
            });
            return Ok(());
          }
        }
      }
    };

    log::debug!(target: "takumi::image", "loaded {url}");
    self.resolved.insert(Arc::clone(&key), url);
    self.images.insert(key, image);

    Ok(())
  }

  /// The images, keyed as the documents write them.
  pub(crate) fn into_images(self) -> Images {
    self.images
  }
}
