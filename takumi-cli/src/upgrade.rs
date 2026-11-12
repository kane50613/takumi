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

/// The two names Cargo records crates.io under.
const CRATES_IO_INDEX: &str = "https://github.com/rust-lang/crates.io-index";
const CRATES_IO_SPARSE_INDEX: &str = "sparse+https://index.crates.io/";

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
  /// A registry: crates.io when `index` is none, or the registry at that index URL.
  Registry { index: Option<String> },
  /// A local checkout.
  Path(PathBuf),
}

impl UpgradeArgs {
  /// Reinstalls takumi from its recorded source, or prints the command with `--dry-run`.
  pub(crate) fn run(&self) -> Result<(), Box<dyn Error>> {
    let executable = env::current_exe()?;
    let root = executable
      .parent()
      .and_then(Path::parent)
      .ok_or("cannot tell where takumi is installed")?;
    let source = installed_source(root)?;
    let arguments = install_arguments(&source, root)?;
    let shown = command_line(&arguments);

    if self.dry_run {
      println!("cargo {shown}");
      return Ok(());
    }

    // Windows keeps a running executable locked, so `cargo install` cannot replace takumi.exe
    // while this process waits for it. <https://github.com/rust-lang/cargo/issues/3486>
    if cfg!(windows) {
      return Err(format!("Windows cannot replace takumi while it runs. Run `cargo {shown}` in PowerShell once this command exits.").into());
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
          .is_some_and(|bins| {
            bins
              .iter()
              .filter_map(serde_json::Value::as_str)
              .any(|bin| is_takumi(bin, env::consts::EXE_SUFFIX))
          })
    })
    .and_then(|(id, _)| Source::from_package_id(id))
    .ok_or_else(not_from_cargo)
}

/// Whether `bin`, a file name Cargo records, is takumi's executable on a platform whose
/// executables end in `suffix`, such as `.exe`.
fn is_takumi(bin: &str, suffix: &str) -> bool {
  bin.strip_suffix(suffix) == Some("takumi")
}

impl Source {
  /// The source in a Cargo package ID such as `takumi-cli 0.1.0 (git+https://…?branch=main#abc)`.
  fn from_package_id(id: &str) -> Option<Self> {
    let source = id.split_once(" (")?.1.strip_suffix(')')?;

    if let Some(index) = source.strip_prefix("registry+") {
      return Some(Self::Registry {
        index: (index != CRATES_IO_INDEX).then(|| index.to_owned()),
      });
    }

    if source.starts_with("sparse+") {
      return Some(Self::Registry {
        index: (source != CRATES_IO_SPARSE_INDEX).then(|| source.to_owned()),
      });
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
      let unpinned = Source::Git {
        url: url.clone(),
        branch: None,
      };
      let command = command_line(&install_arguments(&unpinned, root)?);

      return Err(format!("takumi was installed from {} at {pin}, which does not move. To follow the default branch instead, run `cargo {command}`.", without_credentials(url)).into());
    }
    // Without `--registry`, Cargo installs from a configured `registry.default`, not crates.io.
    Source::Registry { index } => arguments.extend(match index {
      Some(index) => ["--index".into(), index.into()],
      None => ["--registry".into(), "crates-io".into()],
    }),
    Source::Path(path) => {
      let command = command_line(&[
        "install".into(),
        "--locked".into(),
        "--path".into(),
        path.into(),
        "--root".into(),
        root.into(),
      ]);

      return Err(format!("takumi was installed from the local checkout at {}. Update it, then run `cargo {command}`.", path.display()).into());
    }
  }

  arguments.extend([PACKAGE.into(), "--root".into(), root.into()]);

  Ok(arguments)
}

/// `arguments` joined into one line to show, with any URL's credentials left out, that a POSIX
/// shell, or PowerShell on Windows, splits back into the same arguments.
fn command_line(arguments: &[OsString]) -> String {
  arguments
    .iter()
    .map(|argument| Shell::NATIVE.quote(&without_credentials(&argument.to_string_lossy())))
    .collect::<Vec<_>>()
    .join(" ")
}

/// `text` with the password of a URL it holds removed, and its user name unless it is an SSH
/// URL, so a shown command carries no credential.
fn without_credentials(text: &str) -> String {
  let Ok(mut url) = Url::parse(text) else {
    return text.to_owned();
  };
  // An SSH user name, such as GitHub's `git`, names the account to log in as, not a secret.
  let drops_username = !url.username().is_empty() && url.scheme() != "ssh";

  if !drops_username && url.password().is_none() {
    return text.to_owned();
  }

  if drops_username {
    let _ = url.set_username("");
  }

  let _ = url.set_password(None);
  url.to_string()
}

