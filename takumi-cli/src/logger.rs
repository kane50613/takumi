//! What `--log` prints: a line to standard error for each step of a render, with the time since
//! the command began.

use std::{
  collections::HashSet,
  error::Error,
  fmt,
  sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
  },
  time::Instant,
};

use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

/// The prefix of every target takumi logs under, which the stage name follows.
const TARGET_PREFIX: &str = "takumi::";

/// How much of a render `--log` prints.
#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum LogLevel {
  /// One line per stage: the input, its resources, the render, the output.
  Info,
  /// Each resource loaded, each page, and each font family or image the render found missing.
  Debug,
}

/// The distinct warnings the command has raised, which `-q` hides and still counts.
static WARNINGS: AtomicUsize = AtomicUsize::new(0);

/// The failure `--deny-warnings` turns a command's warnings into.
#[derive(Debug)]
pub(crate) struct DeniedWarnings(usize);

impl fmt::Display for DeniedWarnings {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "`--deny-warnings` is set and the command raised {} {}, so it wrote nothing. Fix each one, or drop the flag to write the output anyway.",
      self.0,
      if self.0 == 1 { "warning" } else { "warnings" }
    )
  }
}

impl Error for DeniedWarnings {}

/// Writes takumi's own log records to standard error.
pub(crate) struct Logger {
  started: Instant,
  /// Whether `-q` hides warnings, which `--log` leaves on.
  quiet: bool,
  /// The warnings and `debug` lines already printed, since a render reaches the same one many times.
  printed: Mutex<HashSet<String>>,
}

impl Logger {
  /// Starts logging at `level`, or warnings alone without one, which `quiet` hides.
  pub(crate) fn install(level: Option<LogLevel>, quiet: bool) -> Result<(), SetLoggerError> {
    let filter = match level {
      Some(LogLevel::Debug) => LevelFilter::Debug,
      Some(LogLevel::Info) => LevelFilter::Info,
      None => LevelFilter::Warn,
    };

    log::set_logger(Box::leak(Box::new(Self {
      started: Instant::now(),
      quiet,
      printed: Mutex::default(),
    })))?;
    log::set_max_level(filter);

    Ok(())
  }

  /// Fails when `deny` is set and the command has raised a warning.
  pub(crate) fn deny_warnings(deny: bool) -> Result<(), DeniedWarnings> {
    match WARNINGS.load(Ordering::Relaxed) {
      count if deny && count > 0 => Err(DeniedWarnings(count)),
      _ => Ok(()),
    }
  }

  /// Whether `line` has not been printed before.
  fn is_new(&self, line: &str) -> bool {
    self
      .printed
      .lock()
      .is_ok_and(|mut printed| printed.insert(line.to_owned()))
  }
}

impl Log for Logger {
  fn enabled(&self, metadata: &Metadata<'_>) -> bool {
    metadata.target().starts_with(TARGET_PREFIX)
  }

  fn log(&self, record: &Record<'_>) {
    let Some(stage) = record.target().strip_prefix(TARGET_PREFIX) else {
      return;
    };

    if record.level() <= Level::Warn {
      let warning = record.args().to_string();

      if self.is_new(&warning) {
        WARNINGS.fetch_add(1, Ordering::Relaxed);

        if !self.quiet {
          eprintln!("warning: {warning}");
        }
      }

      return;
    }

    let line = format!("{stage}: {}", record.args());

    if record.level() == Level::Debug && !self.is_new(&line) {
      return;
    }

    eprintln!(
      "[{:>8.1}ms] {line}",
      self.started.elapsed().as_secs_f64() * 1000.0
    );
  }

  fn flush(&self) {}
}
