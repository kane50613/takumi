//! The only way a document reaches outside itself: reads of local files under the granted roots
//! and `data:` URLs, within a budget. The network is not reachable at all.

use std::{
  cell::Cell,
  fmt, fs,
  io::{self, Read},
  path::{self, Component, Path, PathBuf, Prefix},
};

use data_url::DataUrl;
use url::Url;

/// The most bytes one resource may hold.
const MAX_RESOURCE_BYTES: u64 = 32 << 20;

/// The most bytes all the resources of one render may hold together.
const MAX_TOTAL_BYTES: u64 = 256 << 20;

/// The most resources one render may load.
const MAX_RESOURCES: usize = 1024;

/// Why a resource did not load.
#[derive(Debug)]
pub(crate) enum ResourceError {
  /// The file is outside every granted root.
  Denied { url: Url, path: PathBuf },
  /// The URL needs the network, which the CLI never reaches.
  Network(Url),
  /// The URL's scheme names nothing the CLI can load.
  UnsupportedScheme(Url),
  /// A `data:` URL that does not decode.
  InvalidData(Url),
  /// The file is missing or unreadable.
  Io { url: Url, error: io::Error },
  /// The resource, or all of them together, went past the budget.
  OverBudget(Url),
}

impl ResourceError {
  /// Whether the render stops: the sandbox denied the read or the budget ran out. Anything else
  /// the render skips, as a browser skips a resource that fails to load.
  pub(crate) fn fails_render(&self) -> bool {
    matches!(self, Self::Denied { .. } | Self::OverBudget(_))
  }
}

impl fmt::Display for ResourceError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Denied { url, path } => write!(
        f,
        "{url} is outside the readable directories. Pass `--allow-read {}` to allow it.",
        path.parent().unwrap_or(path).display()
      ),
      Self::Network(url) => write!(f, "{url} needs the network, which takumi does not reach."),
      Self::UnsupportedScheme(url) => write!(f, "{url} has a scheme takumi cannot load."),
      Self::InvalidData(url) => write!(f, "{url} is not a valid data: URL."),
      Self::Io { url, error } => write!(f, "{url} could not be read: {error}"),
      Self::OverBudget(url) => write!(
        f,
        "{url} goes past the limit of {} MiB per resource, {} MiB in total, or {MAX_RESOURCES} resources.",
        MAX_RESOURCE_BYTES >> 20,
        MAX_TOTAL_BYTES >> 20
      ),
    }
  }
}

impl std::error::Error for ResourceError {}

/// The directories a document may read and what it has read so far.
#[derive(Debug)]
pub(crate) struct Sandbox {
  /// Canonical paths, so a symlink or `..` cannot step out of one.
  roots: Vec<PathBuf>,
  /// The roots as given, made absolute, to check a path against before touching the filesystem.
  given_roots: Vec<PathBuf>,
  loaded_bytes: Cell<u64>,
  loaded_resources: Cell<usize>,
}

impl Sandbox {
  /// A sandbox reading under `roots`, each of which must exist.
  pub(crate) fn new(roots: impl IntoIterator<Item = PathBuf>) -> io::Result<Self> {
    let mut canonical_roots = Vec::new();
    let mut given_roots = Vec::new();

    for root in roots {
      let with_path =
        |error: io::Error| io::Error::new(error.kind(), format!("{}: {error}", root.display()));

      canonical_roots.push(fs::canonicalize(&root).map_err(with_path)?);
      given_roots.extend(lexical(&path::absolute(&root).map_err(with_path)?));
    }

    Ok(Self {
      roots: canonical_roots,
      given_roots,
      loaded_bytes: Cell::new(0),
      loaded_resources: Cell::new(0),
    })
  }

  /// The bytes `url` holds, if the sandbox lets the document read it.
  pub(crate) fn read(&self, url: &Url) -> Result<Vec<u8>, ResourceError> {
    let bytes = match url.scheme() {
      "data" => self.read_data(url)?,
      "file" => self.read_file(url)?,
      "http" | "https" => return Err(ResourceError::Network(url.clone())),
      _ => return Err(ResourceError::UnsupportedScheme(url.clone())),
    };

    self.charge(url, bytes.len() as u64)?;

    Ok(bytes)
  }

