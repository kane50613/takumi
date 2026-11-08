//! `takumi image`: a PNG, JPEG, WebP, or SVG of the document.

use std::{error::Error, sync::Arc};

use clap::ValueEnum;
use takumi::{
  needs,
  prelude::{
    Error as RenderError, Fonts, OutputFormat, Quality, RenderNeeds, RenderOptions, SvgOptions,
    Viewport,
  },
  render, render_svg, write_image,
};

use crate::{
  document::Document,
  input::{InputArgs, OutputArgs},
  product::{Images, Product},
};

/// Renders an HTML file to a PNG, JPEG, WebP, or SVG image.
#[derive(clap::Args)]
#[command(after_help = "Examples:
  takumi image card.html -o card.png --width 1200 --height 630
  takumi image card.html -o card@2x.webp --dpr 2
  cat card.html | takumi image - -o - --format png > card.png")]
pub(crate) struct ImageArgs {
  #[command(flatten)]
  pub(crate) input: InputArgs,

  #[command(flatten)]
  pub(crate) output: OutputArgs,

  #[command(flatten)]
  viewport: ViewportArgs,

  /// The format to write, in place of the one the output's extension names.
  #[arg(long, value_enum)]
  format: Option<ImageFormat>,

  /// The encoding quality from 0 to 100. JPEG defaults to 75, and WebP is lossless without it.
  #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100))]
  quality: Option<u8>,
}

/// The viewport a raster product lays the document out in.
#[derive(clap::Args)]
struct ViewportArgs {
  /// The viewport width in CSS pixels.
  #[arg(long, default_value_t = 1200)]
  width: u32,

  /// The viewport height in CSS pixels. Without it, the image is as tall as its content.
  #[arg(long)]
  height: Option<u32>,

  /// The device pixel ratio, which scales the image's pixels.
  #[arg(long, default_value_t = 1.0)]
  dpr: f32,
}

#[derive(Clone, Copy, ValueEnum)]
enum ImageFormat {
  Png,
  #[value(alias = "jpg")]
  Jpeg,
  Webp,
  Svg,
}

/// An image product with its format settled.
pub(crate) struct Image {
  format: ImageFormat,
  quality: Option<Quality>,
  viewport: Viewport,
}

impl ImageArgs {
  pub(crate) fn product(&self) -> Result<Image, Box<dyn Error>> {
    let format = match (self.format, self.output.extension().as_deref()) {
      (Some(format), _) => format,
      (None, Some(extension)) => ImageFormat::from_str(extension, true).map_err(|_| {
        format!("`.{extension}` is not an image format takumi writes. Use png, jpg, webp, or svg, or pass `--format`.")
      })?,
      (None, None) if self.output.is_stdout() => {
        return Err("writing to standard output needs `--format`.".into());
      }
      (None, None) => {
        return Err("the output has no extension. Add png, jpg, webp, or svg, or pass `--format`.".into());
      }
    };

    Ok(Image {
      format,
      quality: self.quality.map(Quality::new),
      viewport: Viewport::new((self.viewport.width, self.viewport.height))
        .with_device_pixel_ratio(self.viewport.dpr),
    })
  }
}

impl Image {
  fn options<'f>(
    &self,
    document: &Document,
    fonts: &'f Fonts,
    images: Images,
  ) -> RenderOptions<'f> {
    RenderOptions::builder()
      .viewport(self.viewport)
      .node(document.node.clone())
      .fonts(fonts)
      .images(images)
      .stylesheet(Arc::clone(&document.stylesheet))
      .build()
  }
}

impl Product for Image {
  fn needs(
    &self,
    document: &Document,
    fonts: &Fonts,
    images: &Images,
  ) -> Result<RenderNeeds, Box<dyn Error>> {
    Ok(needs(self.options(document, fonts, images.clone())))
  }

  fn render(
    &self,
    document: Document,
    fonts: &Fonts,
    images: Images,
  ) -> Result<Vec<u8>, Box<dyn Error>> {
    let format = match (self.format, self.quality) {
      (ImageFormat::Svg, _) => {
        let svg = render_svg(
          SvgOptions::builder()
            .viewport(self.viewport)
            .fonts(fonts)
            .node(document.node)
            .images(images)
            .stylesheet(document.stylesheet)
            .build(),
        )?;

        return Ok(svg.into_bytes());
      }
      (ImageFormat::Png, _) => OutputFormat::Png,
      (ImageFormat::Jpeg, quality) => OutputFormat::Jpeg {
        quality: quality.unwrap_or_default(),
      },
      (ImageFormat::Webp, Some(quality)) => OutputFormat::WebP { quality },
      (ImageFormat::Webp, None) => OutputFormat::WebPLossless,
    };
    let bitmap = render(self.options(&document, fonts, images)).map_err(|error| -> Box<dyn Error> {
      match error {
        RenderError::InvalidViewport if self.viewport.size.height.is_none() => {
          "the document's content is 0px tall, so the image would have no pixels. Pass `--height`.".into()
        }
        error => error.into(),
      }
    })?;
    let mut bytes = Vec::new();

    write_image(&bitmap, &mut bytes, format)?;

    Ok(bytes)
  }

  fn is_text(&self) -> bool {
    matches!(self.format, ImageFormat::Svg)
  }
}
