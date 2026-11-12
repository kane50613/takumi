//! The only way a document reaches outside itself: reads of local files under the granted roots
//! and `data:` URLs, within a budget. The network is not reachable at all.
//!
//! Each root is an open directory handle, and every read resolves relative to one through
//! `cap-std`, so neither `..`, a symlink, nor a file swapped for a symlink between a check and
//! the read can step outside it, and nothing outside a root is opened or probed.

use std::{
  cell::Cell,
  fmt,
  io::{self, Read},
  path::{self, Component, Path, PathBuf, Prefix},
};

use cap_std::{
  ambient_authority,
  fs::{Dir, OpenOptions},
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
  /// The file is named under a granted root, but a symlink in its path leads outside it.
  Escapes(Url),
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
  /// The resource loaded, but takumi cannot decode it as the image or font it should be.
  Undecodable { url: String, reason: String },
  /// A stylesheet that `@import` rules nest deeper than the CLI follows.
  ImportTooDeep(Url),
  /// The page and a header or footer write the same image URL for different files.
  SharedKey {
    written: String,
    kept: String,
    other: String,
  },
}

impl ResourceError {
  /// Whether the render stops: the sandbox denied the read or the budget ran out. Anything else
  /// the render skips, as a browser skips a resource that fails to load.
  pub(crate) fn fails_render(&self) -> bool {
    matches!(
      self,
      Self::Denied { .. } | Self::Escapes(_) | Self::OverBudget(_)
    )
  }

  /// The URL that failed to load, none for a failure that names no single URL.
  pub(crate) fn url(&self) -> Option<&Url> {
    match self {
      Self::Denied { url, .. }
      | Self::Escapes(url)
      | Self::Network(url)
      | Self::UnsupportedScheme(url)
      | Self::InvalidData(url)
      | Self::Io { url, .. }
      | Self::OverBudget(url)
      | Self::ImportTooDeep(url) => Some(url),
      Self::Undecodable { .. } | Self::SharedKey { .. } => None,
    }
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
      Self::Escapes(url) => write!(
        f,
        "{url} is an absolute symlink, or one that leads out of its readable directory, and takumi follows neither, even into another readable directory. Make the symlink relative, reference the file it points to, or pass a `--root` that holds both."
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
      Self::Undecodable { url, reason } => write!(f, "{url} could not be decoded: {reason}"),
      Self::ImportTooDeep(url) => write!(
        f,
        "{url} is nested deeper than {} `@import` levels, so takumi left it out.",
        crate::stylesheet::MAX_IMPORT_DEPTH
      ),
      Self::SharedKey {
        written,
        kept,
        other,
      } => write!(
        f,
        "`{written}` names {kept} in one document and {other} in another, so both draw {kept}. Give the files different names."
      ),
    }
  }
}

impl std::error::Error for ResourceError {}

/// A granted directory: its absolute path, to match a URL against by name, and its handle.
#[derive(Debug)]
struct Root {
  path: PathBuf,
  dir: Dir,
}

/// The directories a document may read and what it has read so far.
#[derive(Debug)]
pub(crate) struct Sandbox {
  roots: Vec<Root>,
  loaded_bytes: Cell<u64>,
  loaded_resources: Cell<usize>,
}

impl Sandbox {
  /// A sandbox reading under `roots`, each of which must be a directory.
  pub(crate) fn new(roots: impl IntoIterator<Item = PathBuf>) -> io::Result<Self> {
    let roots = roots
      .into_iter()
      .map(|root| {
        let with_path =
          |error: io::Error| io::Error::new(error.kind(), format!("{}: {error}", root.display()));
        let path = absolute_path(&root).map_err(with_path)?;
        let dir = Dir::open_ambient_dir(&path, ambient_authority()).map_err(with_path)?;

        Ok(Root { path, dir })
      })
      .collect::<io::Result<_>>()?;

    Ok(Self {
      roots,
      loaded_bytes: Cell::new(0),
      loaded_resources: Cell::new(0),
    })
  }

