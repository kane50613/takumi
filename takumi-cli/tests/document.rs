//! Runs the `takumi` binary on HTML files that leave out `<html>` and `<body>`.

use std::{
  env, fs,
  process::{self, Command},
};

#[test]
fn a_file_without_html_and_body_still_matches_body_rules() {
  let dir = env::temp_dir().join(format!("takumi-cli-document-{}", process::id()));

  fs::create_dir_all(&dir).unwrap();
  fs::write(dir.join("brand.woff2"), b"").unwrap();
  fs::write(
    dir.join("card.html"),
    "<style>@font-face { font-family: Brand; src: url(brand.woff2) } body { font-family: Brand }</style><p>Hello</p>",
  )
  .unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .args(["image", "--list-resources"])
    .arg(dir.join("card.html"))
    .output()
    .expect("runs takumi");
  let listing = String::from_utf8_lossy(&output.stdout);

  assert!(output.status.success());
  assert!(listing.contains("brand.woff2"), "{listing}");
}
