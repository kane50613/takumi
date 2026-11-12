//! Runs `takumi image --list-resources` on documents whose files are missing or blocked.

use std::{
  env, fs,
  process::{self, Command},
};

#[test]
fn a_stylesheet_that_would_not_load_is_listed_as_blocked() {
  let dir = env::temp_dir().join(format!("takumi-cli-listing-{}", process::id()));
  let site = dir.join("site");

  fs::create_dir_all(&site).unwrap();
  fs::write(dir.join("outside.css"), "a{}").unwrap();
  fs::write(
    site.join("page.html"),
    r#"<link rel="stylesheet" href="../outside.css"><link rel="stylesheet" href="missing.css">"#,
  )
  .unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .args(["image", "--list-resources"])
    .arg(site.join("page.html"))
    .output()
    .expect("runs takumi");
  let listing = String::from_utf8_lossy(&output.stdout);

  assert!(output.status.success());
  assert!(
    listing.contains("stylesheet\tblocked\tfile://"),
    "{listing}"
  );
  assert!(listing.contains("/outside.css\t"), "{listing}");
  assert!(listing.contains("/site/missing.css\t"), "{listing}");
}