  /// The bytes `url` holds, if the sandbox lets the document read it.
  pub(crate) fn read(&self, url: &Url) -> Result<Vec<u8>, ResourceError> {
    let bytes = match url.scheme() {
      "data" => decode_data(url)?,
      "file" => {
        let mut bytes = Vec::new();

        self
          .open(url)?
          .take(MAX_RESOURCE_BYTES + 1)
          .read_to_end(&mut bytes)
          .map_err(|error| ResourceError::Io {
            url: url.clone(),
            error,
          })?;

        bytes
      }
      "http" | "https" => return Err(ResourceError::Network(url.clone())),
      _ => return Err(ResourceError::UnsupportedScheme(url.clone())),
    };

    self.charge(url, bytes.len() as u64)?;

    Ok(bytes)
  }

  /// Whether the sandbox would let the document read `url` on its own, without reading it.
  pub(crate) fn check(&self, url: &Url) -> Result<(), ResourceError> {
    let bytes = match url.scheme() {
      "data" => decode_data(url)?.len() as u64,
      "file" => self.regular_file(url)?.2,
      "http" | "https" => return Err(ResourceError::Network(url.clone())),
      _ => return Err(ResourceError::UnsupportedScheme(url.clone())),
    };

    if bytes > MAX_RESOURCE_BYTES {
      return Err(ResourceError::OverBudget(url.clone()));
    }

    Ok(())
  }

  /// The file `url` names, opened through the root whose path holds it. Only a regular file
  /// opens, so a FIFO or a device under a root cannot hold the render up.
  fn open(&self, url: &Url) -> Result<cap_std::fs::File, ResourceError> {
    let (root, relative, _) = self.regular_file(url)?;
    let mut options = OpenOptions::new();

    options.read(true);

    // A file swapped for a FIFO after the check above still cannot block the open.
    #[cfg(unix)]
    {
      use cap_std::fs::OpenOptionsExt;

      options.custom_flags(libc::O_NONBLOCK);
    }

    let file = root
      .dir
      .open_with(&relative, &options)
      .map_err(|error| open_error(url, error))?;

    if !file
      .metadata()
      .map_err(|error| open_error(url, error))?
      .is_file()
    {
      return Err(not_a_file(url));
    }

    Ok(file)
  }

  /// The root holding the regular file `url` names, the file's path inside it, and its size, read
  /// from its metadata without opening it. A URL naming another machine never reaches it.
  fn regular_file(&self, url: &Url) -> Result<(&Root, PathBuf, u64), ResourceError> {
    if url
      .host_str()
      .is_some_and(|host| !host.is_empty() && !host.eq_ignore_ascii_case("localhost"))
    {
      return Err(ResourceError::Network(url.clone()));
    }

    if has_illegal_encoded_byte(url) {
      return Err(ResourceError::Io {
        url: url.clone(),
        error: io::Error::new(
          io::ErrorKind::InvalidInput,
          "an encoded slash, backslash, or NUL cannot be part of a file name",
        ),
      });
    }

    let path = url
      .to_file_path()
      .ok()
      .and_then(|path| lexical(&path))
      .ok_or_else(|| ResourceError::UnsupportedScheme(url.clone()))?;
    let Some((root, relative)) = self
      .roots
      .iter()
      .find_map(|root| Some((root, path.strip_prefix(&root.path).ok()?.to_path_buf())))
      .filter(|(_, relative)| !relative.as_os_str().is_empty())
    else {
      return Err(ResourceError::Denied {
        url: url.clone(),
        path,
      });
    };

    let metadata = root
      .dir
      .metadata(&relative)
      .map_err(|error| open_error(url, error))?;

    if !metadata.is_file() {
      return Err(not_a_file(url));
    }

    Ok((root, relative, metadata.len()))
  }

