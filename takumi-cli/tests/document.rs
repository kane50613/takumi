//! Runs the `takumi` binary on HTML files that leave out `<html>` and `<body>`.

use std::{env, fs, process::Command};

mod common;

use common::TempDir;

#[test]
fn a_file_without_html_and_body_still_matches_body_rules() {
  let dir = TempDir::new("document");

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

/// A 1×1 red PNG.
const RED: &[u8] = &[
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0,
  0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 0, 3, 1, 1, 0,
  201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

/// A 1×1 blue PNG.
const BLUE: &[u8] = &[
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0,
  0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 96, 96, 248, 15, 0, 1, 3, 1, 0,
  8, 137, 194, 236, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn a_page_and_its_header_keep_their_own_image_of_one_name() {
  let dir = TempDir::new("shared-src");

  fs::create_dir_all(dir.join("bands")).unwrap();
  fs::write(dir.join("logo.png"), RED).unwrap();
  fs::write(dir.join("bands/logo.png"), BLUE).unwrap();
  fs::write(dir.join("page.html"), r#"<img src="logo.png">"#).unwrap();
  fs::write(dir.join("bands/header.html"), r#"<img src="logo.png">"#).unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .arg("pdf")
    .arg(dir.join("page.html"))
    .arg("--header")
    .arg(dir.join("bands/header.html"))
    .arg("-o")
    .arg(dir.join("out.pdf"))
    .output()
    .expect("runs takumi");
  let messages = String::from_utf8_lossy(&output.stderr);

  assert!(output.status.success(), "{messages}");
  assert!(messages.is_empty(), "{messages}");
}
