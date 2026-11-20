//! Renders `tests/fixtures/bands/page.html` through the `takumi pdf` binary with a header, a
//! footer, or both, and writes each PDF to `tests/fixtures-generated/`. The goldens pin which
//! band the CLI draws in which margin.

use std::{
  fs,
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

  let mut pdf = fs::read(&output).expect("reads the PDF");

  zero_producer_version(&mut pdf);
  fs::write(&output, pdf).expect("writes the PDF");
}

/// Writes each digit of the `takumi-pdf` version in the producer as `0`, so a release does not
/// change the goldens. The length stays, and with it every byte offset the PDF records.
fn zero_producer_version(pdf: &mut [u8]) {
  const PRODUCER: &[u8] = b"takumi-pdf ";

  let mut start = 0;

  while let Some(found) = pdf[start..]
    .windows(PRODUCER.len())
    .position(|window| window == PRODUCER)
  {
    let mut index = start + found + PRODUCER.len();

    while let Some(byte) = pdf
      .get_mut(index)
      .filter(|byte| byte.is_ascii_digit() || **byte == b'.')
    {
      if byte.is_ascii_digit() {
        *byte = b'0';
      }

      index += 1;
    }

    start = index;
  }
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