  /// Counts `bytes` read from `url` against the budget, failing once it runs out.
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

/// Whether `url`'s path encodes a byte no file name can hold, a slash, a NUL, or on Windows a
/// backslash, which Chromium's `FileURLToFilePath` refuses.
/// <https://source.chromium.org/chromium/chromium/src/+/main:net/base/filename_util.cc>
fn has_illegal_encoded_byte(url: &Url) -> bool {
  let path = url.path().to_ascii_lowercase();

  path.contains("%2f") || path.contains("%00") || (cfg!(windows) && path.contains("%5c"))
}

/// The error for a failed open of `url` through a root. cap-std refuses a path that resolves
/// outside the root as permission denied with no OS error code, unlike a file the OS refuses,
/// except on FreeBSD, where a metadata read beneath the root returns the raw `ENOTCAPABLE`.
fn open_error(url: &Url, error: io::Error) -> ResourceError {
  let escapes = error.kind() == io::ErrorKind::PermissionDenied && error.raw_os_error().is_none();
  #[cfg(target_os = "freebsd")]
  let escapes = escapes || error.raw_os_error() == Some(libc::ENOTCAPABLE);

  if escapes {
    ResourceError::Escapes(url.clone())
  } else {
    ResourceError::Io {
      url: url.clone(),
      error,
    }
  }
}

/// The error for `url` naming something other than a regular file.
fn not_a_file(url: &Url) -> ResourceError {
  ResourceError::Io {
    url: url.clone(),
    error: io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"),
  }
}

/// The bytes a `data:` URL holds.
fn decode_data(url: &Url) -> Result<Vec<u8>, ResourceError> {
  DataUrl::process(url.as_str())
    .ok()
    .and_then(|data| data.decode_to_vec().ok())
    .map(|(bytes, _)| bytes)
    .ok_or_else(|| ResourceError::InvalidData(url.clone()))
}

/// `path` made absolute against the working directory, with `.` and `..` resolved by name.
pub(crate) fn absolute_path(path: &Path) -> io::Result<PathBuf> {
  lexical(&path::absolute(path)?).ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::InvalidInput,
      "names another machine or a device, which takumi does not read",
    )
  })
}

/// `path` with `.` and `..` resolved by name alone, none when it names another machine or a
/// device, as a Windows UNC path does.
fn lexical(path: &Path) -> Option<PathBuf> {
  let mut resolved = PathBuf::new();

  for component in path.components() {
    match component {
      Component::Prefix(prefix) => match prefix.kind() {
        Prefix::Disk(_) => resolved.push(component),
        // `\\?\C:\` names the same directory as `C:\`, which is how a `file:` URL names it.
        Prefix::VerbatimDisk(disk) => resolved.push(format!("{}:", char::from(disk))),
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
  Url::from_directory_path(absolute_path(directory)?)
    .map_err(|()| io::Error::new(io::ErrorKind::InvalidInput, "not an absolute path"))
}

#[cfg(test)]
mod tests {
  use std::fs;

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

    assert!(matches!(sandbox.read(&url), Err(ResourceError::Escapes(_))));
  }

  #[cfg(unix)]
  #[test]
  fn refuses_a_fifo_without_blocking() {
    let root = temp_dir("fifo");
    let fifo = root.join("pipe.css");
    let _ = fs::remove_file(&fifo);
    let status = std::process::Command::new("mkfifo")
      .arg(&fifo)
      .status()
      .unwrap();

    assert!(status.success());

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let url = directory_url(&root).unwrap().join("pipe.css").unwrap();

    assert!(matches!(sandbox.read(&url), Err(ResourceError::Io { .. })));
    assert!(sandbox.check(&url).is_err());
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
  fn an_encoded_slash_names_no_directory() {
    let root = temp_dir("encoded-slash");

    fs::create_dir_all(root.join("dir")).unwrap();
    fs::write(root.join("dir/style.css"), "a{}").unwrap();

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let url = directory_url(&root)
      .unwrap()
      .join("dir%2Fstyle.css")
      .unwrap();

    assert!(matches!(sandbox.read(&url), Err(ResourceError::Io { .. })));
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
  fn checks_a_data_url_payload() {
    let sandbox = Sandbox::new([]).unwrap();

    assert!(matches!(
      sandbox.check(&Url::parse("DATA:image/png;base64,A").unwrap()),
      Err(ResourceError::InvalidData(_))
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
    assert!(matches!(
      sandbox.check(&url),
      Err(ResourceError::OverBudget(_))
    ));
  }

  #[cfg(unix)]
  #[test]
  fn an_unreadable_file_under_a_root_is_skipped_not_an_escape() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("unreadable");
    let path = root.join("locked.css");

    fs::write(&path, "a{}").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let url = directory_url(&root).unwrap().join("locked.css").unwrap();
    let result = sandbox.read(&url);

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

    // Root reads any file, so only an unprivileged run sees the denial.
    if let Err(error) = result {
      assert!(matches!(error, ResourceError::Io { .. }), "{error:?}");
      assert!(!error.fails_render());
    }
  }

  #[cfg(windows)]
  #[test]
  fn a_verbatim_disk_root_matches_its_file_urls() {
    assert_eq!(
      lexical(Path::new(r"\\?\C:\site\a.png")),
      Some(PathBuf::from(r"C:\site\a.png"))
    );
  }
}
