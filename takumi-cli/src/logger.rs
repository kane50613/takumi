//! What `--log` prints: a line to standard error for each step of a render, with the time since
//! the command began.

use std::{collections::HashSet, sync::Mutex, time::Instant};

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

/// Writes takumi's own log records to standard error.
pub(crate) struct Logger {
  started: Instant,
  /// The lines below a warning already printed, since a render reaches the same one many times.
  printed: Mutex<HashSet<String>>,
}

impl Logger {
  /// Starts logging at `level`, or warnings alone without one, which `quiet` hides.
  pub(crate) fn install(level: Option<LogLevel>, quiet: bool) -> Result<(), SetLoggerError> {
    let filter = match level {
      Some(LogLevel::Debug) => LevelFilter::Debug,
      Some(LogLevel::Info) => LevelFilter::Info,
      None if quiet => LevelFilter::Off,
      None => LevelFilter::Warn,
    };

    log::set_logger(Box::leak(Box::new(Self {
      started: Instant::now(),
      printed: Mutex::default(),
    })))?;
    log::set_max_level(filter);

    Ok(())
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
      eprintln!("warning: {}", record.args());
      return;
    }

    let line = format!("{stage}: {}", record.args());

    if record.level() == Level::Debug
      && let Ok(mut printed) = self.printed.lock()
      && !printed.insert(line.clone())
    {
      return;
    }

    eprintln!(
      "[{:>8.1}ms] {line}",
      self.started.elapsed().as_secs_f64() * 1000.0
    );
  }

  fn flush(&self) {}
}