  /// Whether the sandbox would let the document read `url`, without reading it.
  pub(crate) fn check(&self, url: &Url) -> Result<(), ResourceError> {
    match url.scheme() {
      "data" => DataUrl::process(url.as_str())
        .map(drop)
        .map_err(|_| ResourceError::InvalidData(url.clone())),
      "file" => self.allowed_path(url).map(drop),
      "http" | "https" => Err(ResourceError::Network(url.clone())),
      _ => Err(ResourceError::UnsupportedScheme(url.clone())),
    }
  }

  fn read_data(&self, url: &Url) -> Result<Vec<u8>, ResourceError> {
    DataUrl::process(url.as_str())
      .ok()
      .and_then(|data| data.decode_to_vec().ok())
      .map(|(bytes, _)| bytes)
      .ok_or_else(|| ResourceError::InvalidData(url.clone()))
  }

  fn read_file(&self, url: &Url) -> Result<Vec<u8>, ResourceError> {
    let path = self.allowed_path(url)?;
    let io_error = |error| ResourceError::Io {
      url: url.clone(),
      error,
    };
    let mut bytes = Vec::new();

    fs::File::open(&path)
      .and_then(|file| file.take(MAX_RESOURCE_BYTES + 1).read_to_end(&mut bytes))
      .map_err(io_error)?;

    Ok(bytes)
  }

  /// The canonical path `url` names, if it lies under a root. The path is checked as written
  /// before the filesystem is touched, so a path outside every root is never probed, and a URL
  /// naming another machine never reaches it.
  fn allowed_path(&self, url: &Url) -> Result<PathBuf, ResourceError> {
    if url
      .host_str()
      .is_some_and(|host| !host.is_empty() && !host.eq_ignore_ascii_case("localhost"))
    {
      return Err(ResourceError::Network(url.clone()));
    }

    let path = url
      .to_file_path()
      .ok()
      .and_then(|path| lexical(&path))
      .ok_or_else(|| ResourceError::UnsupportedScheme(url.clone()))?;
    let denied = |path: PathBuf| ResourceError::Denied {
      url: url.clone(),
      path,
    };

    if !self
      .given_roots
      .iter()
      .chain(&self.roots)
      .any(|root| path.starts_with(root))
    {
      return Err(denied(path));
    }

    let canonical = fs::canonicalize(&path).map_err(|error| ResourceError::Io {
      url: url.clone(),
      error,
    })?;

    if !self.roots.iter().any(|root| canonical.starts_with(root)) {
      return Err(denied(canonical));
    }

    if !canonical.is_file() {
      return Err(ResourceError::Io {
        url: url.clone(),
        error: io::Error::new(io::ErrorKind::InvalidInput, "not a file"),
      });
    }

    Ok(canonical)
  }

  fn charge(&self, url: &Url, bytes: u64) -> Result<(), ResourceError> {
    let total = self.loaded_bytes.get() + bytes;
    let count = self.loaded_resources.get() + 1;

    if bytes > MAX_RESOURCE_BYTES || total > MAX_TOTAL_BYTES || count > MAX_RESOURCES {
      return Err(ResourceError::OverBudget(url.clone()));
    }

    self.loaded_bytes.set(total);
    self.loaded_resources.set(count);

    Ok(())
  }
}

/// `path` with `.` and `..` resolved by name alone, none when it names another machine or a
/// device, as a Windows UNC path does.
fn lexical(path: &Path) -> Option<PathBuf> {
  let mut resolved = PathBuf::new();

  for component in path.components() {
    match component {
      Component::Prefix(prefix) => match prefix.kind() {
        Prefix::Disk(_) | Prefix::VerbatimDisk(_) => resolved.push(component),
        Prefix::UNC(..) | Prefix::VerbatimUNC(..) | Prefix::Verbatim(_) | Prefix::DeviceNS(_) => {
          return None;
        }
      },
      Component::RootDir => resolved.push(component),
      Component::CurDir => {}
      Component::ParentDir => {
        resolved.pop();
      }
      Component::Normal(name) => resolved.push(name),
    }
  }

  Some(resolved)
}

