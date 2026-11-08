//! The flags every product shares: the input and what it may read, and where the output goes.

use std::{
  env,
  error::Error,
  fs,
  io::{self, IsTerminal, Read, Write},
  path::{Path, PathBuf},
};

use takumi::prelude::{FontResource, Fonts};
use url::Url;

use crate::sandbox::{Sandbox, absolute_path, directory_url};

/// The font text falls back to when the document loads none of its own.
const DEFAULT_FONT: &[u8] = include_bytes!("../../assets/fonts/geist/Geist[wght].woff2");

/// The input document, the fonts the command line names, and the directories it may read.
#[derive(clap::Args)]
pub(crate) struct InputArgs {
  /// The HTML file to render, or `-` to read it from standard input.
  input: PathBuf,

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
}

/// The HTML an [`InputArgs`] names and the URL its relative references start from.
pub(crate) struct Input {
  pub(crate) html: String,
  pub(crate) location: Url,
  pub(crate) sandbox: Sandbox,
}

impl InputArgs {
  /// Reads the input and sets up the sandbox its document reads through.
  pub(crate) fn read(&self) -> Result<Input, Box<dyn Error>> {
    let (html, location, default_root) = if self.input == Path::new("-") {
      let mut stdin = io::stdin();

      if stdin.is_terminal() {
        return Err("the input is `-`, but standard input is a terminal. Pipe HTML into takumi or name a file.".into());
      }

      let mut html = String::new();

      stdin.read_to_string(&mut html)?;

      (html, directory_url(&env::current_dir()?)?, None)
    } else {
      let (html, location) = read_html_file(&self.input)?;
      let directory = location
        .to_file_path()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));

      (html, location, directory)
    };
    let roots = self.root.clone().or(default_root).into_iter();
    let sandbox = Sandbox::new(roots.chain(self.allow_read.iter().cloned()))?;

    Ok(Input {
      html,
      location,
      sandbox,
    })
  }

  /// The default font and the fonts the command line names.
  pub(crate) fn fonts(&self) -> Result<Fonts, Box<dyn Error>> {
    let mut fonts = Fonts::default();

    fonts.register(FontResource::new(DEFAULT_FONT))?;

    for path in &self.fonts {
      let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;

      fonts
        .register(FontResource::new(bytes))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    }

    Ok(fonts)
  }
}

/// The HTML of a file named on the command line, and its `file:` URL as named, so its relative
/// references resolve beside the name even when it is a symlink.
pub(crate) fn read_html_file(path: &Path) -> Result<(String, Url), Box<dyn Error>> {
  let with_path = |error: io::Error| format!("{}: {error}", path.display());
  let absolute = absolute_path(path).map_err(with_path)?;
  let location = Url::from_file_path(&absolute)
    .map_err(|()| format!("{} is not a local file", path.display()))?;

  Ok((fs::read_to_string(&absolute).map_err(with_path)?, location))
}

/// Where the rendered file goes, or a list of what it would load instead.
#[derive(clap::Args)]
pub(crate) struct OutputArgs {
  /// The file to write, or `-` for standard output.
  #[arg(short, long, required_unless_present = "list_resources")]
  output: Option<PathBuf>,

  /// Lists the files the document would load and whether it may, without rendering.
  #[arg(long)]
  pub(crate) list_resources: bool,
}

/// A command line that names something takumi cannot do, such as an output format it does not
/// write, as clap reports a malformed one.
#[derive(Debug)]
pub(crate) struct UsageError(pub(crate) String);

impl std::fmt::Display for UsageError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

impl Error for UsageError {}

impl OutputArgs {
  /// Whether the command writes a file rather than listing resources.
  pub(crate) fn renders(&self) -> bool {
    !self.list_resources
  }

  /// Whether the output goes to standard output.
  pub(crate) fn is_stdout(&self) -> bool {
    self.output.as_deref() == Some(Path::new("-"))
  }

  /// The lowercase extension of the output file, none for standard output or no output.
  pub(crate) fn extension(&self) -> Option<String> {
    let output = self.output.as_deref().filter(|_| !self.is_stdout())?;

    output.extension()?.to_str().map(str::to_ascii_lowercase)
  }

  /// Writes `bytes` to the output. `is_text` lets a terminal receive them.
  pub(crate) fn write(&self, bytes: &[u8], is_text: bool) -> Result<(), Box<dyn Error>> {
    let Some(output) = self.output.as_deref() else {
      return Err(UsageError("the command needs `-o` to write its output.".into()).into());
    };

    if !self.is_stdout() {
      return fs::write(output, bytes)
        .map_err(|error| format!("{}: {error}", output.display()).into());
    }

    let mut stdout = io::stdout().lock();

    if !is_text && stdout.is_terminal() {
      return Err("the output is binary and standard output is a terminal. Redirect it or pass a file to `-o`.".into());
    }

    stdout.write_all(bytes)?;

    Ok(())
  }
}
