//! `takumi`: renders HTML and CSS to images and PDFs. See `DESIGN.md` for the conventions every
//! command follows, and [`sandbox`] for what a document may read.

mod completions;
mod document;
mod image;
mod input;
mod logger;
mod paint;
mod pdf;
mod product;
mod sandbox;
mod stylesheet;
mod upgrade;

use std::{error::Error, process::ExitCode};

use clap::{CommandFactory, Parser, Subcommand};

use crate::{
  completions::CompletionsArgs,
  image::ImageArgs,
  input::UsageError,
  logger::{LogLevel, Logger},
  paint::PaintArgs,
  pdf::PdfArgs,
  sandbox::ResourceError,
  upgrade::UpgradeArgs,
};

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

  /// Prints what the render does to standard error: `info` for a line per stage, `debug` for
  /// each resource, page, and missing font or image as well.
  #[arg(long, global = true, value_name = "LEVEL")]
  log: Option<LogLevel>,

  /// Fails when the command raises a warning, and writes no output.
  #[arg(long, global = true)]
  deny_warnings: bool,
}

#[derive(Subcommand)]
enum Command {
  Image(ImageArgs),
  Pdf(PdfArgs),
  Paint(PaintArgs),
  Upgrade(UpgradeArgs),
  Completions(CompletionsArgs),
}

fn main() -> ExitCode {
  let cli = Cli::parse();

  if let Err(error) = Logger::install(cli.log, cli.quiet) {
    eprintln!("error: {error}");
    return ExitCode::FAILURE;
  }

  let result = match &cli.command {
    Command::Image(args) => args
      .product()
      .and_then(|image| product::run(&image, &args.input, &args.output, cli.deny_warnings)),
    Command::Pdf(args) => args
      .product()
      .and_then(|pdf| product::run(&pdf, &args.input, &args.output, cli.deny_warnings)),
    Command::Paint(args) => product::run(
      &args.product(),
      &args.input,
      &args.output,
      cli.deny_warnings,
    ),
    Command::Upgrade(args) => args.run(),
    Command::Completions(args) => args.run(Cli::command()),
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("error: {error}");
      exit_code(error.as_ref())
    }
  }
}

/// `2` for a usage error, as clap exits with on a malformed command line, `3` when the sandbox
/// denied a read, so a script can tell a policy failure from a broken document, and `1` for any
/// other failure.
fn exit_code(error: &(dyn Error + 'static)) -> ExitCode {
  if error.is::<UsageError>() {
    return ExitCode::from(2);
  }

  match error.downcast_ref::<ResourceError>() {
    Some(ResourceError::Denied { .. } | ResourceError::Escapes(_)) => ExitCode::from(3),
    _ => ExitCode::FAILURE,
  }
}