/// The `file:` URL of a directory, ending in a slash so relative references resolve inside it.
pub(crate) fn directory_url(directory: &Path) -> io::Result<Url> {
  let canonical = fs::canonicalize(directory)?;

  Url::from_directory_path(&canonical)
    .map_err(|()| io::Error::new(io::ErrorKind::InvalidInput, "not an absolute path"))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("takumi-cli-{name}-{}", std::process::id()));

    fs::create_dir_all(&dir).unwrap();

    dir
  }

  #[test]
  fn reads_a_file_under_a_root() {
    let root = temp_dir("under-root");

    fs::write(root.join("a.css"), "a{}").unwrap();

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let url = directory_url(&root).unwrap().join("a.css").unwrap();

    assert_eq!(sandbox.read(&url).unwrap(), b"a{}");
  }

  #[test]
  fn denies_a_file_outside_every_root() {
    let root = temp_dir("outside-root");
    let outside = temp_dir("outside-other");

    fs::write(outside.join("secret.txt"), "secret").unwrap();

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let url = directory_url(&root)
      .unwrap()
      .join(&format!(
        "../{}/secret.txt",
        outside.file_name().unwrap().to_string_lossy()
      ))
      .unwrap();

    assert!(matches!(
      sandbox.read(&url),
      Err(ResourceError::Denied { .. })
    ));
  }

  #[cfg(unix)]
  #[test]
  fn denies_a_symlink_that_points_outside_a_root() {
    let root = temp_dir("symlink-root");
    let outside = temp_dir("symlink-outside");

    fs::write(outside.join("secret.txt"), "secret").unwrap();
    let _ = fs::remove_file(root.join("link.txt"));
    std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.txt")).unwrap();

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let url = directory_url(&root).unwrap().join("link.txt").unwrap();

    assert!(matches!(
      sandbox.read(&url),
      Err(ResourceError::Denied { .. })
    ));
  }

  #[test]
  fn never_probes_a_missing_file_outside_every_root() {
    let root = temp_dir("probe-root");
    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let url = directory_url(&root)
      .unwrap()
      .join("../takumi-cli-no-such-file")
      .unwrap();

    assert!(matches!(
      sandbox.read(&url),
      Err(ResourceError::Denied { .. })
    ));
  }

  #[test]
  fn a_file_url_naming_another_machine_never_reaches_it() {
    let sandbox = Sandbox::new([]).unwrap();

    assert!(matches!(
      sandbox.read(&Url::parse("file://attacker.example/share/a.png").unwrap()),
      Err(ResourceError::Network(_))
    ));
  }

  #[test]
  fn never_reaches_the_network() {
    let sandbox = Sandbox::new([]).unwrap();

    for url in ["https://example.com/a.css", "http://169.254.169.254/latest"] {
      assert!(matches!(
        sandbox.read(&Url::parse(url).unwrap()),
        Err(ResourceError::Network(_))
      ));
    }
    assert!(matches!(
      sandbox.read(&Url::parse("ftp://example.com/a.css").unwrap()),
      Err(ResourceError::UnsupportedScheme(_))
    ));
  }

  #[test]
  fn decodes_a_data_url_with_no_root() {
    let sandbox = Sandbox::new([]).unwrap();
    let url = Url::parse("data:text/css;base64,YXt9").unwrap();

    assert_eq!(sandbox.read(&url).unwrap(), b"a{}");
  }

  #[test]
  fn a_file_past_the_size_limit_is_over_budget() {
    let root = temp_dir("budget");
    let path = root.join("big.bin");

    fs::File::create(&path)
      .unwrap()
      .set_len(MAX_RESOURCE_BYTES + 1)
      .unwrap();

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let url = directory_url(&root).unwrap().join("big.bin").unwrap();

    assert!(matches!(
      sandbox.read(&url),
      Err(ResourceError::OverBudget(_))
    ));
  }
}
