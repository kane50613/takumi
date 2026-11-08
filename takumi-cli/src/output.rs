//! What the CLI writes: a raster image, an SVG, or a PDF, and what each needs loaded first.

use std::{collections::HashMap, error::Error, path::Path, sync::Arc};

use takumi::{
  needs,
  prelude::{
    Fonts, ImageSource, OutputFormat, Quality, RenderNeeds, RenderOptions, SvgOptions, Viewport,
  },
  render, render_svg, write_image,
};
use takumi_pdf::{MeasureOptions, PageOptions, PdfOptions};

use crate::document::Document;

/// The format the CLI writes.
#[derive(Clone, Copy)]
pub(crate) enum Output {
  Raster(OutputFormat),
  Svg,
  Pdf(PageOptions),
}

/// What every output renders from, apart from the document and the fonts.
pub(crate) struct RenderInputs {
  pub(crate) viewport: Viewport,
  pub(crate) images: HashMap<Arc<str>, ImageSource>,
}

impl Output {
  /// The format a file named `path` holds, by its extension.
  pub(crate) fn from_path(path: &Path, page: PageOptions) -> Option<Self> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();

    Some(match extension.as_str() {
      "png" => Self::Raster(OutputFormat::Png),
      "jpg" | "jpeg" => Self::Raster(OutputFormat::Jpeg {
        quality: Quality::default(),
      }),
      "webp" => Self::Raster(OutputFormat::WebPLossless),
      "svg" => Self::Svg,
      "pdf" => Self::Pdf(page),
      _ => return None,
    })
  }

  /// What the document needs loaded before this output renders it.
  pub(crate) fn needs(
    &self,
    inputs: &RenderInputs,
    document: &Document,
    fonts: &Fonts,
  ) -> Result<RenderNeeds, Box<dyn Error>> {
    Ok(match self {
      Self::Raster(_) | Self::Svg => needs(raster_options(inputs, document, fonts)),
      Self::Pdf(page) => takumi_pdf::needs(
        MeasureOptions::builder()
          .fonts(fonts)
          .node(document.node.clone())
          .stylesheet(Arc::clone(&document.stylesheet))
          .images(inputs.images.clone())
          .page(*page)
          .build(),
      )?,
    })
  }

  /// The bytes of the document rendered in this format.
  pub(crate) fn render(
    &self,
    inputs: RenderInputs,
    document: Document,
    fonts: &Fonts,
  ) -> Result<Vec<u8>, Box<dyn Error>> {
    match self {
      Self::Raster(format) => {
        let bitmap = render(raster_options(&inputs, &document, fonts))?;
        let mut bytes = Vec::new();

        write_image(&bitmap, &mut bytes, *format)?;

        Ok(bytes)
      }
      Self::Svg => Ok(
        render_svg(
          SvgOptions::builder()
            .viewport(inputs.viewport)
            .fonts(fonts)
            .node(document.node)
            .images(inputs.images)
            .stylesheet(document.stylesheet)
            .build(),
        )?
        .into_bytes(),
      ),
      Self::Pdf(page) => Ok(takumi_pdf::render(
        PdfOptions::builder()
          .fonts(fonts)
          .node(document.node)
          .stylesheet(document.stylesheet)
          .images(inputs.images)
          .page(*page)
          .build(),
      )?),
    }
  }
}

fn raster_options<'f>(
  inputs: &RenderInputs,
  document: &Document,
  fonts: &'f Fonts,
) -> RenderOptions<'f> {
  RenderOptions::builder()
    .viewport(inputs.viewport)
    .node(document.node.clone())
    .fonts(fonts)
    .images(inputs.images.clone())
    .stylesheet(Arc::clone(&document.stylesheet))
    .build()
}
