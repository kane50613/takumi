//! `takumi`: renders an HTML file to an image, SVG, or PDF.
//!
//! A document reads only local files under the directories it is granted, and never the network.
//! See [`sandbox`].

mod document;
mod output;
mod sandbox;
mod stylesheet;

use std::{
  env,
  error::Error,
  fs,
  io::{self, Read},
  path::{Path, PathBuf},
  process::ExitCode,
};

use clap::{Parser, Subcommand, ValueEnum};
use takumi::prelude::{FontResource, Fonts, Viewport};
use takumi_pdf::PageOptions;
use url::Url;

use crate::{
  document::Document,
  output::{Output, RenderInputs},
  sandbox::{Sandbox, directory_url},
};

/// The font text falls back to when the document loads none of its own.
const DEFAULT_FONT: &[u8] = include_bytes!("../../assets/fonts/geist/Geist[wght].woff2");

/// Renders HTML and CSS to images, SVG, and PDF.
#[derive(Parser)]
#[command(name = "takumi", version)]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Subcommand)]
enum Command {
  /// Renders an HTML file to a PNG, JPEG, WebP, SVG, or PDF file.
  Render(RenderArgs),
}

#[derive(clap::Args)]
struct RenderArgs {
  /// The HTML file to render, or `-` to read it from standard input.
  input: PathBuf,

  /// The file to write. Its extension picks the format: png, jpg, webp, svg, or pdf.
  #[arg(short, long)]
  output: PathBuf,

  /// The viewport width in CSS pixels.
  #[arg(long, default_value_t = 1200)]
  width: u32,

  /// The viewport height in CSS pixels. Without it, the image is as tall as its content.
  #[arg(long)]
  height: Option<u32>,

  /// The device pixel ratio, which scales a raster image.
  #[arg(long, default_value_t = 1.0)]
  dpr: f32,

  /// The PDF page size, which an `@page` rule overrides.
  #[arg(long, value_enum, default_value_t = PageSize::A4)]
  page: PageSize,

  /// A font file to register. Repeat it for more fonts.
  #[arg(long = "font", value_name = "FILE")]
  fonts: Vec<PathBuf>,

  /// The directory the document may read files from. Defaults to the input file's directory,
  /// and to none when the input is standard input.
  #[arg(long, value_name = "DIR")]
  root: Option<PathBuf>,

  /// Another directory the document may read files from. Repeat it for more.
  #[arg(long = "allow-read", value_name = "DIR")]
  allow_read: Vec<PathBuf>,

  /// Lists the files the document would load and whether it may, without rendering.
  #[arg(long)]
  list_resources: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum PageSize {
  A4,
  Letter,
}

impl From<PageSize> for PageOptions {
  fn from(size: PageSize) -> Self {
    match size {
      PageSize::A4 => Self::A4,
      PageSize::Letter => Self::LETTER,
    }
  }
}

fn main() -> ExitCode {
  let Command::Render(args) = Cli::parse().command;

  match args.run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("error: {error}");
      ExitCode::FAILURE
    }
  }
}

impl RenderArgs {
  fn run(self) -> Result<(), Box<dyn Error>> {
    let output = Output::from_path(&self.output, self.page.into()).ok_or_else(|| {
      format!(
        "{} has no extension takumi writes. Use png, jpg, webp, svg, or pdf.",
        self.output.display()
      )
    })?;
    let (html, location, default_root) = self.read_input()?;
    let sandbox = Sandbox::new(
      self
        .root
        .clone()
        .or(default_root)
        .into_iter()
        .chain(self.allow_read.clone()),
    )?;
    let mut warnings = Vec::new();
    let document = Document::load(&html, &location, &sandbox, &mut warnings)?;

    if self.list_resources {
      return self.list_resources(&output, &document, &sandbox);
    }

    let mut fonts = self.fonts()?;
    let mut inputs = RenderInputs {
      viewport: self.viewport(),
      images: Default::default(),
    };

    inputs.images =
      document.load_resources(&output, &inputs, &mut fonts, &sandbox, &mut warnings)?;

    for warning in &warnings {
      eprintln!("warning: {warning}");
    }

    fs::write(&self.output, output.render(inputs, document, &fonts)?)?;

    Ok(())
  }

  /// The HTML, the URL it is found at, and the directory it may read by default.
  fn read_input(&self) -> io::Result<(String, Url, Option<PathBuf>)> {
    if self.input == Path::new("-") {
      let mut html = String::new();

      io::stdin().read_to_string(&mut html)?;

      return Ok((html, directory_url(&env::current_dir()?)?, None));
    }

    let path = fs::canonicalize(&self.input)?;
    let location = Url::from_file_path(&path)
      .map_err(|()| io::Error::new(io::ErrorKind::InvalidInput, "not an absolute path"))?;

    Ok((
      fs::read_to_string(&path)?,
      location,
      path.parent().map(Path::to_path_buf),
    ))
  }

  /// The fonts the command line names, after the default one.
  fn fonts(&self) -> Result<Fonts, Box<dyn Error>> {
    let mut fonts = Fonts::default();

    fonts.register(FontResource::new(DEFAULT_FONT))?;

    for path in &self.fonts {
      fonts
        .register(FontResource::new(fs::read(path)?))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    }

    Ok(fonts)
  }

  fn viewport(&self) -> Viewport {
    Viewport::new((self.width, self.height)).with_device_pixel_ratio(self.dpr)
  }

  /// Prints each resource the document reaches and whether the sandbox lets it load.
  fn list_resources(
    &self,
    output: &Output,
    document: &Document,
    sandbox: &Sandbox,
  ) -> Result<(), Box<dyn Error>> {
    let fonts = self.fonts()?;
    let inputs = RenderInputs {
      viewport: self.viewport(),
      images: Default::default(),
    };
    let needs = output.needs(&inputs, document, &fonts)?;

    for (kind, written) in needs
      .images
      .iter()
      .map(|url| ("image", url))
      .chain(needs.font_files.iter().map(|url| ("font", url)))
    {
      let verdict = match document.base.join(written) {
        Ok(url) => match sandbox.check(&url) {
          Ok(()) => format!("allowed  {url}"),
          Err(error) => format!("blocked  {error}"),
        },
        Err(error) => format!("invalid  {written}: {error}"),
      };

      println!("{kind:<5}  {verdict}");
    }

    Ok(())
  }
}
