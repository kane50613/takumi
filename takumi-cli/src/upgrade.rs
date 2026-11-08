//! `takumi upgrade`: reinstalls takumi from the source Cargo installed it from.

use std::{
  env,
  error::Error,
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use url::Url;

/// The package `cargo install` takes.
const PACKAGE: &str = "takumi-cli";

/// Reinstalls takumi from the source it was installed from, such as its Git branch.
#[derive(clap::Args)]
#[command(
  after_help = "takumi reads the source from the record `cargo install` keeps beside the
binary, and installs into the same place. It never checks for updates on its own."
)]
pub(crate) struct UpgradeArgs {
  /// Prints the command that would run, without running it.
  #[arg(short = 'n', long)]
  dry_run: bool,
}

/// Where `cargo install` installed takumi from.
#[derive(Debug, PartialEq)]
enum Source {
  /// A Git repository, following `branch`, or its default branch when none is named.
  Git { url: String, branch: Option<String> },
  /// A Git repository pinned to a tag or a commit, which an upgrade cannot move.
  PinnedGit { url: String, pin: String },
  /// crates.io.
  Registry,
  /// A local checkout.
  Path(PathBuf),
}

impl UpgradeArgs {
  pub(crate) fn run(&self) -> Result<(), Box<dyn Error>> {
    let executable = env::current_exe()?;
    let root = executable
      .parent()
      .and_then(Path::parent)
      .ok_or("cannot tell where takumi is installed")?;
    let source = installed_source(root)?;
    let arguments = install_arguments(&source, root)?;
    let shown = arguments
      .iter()
      .map(|argument| argument.to_string_lossy())
      .collect::<Vec<_>>()
      .join(" ");

    if self.dry_run {
      println!("cargo {shown}");
      return Ok(());
    }

    eprintln!("Running: cargo {shown}");

    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(&cargo)
      .args(&arguments)
      .status()
      .map_err(|error| {
        format!(
          "could not run {}: {error}. Install Rust from https://rustup.rs to upgrade takumi.",
          cargo.to_string_lossy()
        )
      })?;

    if !status.success() {
      return Err("`cargo install` failed, so takumi was not upgraded.".into());
    }

    Ok(())
  }
}

/// The source the `.crates2.json` record under `root` names for takumi.
fn installed_source(root: &Path) -> Result<Source, Box<dyn Error>> {
  let not_from_cargo = || -> Box<dyn Error> {
    "takumi was not installed with `cargo install`, so it cannot upgrade itself. Upgrade it the way you installed it.".into()
  };
  let record = fs::read_to_string(root.join(".crates2.json")).map_err(|_| not_from_cargo())?;
  let record: serde_json::Value = serde_json::from_str(&record)?;
  let installs = record
    .get("installs")
    .and_then(serde_json::Value::as_object)
    .ok_or_else(not_from_cargo)?;

  installs
    .iter()
    .find(|(id, install)| {
      id.split(' ').next() == Some(PACKAGE)
        && install
          .get("bins")
          .and_then(serde_json::Value::as_array)
          .is_some_and(|bins| bins.iter().any(|bin| bin == "takumi"))
    })
    .and_then(|(id, _)| Source::from_package_id(id))
    .ok_or_else(not_from_cargo)
}

impl Source {
  /// The source in a Cargo package ID such as `takumi-cli 0.1.0 (git+https://…?branch=main#abc)`.
  fn from_package_id(id: &str) -> Option<Self> {
    let source = id.split_once(" (")?.1.strip_suffix(')')?;

    if source.starts_with("registry+") || source.starts_with("sparse+") {
      return Some(Self::Registry);
    }

    if let Some(path) = source.strip_prefix("path+") {
      return Url::parse(path).ok()?.to_file_path().ok().map(Self::Path);
    }

    let location = source.strip_prefix("git+")?;
    let location = location
      .split_once('#')
      .map_or(location, |(location, _)| location);
    let mut url = Url::parse(location).ok()?;
    let query: Vec<(String, String)> = url.query_pairs().into_owned().collect();

    url.set_query(None);

    let url = url.to_string();

    Some(match query.into_iter().next() {
      Some((kind, name)) if kind == "branch" => Self::Git {
        url,
        branch: Some(name),
      },
      Some((kind, name)) => Self::PinnedGit {
        url,
        pin: format!("{kind} {name}"),
      },
      None => Self::Git { url, branch: None },
    })
  }
}

/// The `cargo install` arguments that reinstall takumi from `source` into `root`.
fn install_arguments(source: &Source, root: &Path) -> Result<Vec<OsString>, Box<dyn Error>> {
  let mut arguments: Vec<OsString> = vec!["install".into(), "--locked".into()];

  match source {
    Source::Git { url, branch } => {
      arguments.extend(["--git".into(), url.into()]);

      if let Some(branch) = branch {
        arguments.extend(["--branch".into(), branch.into()]);
      }
    }
    Source::PinnedGit { url, pin } => {
      return Err(format!("takumi was installed from {url} at {pin}, which does not move. To follow the default branch instead, run `cargo install --locked --git {url} {PACKAGE}`.").into());
    }
    Source::Registry => {}
    Source::Path(path) => {
      return Err(format!("takumi was installed from the local checkout at {}. Update it, then run `cargo install --locked --path {}`.", path.display(), path.display()).into());
    }
  }

  arguments.extend([PACKAGE.into(), "--root".into(), root.into()]);

  Ok(arguments)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_each_kind_of_source() {
    let cases = [
      (
        "takumi-cli 0.1.0 (git+https://github.com/kane50613/takumi?branch=feat/cli#93de0e3d)",
        Source::Git {
          url: "https://github.com/kane50613/takumi".into(),
          branch: Some("feat/cli".into()),
        },
      ),
      (
        "takumi-cli 0.1.0 (git+https://github.com/kane50613/takumi#93de0e3d)",
        Source::Git {
          url: "https://github.com/kane50613/takumi".into(),
          branch: None,
        },
      ),
      (
        "takumi-cli 0.1.0 (git+https://github.com/kane50613/takumi?tag=v0.1.0#93de0e3d)",
        Source::PinnedGit {
          url: "https://github.com/kane50613/takumi".into(),
          pin: "tag v0.1.0".into(),
        },
      ),
      (
        "takumi-cli 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
        Source::Registry,
      ),
    ];

    for (id, source) in cases {
      assert_eq!(Source::from_package_id(id), Some(source), "{id}");
    }
  }

  #[test]
  fn reinstalls_a_branch_into_the_same_root() {
    let arguments = install_arguments(
      &Source::Git {
        url: "https://github.com/kane50613/takumi".into(),
        branch: Some("main".into()),
      },
      Path::new("/opt/takumi"),
    )
    .unwrap();

    assert_eq!(
      arguments,
      [
        "install",
        "--locked",
        "--git",
        "https://github.com/kane50613/takumi",
        "--branch",
        "main",
        "takumi-cli",
        "--root",
        "/opt/takumi"
      ]
      .map(OsString::from)
    );
  }

  #[test]
  fn a_local_checkout_is_left_to_the_user() {
    assert!(install_arguments(&Source::Path("/src/takumi".into()), Path::new("/opt")).is_err());
  }
}
