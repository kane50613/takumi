//! `takumi image`: a PNG, JPEG, WebP, or SVG of the document.

use std::{error::Error, sync::Arc};

use clap::ValueEnum;
use takumi::{
  needs,
  prelude::{
    Error as RenderError, Fonts, OutputFormat, Quality, RenderNeeds, RenderOptions, SvgOptions,
    Viewport,
  },
  render, render_svg, uncovered_characters, write_image,
};

use crate::{
  document::Document,
  input::{InputArgs, OutputArgs, UsageError},
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
  /// The viewport width in CSS pixels. Set it to the width the document is designed for.
  #[arg(long, default_value_t = 1200, value_parser = clap::value_parser!(u32).range(1..))]
  width: u32,

  /// The viewport height in CSS pixels. Without it, the image is as tall as its content.
  #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
  height: Option<u32>,

  /// The device pixel ratio, the image pixels per CSS pixel. `--dpr 2` draws each CSS pixel as
  /// two image pixels across, for high-density screens.
  #[arg(long, default_value_t = 1.0, value_parser = parse_device_pixel_ratio)]
  dpr: f32,
}

impl ViewportArgs {
  /// The viewport in the device pixels takumi measures it in: the CSS size scaled by `--dpr`.
  fn device_viewport(&self) -> Viewport {
    let device_pixels = |css_pixels: u32| {
      (f64::from(css_pixels) * f64::from(self.dpr))
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32
    };

    Viewport::new((device_pixels(self.width), self.height.map(device_pixels)))
      .with_device_pixel_ratio(self.dpr)
  }
}

/// `--dpr`: a finite ratio above zero.
fn parse_device_pixel_ratio(value: &str) -> Result<f32, String> {
  value
    .parse::<f32>()
    .ok()
    .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
    .ok_or_else(|| {
      format!("`{value}` is not a device pixel ratio. Use a number above 0, such as 2.")
    })
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
  /// The image product these flags describe, its format read from `--format` or the output's extension.
  pub(crate) fn product(&self) -> Result<Image, Box<dyn Error>> {
    let usage = |message: String| -> Box<dyn Error> { Box::new(UsageError(message)) };
    let format = match (self.format, self.output.extension().as_deref()) {
      (Some(format), _) => format,
      // A listing writes no file, and every format loads the same resources.
      (None, _) if !self.output.renders() => ImageFormat::Png,
      (None, Some("pdf")) => {
        return Err(usage(
          "`takumi image` writes images, but the output ends in `.pdf`. Use `takumi pdf`.".into(),
        ));
      }
      (None, Some(extension)) => ImageFormat::from_str(extension, true).map_err(|_| {
        usage(format!("`.{extension}` is not an image format takumi writes. Use png, jpg, webp, or svg, or pass `--format`."))
      })?,
      (None, None) if self.output.is_stdout() => {
        return Err(usage("writing to standard output needs `--format`.".into()));
      }
      (None, None) => {
        return Err(usage("the output has no extension. Add png, jpg, webp, or svg, or pass `--format`.".into()));
      }
    };

    Ok(Image {
      format,
      quality: self.quality.map(Quality::new),
      viewport: self.viewport.device_viewport(),
    })
  }
}

impl Image {
  /// The options that render `document` at this product's viewport.
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

  fn warnings(&self, document: &Document, fonts: &Fonts, images: &Images) -> Vec<String> {
    let uncovered =
      uncovered_characters(self.options(document, fonts, images.clone())).unwrap_or_default();

    if uncovered.is_empty() {
      return Vec::new();
    }

    let named = uncovered
      .iter()
      .map(|&character| format!("{character} (U+{:04X})", character as u32))
      .collect::<Vec<_>>()
      .join(", ");

    vec![format!(
      "no font draws {named}, so each shows as an empty box. Pass a font that does with `--font`, or check the `font-family` of that text."
    )]
  }

  fn render(
    &self,
    document: Document,
    _bands: Vec<Document>,
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
          "the image would be 0px tall or over 64 megapixels at the content's height. Pass `--height`.".into()
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_device_pixel_ratio_scales_the_css_size() {
    let viewport = ViewportArgs {
      width: 1200,
      height: Some(630),
      dpr: 1.5,
    }
    .device_viewport();

    assert_eq!(
      (viewport.size.width, viewport.size.height),
      (Some(1800), Some(945))
    );
    assert_eq!(viewport.device_pixel_ratio, 1.5);
  }
}
