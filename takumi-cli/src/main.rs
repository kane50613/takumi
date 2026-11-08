//! `takumi`: renders HTML and CSS to images and PDFs. See `DESIGN.md` for the conventions every
//! command follows, and [`sandbox`] for what a document may read.

mod document;
mod image;
mod input;
mod pdf;
mod product;
mod sandbox;
mod stylesheet;

use std::{error::Error, process::ExitCode};

use clap::{Parser, Subcommand};

use crate::{image::ImageArgs, pdf::PdfArgs, sandbox::ResourceError};

/// Renders HTML and CSS to images and PDFs.
#[derive(Parser)]
#[command(
  name = "takumi",
  version,
  infer_subcommands = false,
  after_help = "Docs and issues: https://github.com/kane50613/takumi"
)]
struct Cli {
  #[command(subcommand)]
  command: Command,

  /// Hides warnings, such as resources that failed to load.
  #[arg(short, long, global = true)]
  quiet: bool,
}

#[derive(Subcommand)]
enum Command {
  Image(ImageArgs),
  Pdf(PdfArgs),
}

fn main() -> ExitCode {
  let cli = Cli::parse();
  let result = match &cli.command {
    Command::Image(args) => args
      .product()
      .and_then(|image| product::run(&image, &args.input, &args.output, cli.quiet)),
    Command::Pdf(args) => args
      .product()
      .and_then(|pdf| product::run(&pdf, &args.input, &args.output, cli.quiet)),
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("error: {error}");
      exit_code(error.as_ref())
    }
  }
}

/// `3` when the sandbox denied a read, so a script can tell a policy failure from a broken
/// document, and `1` for any other failure. clap exits with `2` on a usage error.
fn exit_code(error: &(dyn Error + 'static)) -> ExitCode {
  match error.downcast_ref::<ResourceError>() {
    Some(ResourceError::Denied { .. }) => ExitCode::from(3),
    _ => ExitCode::FAILURE,
  }
}
