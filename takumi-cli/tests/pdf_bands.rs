//! Renders `tests/fixtures/bands/page.html` through the `takumi pdf` binary with a header, a
//! footer, or both, and writes each PDF to `tests/fixtures-generated/`. The goldens pin which
//! band the CLI draws in which margin.

use std::{
  path::{Path, PathBuf},
  process::Command,
};

fn fixture(name: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("tests/fixtures/bands")
    .join(name)
}

fn render(golden: &str, bands: &[(&str, &str)]) {
  let output = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("tests/fixtures-generated")
    .join(format!("{golden}.pdf"));
  let mut command = Command::new(env!("CARGO_BIN_EXE_takumi"));

  command
    .arg("pdf")
    .arg(fixture("page.html"))
    .arg("-o")
    .arg(&output)
    .args(["--margin", "2cm 1cm"]);

  for (flag, file) in bands {
    command.arg(flag).arg(fixture(file));
  }

  let result = command.output().expect("runs takumi");

  assert!(
    result.status.success(),
    "{}",
    String::from_utf8_lossy(&result.stderr)
  );
}

#[test]
fn a_footer_alone_draws_in_the_bottom_margin() {
  render("bands-footer-only", &[("--footer", "footer.html")]);
}

#[test]
fn a_header_and_a_footer_draw_in_their_own_margins() {
  render(
    "bands-header-and-footer",
    &[("--header", "header.html"), ("--footer", "footer.html")],
  );
}
