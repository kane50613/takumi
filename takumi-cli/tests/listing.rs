//! Runs `takumi image` on documents whose files are missing or blocked, or whose text no font
//! draws.

use std::{env, fs, process::Command};

mod common;

use common::TempDir;

#[test]
fn a_file_that_would_not_load_is_listed_as_blocked_or_missing() {
  let dir = TempDir::new("listing");
  let site = dir.join("site");

  fs::create_dir_all(&site).unwrap();
  fs::write(dir.join("outside.css"), "a{}").unwrap();
  fs::write(
    site.join("page.html"),
    r#"<link rel="stylesheet" href="../outside.css"><link rel="stylesheet" href="missing.css"><img src="gone.png">"#,
  )
  .unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .args(["image", "--list-resources"])
    .arg(site.join("page.html"))
    .output()
    .expect("runs takumi");
  let listing = String::from_utf8_lossy(&output.stdout);

  assert!(output.status.success());
  let status_of = |file: &str| {
    listing
      .lines()
      .find(|line| line.contains(file))
      .and_then(|line| line.split('\t').nth(1))
      .map(str::to_owned)
  };

  assert_eq!(
    status_of("/outside.css").as_deref(),
    Some("blocked"),
    "{listing}"
  );
  assert_eq!(
    status_of("/site/missing.css").as_deref(),
    Some("missing"),
    "{listing}"
  );
  assert_eq!(
    status_of("/site/gone.png").as_deref(),
    Some("missing"),
    "{listing}"
  );
}

#[test]
fn an_image_warns_of_the_characters_no_font_draws() {
  let dir = TempDir::new("uncovered");

  fs::write(dir.join("page.html"), "<p>Hello \u{4F60}\u{597D}</p>").unwrap();

  let run = |quiet: bool| {
    let mut command = Command::new(env!("CARGO_BIN_EXE_takumi"));

    command
      .arg("image")
      .arg(dir.join("page.html"))
      .arg("-o")
      .arg(dir.join("page.png"));
    if quiet {
      command.arg("-q");
    }

    let output = command.output().expect("runs takumi");

    assert!(output.status.success());
    String::from_utf8_lossy(&output.stderr).into_owned()
  };
  let warning = run(false);

  assert!(
    warning.contains("warning: no font draws \u{4F60} (U+4F60), \u{597D} (U+597D)"),
    "{warning}"
  );
  assert!(warning.contains("`--font`"), "{warning}");
  assert!(!warning.contains("H (U+"), "{warning}");
  assert_eq!(run(true), "");
}

#[test]
fn an_image_named_as_a_pdf_points_to_the_pdf_command() {
  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .args(["image", "page.html", "-o", "page.pdf"])
    .output()
    .expect("runs takumi");

  assert_eq!(output.status.code(), Some(2));
  assert!(String::from_utf8_lossy(&output.stderr).contains("Use `takumi pdf`"));
}

#[test]
fn an_svg_refuses_a_device_pixel_ratio() {
  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .args(["image", "page.html", "-o", "page.svg", "--dpr", "2"])
    .output()
    .expect("runs takumi");

  assert_eq!(output.status.code(), Some(2));
  assert!(String::from_utf8_lossy(&output.stderr).contains("Drop `--dpr`"));
}