/// The shell a shown command is quoted for.
#[derive(Clone, Copy)]
enum Shell {
  /// A POSIX shell such as bash or zsh.
  Posix,
  /// PowerShell, whose single quotes expand nothing, so `$` and backticks stay.
  /// <https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_quoting_rules>
  PowerShell,
}

/// The characters PowerShell reads as a single quote.
const POWERSHELL_SINGLE_QUOTES: [char; 5] = ['\'', '\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'];

impl Shell {
  /// The shell takumi quotes shown commands for on this platform.
  const NATIVE: Self = if cfg!(windows) {
    Self::PowerShell
  } else {
    Self::Posix
  };

  /// `argument` as one word that this shell reads back unchanged.
  fn quote(self, argument: &str) -> String {
    let plain = !argument.is_empty()
      && argument
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:=@+,".contains(c));

    match self {
      Self::Posix if plain => argument.to_owned(),
      Self::Posix => format!("'{}'", argument.replace('\'', r"'\''")),
      // A leading `@` splats a variable, and a comma joins words into an array.
      Self::PowerShell if plain && !argument.starts_with('@') && !argument.contains(',') => {
        argument.to_owned()
      }
      Self::PowerShell => {
        let mut quoted = String::from("'");

        for c in argument.chars() {
          if POWERSHELL_SINGLE_QUOTES.contains(&c) {
            quoted.push(c);
          }

          quoted.push(c);
        }

        quoted.push('\'');
        quoted
      }
    }
  }
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
        Source::Registry { index: None },
      ),
      (
        "takumi-cli 0.1.0 (sparse+https://index.crates.io/)",
        Source::Registry { index: None },
      ),
      (
        "takumi-cli 0.1.0 (sparse+https://crates.example.com/index/)",
        Source::Registry {
          index: Some("sparse+https://crates.example.com/index/".into()),
        },
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

  #[cfg(not(windows))]
  #[test]
  fn a_local_checkout_is_left_to_the_user_with_the_same_root() {
    let error = install_arguments(
      &Source::Path("/src/takumi".into()),
      Path::new("/opt/my tools"),
    )
    .unwrap_err()
    .to_string();

    assert!(
      error.contains("`cargo install --locked --path /src/takumi --root '/opt/my tools'`"),
      "{error}"
    );
  }

  #[test]
  fn quotes_what_a_posix_shell_would_split() {
    assert_eq!(Shell::Posix.quote("/opt/it's here"), r"'/opt/it'\''s here'");
    assert_eq!(Shell::Posix.quote(""), "''");
    assert_eq!(Shell::Posix.quote("@release,next"), "@release,next");
  }

  #[test]
  fn a_powershell_quote_expands_nothing() {
    assert_eq!(
      Shell::PowerShell.quote(r"C:\tools\$build"),
      r"'C:\tools\$build'"
    );
    assert_eq!(Shell::PowerShell.quote("it's"), "'it''s'");
    assert_eq!(
      Shell::PowerShell.quote("C:\\Users\\O\u{2019}Connor\\.cargo"),
      "'C:\\Users\\O\u{2019}\u{2019}Connor\\.cargo'"
    );
  }

  #[test]
  fn powershell_quotes_a_splat_or_an_array() {
    assert_eq!(Shell::PowerShell.quote("@release"), "'@release'");
    assert_eq!(Shell::PowerShell.quote("a,b"), "'a,b'");
    assert_eq!(Shell::PowerShell.quote("--branch"), "--branch");
  }

  #[test]
  fn names_takumi_with_the_platform_suffix() {
    assert!(is_takumi("takumi", ""));
    assert!(is_takumi("takumi.exe", ".exe"));
    assert!(!is_takumi("takumi", ".exe"));
    assert!(!is_takumi("takumi-other", ""));
  }

  #[test]
  fn reinstalls_from_crates_io_by_name() {
    let arguments =
      install_arguments(&Source::Registry { index: None }, Path::new("/opt/takumi")).unwrap();

    assert_eq!(
      arguments,
      [
        "install",
        "--locked",
        "--registry",
        "crates-io",
        "takumi-cli",
        "--root",
        "/opt/takumi"
      ]
      .map(OsString::from)
    );
  }

  #[test]
  fn a_shown_command_leaves_out_credentials() {
    assert_eq!(
      command_line(&[
        "--git".into(),
        "https://user:secret@example.com/takumi".into()
      ]),
      "--git https://example.com/takumi"
    );
  }

  #[test]
  fn a_shown_ssh_command_keeps_its_user_name() {
    assert_eq!(
      without_credentials("ssh://git@github.com/kane50613/takumi"),
      "ssh://git@github.com/kane50613/takumi"
    );
    assert_eq!(
      without_credentials("ssh://git:secret@github.com/kane50613/takumi"),
      "ssh://git@github.com/kane50613/takumi"
    );
  }
}
