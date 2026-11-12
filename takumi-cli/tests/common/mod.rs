//! What the integration tests share.

use std::{
  env, fs,
  path::{Path, PathBuf},
  process,
};

/// A directory of the system's temporary directory, empty when made and removed when dropped, so
/// no run reads what an earlier one left.
pub struct TempDir(PathBuf);

impl TempDir {
  pub fn new(name: &str) -> Self {
    let path = env::temp_dir().join(format!("takumi-cli-{name}-{}", process::id()));

    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    Self(path)
  }

  pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
    self.0.join(path)
  }
}

impl Drop for TempDir {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}
