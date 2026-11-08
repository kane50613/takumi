//! An HTML document and everything it loads through the sandbox: style sheets, images, and
//! `@font-face` files.

use std::{collections::HashMap, error::Error, sync::Arc};

use takumi::{
  parse_html,
  prelude::{FontSource, Fonts, FromHtmlOptions, ImageSource, Node, StyleSheet},
};
use url::Url;

use crate::{
  output::{Output, RenderInputs},
  sandbox::{ResourceError, Sandbox},
  stylesheet::StyleSheetLoader,
};

/// A parsed document, its CSS loaded, and the URL its relative references resolve against.
pub(crate) struct Document {
  pub(crate) node: Node,
  pub(crate) stylesheet: Arc<StyleSheet>,
  pub(crate) base: Url,
}

impl Document {
  /// Parses `html` found at `location` and loads the style sheets it links and imports.
  pub(crate) fn load(
    html: &str,
    location: &Url,
    sandbox: &Sandbox,
    warnings: &mut Vec<ResourceError>,
  ) -> Result<Self, Box<dyn Error>> {
    let mut parsed = parse_html(html, FromHtmlOptions::default())?;
    let base = parsed
      .base_href
      .as_deref()
      .and_then(|href| location.join(href).ok())
      .unwrap_or_else(|| location.clone());
    let mut loader = StyleSheetLoader::new(sandbox);

    for css in &mut parsed.css {
      *css = loader.rewrite(css, &base)?;
    }

    let mut denied = None;
    let css = parsed.css_with_links(|link| {
      if denied.is_some() {
        return None;
      }

      let url = base.join(&link.href).ok()?;

      loader.load(&url).unwrap_or_else(|error| {
        denied = Some(error);
        None
      })
    });

    if let Some(error) = denied {
      return Err(error.into());
    }

    warnings.extend(loader.into_report().skipped);

    Ok(Self {
      node: parsed.node,
      stylesheet: Arc::new(StyleSheet::parse_owned_list_loosy(css)),
      base,
    })
  }

  /// The images the document reaches, decoded and keyed as written. Loads its `@font-face` files
  /// into `fonts` first, since each font loaded can change which other faces the text reaches.
  pub(crate) fn load_resources(
    &self,
    output: &Output,
    inputs: &RenderInputs,
    fonts: &mut Fonts,
    sandbox: &Sandbox,
    warnings: &mut Vec<ResourceError>,
  ) -> Result<HashMap<Arc<str>, ImageSource>, Box<dyn Error>> {
    loop {
      let font_files = output.needs(inputs, self, fonts)?.font_files;

      if font_files.is_empty() {
        break;
      }

      let mut failed = Vec::new();

      for written in font_files {
        let loaded = self.read(&written, sandbox, warnings)?.and_then(|bytes| {
          fonts
            .load_font_file(written.as_str(), FontSource::from(bytes))
            .ok()
        });

        if loaded.is_none() {
          failed.push(written);
        }
      }

      *fonts = fonts.with_failed_font_files(&failed);
    }

    let mut images = HashMap::new();

    for written in output.needs(inputs, self, fonts)?.images {
      let Some(bytes) = self.read(&written, sandbox, warnings)? else {
        continue;
      };

      match ImageSource::from_bytes(&bytes) {
        Ok(image) => {
          images.insert(Arc::from(written), image);
        }
        Err(error) => eprintln!("warning: {written} is not an image takumi can decode: {error}"),
      }
    }

    Ok(images)
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

    match sandbox.read(&url) {
      Ok(bytes) => Ok(Some(bytes)),
      Err(error) if error.fails_render() => Err(error),
      Err(error) => {
        warnings.push(error);
        Ok(None)
      }
    }
  }
}
