//! Runs `takumi image --selector` and reads the size of the image it writes.

use std::{
  env, fs,
  process::{Command, Output},
};

mod common;

use common::TempDir;

const PAGE: &str = r#"<style>
  body { margin: 0 }
  header { height: 80px; background: #0f172a }
  .card { width: 280px; height: 120px; margin: 40px; background: #eef2ff }
</style>
<header></header><section id="pro" class="card plan"></section>"#;

fn image(name: &str, arguments: &[&str]) -> (Output, TempDir) {
  let dir = TempDir::new(&format!("selector-{name}"));

  fs::write(dir.join("page.html"), PAGE).unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .arg("image")
    .arg(dir.join("page.html"))
    .arg("-o")
    .arg(dir.join("out.png"))
    .args(arguments)
    .output()
    .expect("runs takumi");

  (output, dir)
}

/// The width and height a PNG's header records.
fn png_size(bytes: &[u8]) -> (u32, u32) {
  let number = |start: usize| u32::from_be_bytes(bytes[start..start + 4].try_into().unwrap());

  (number(16), number(20))
}

#[test]
fn the_image_is_the_element_at_its_own_size() {
  let (output, dir) = image("size", &["--selector", "section.card"]);

  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(
    png_size(&fs::read(dir.join("out.png")).unwrap()),
    (280, 120)
  );
}

#[test]
fn the_device_pixel_ratio_scales_the_element() {
  let (output, dir) = image("dpr", &["--selector", "#pro", "--dpr", "2"]);

  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(
    png_size(&fs::read(dir.join("out.png")).unwrap()),
    (560, 240)
  );
}

#[test]
fn no_match_fails_and_names_the_selector() {
  let (output, dir) = image("missing", &["--selector", ".missing"]);

  assert_eq!(output.status.code(), Some(1));
  assert!(!dir.join("out.png").exists());
  assert_eq!(
    String::from_utf8(output.stderr).unwrap(),
    "error: no element matches `--selector .missing`. An inline element with no background or border draws no box of its own, so give it `display: inline-block`.\n"
  );
}

#[test]
fn the_first_match_in_document_order_wins_over_paint_order() {
  let dir = TempDir::new("selector-order");
  let html = r#"<style>
  body { margin: 0 }
  .card { position: relative; background: #eef2ff }
</style>
<div class="card" style="z-index: 1; width: 100px; height: 50px"></div>
<div class="card" style="z-index: 0; width: 200px; height: 80px"></div>"#;

  fs::write(dir.join("page.html"), html).unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .arg("image")
    .arg(dir.join("page.html"))
    .arg("-o")
    .arg(dir.join("out.png"))
    .args(["--selector", ".card"])
    .output()
    .expect("runs takumi");

  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(png_size(&fs::read(dir.join("out.png")).unwrap()), (100, 50));
}
