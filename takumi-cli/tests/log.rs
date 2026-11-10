//! Runs `takumi` with `--log` and reads the steps it prints to standard error.

use std::{
  env, fs,
  process::{self, Command},
};

const PAGE: &str = r#"<style>
  body { margin: 0; font-family: Nowhere, sans-serif }
  h1 { break-after: page }
</style>
<h1>First</h1><img src="missing.png" width="10" height="10"><p>Second</p>"#;

fn steps(name: &str, arguments: &[&str]) -> String {
  let dir = env::temp_dir().join(format!("takumi-cli-log-{name}-{}", process::id()));

  fs::create_dir_all(&dir).unwrap();
  fs::write(dir.join("page.html"), PAGE).unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .args(arguments)
    .arg(dir.join("page.html"))
    .arg("-o")
    .arg(dir.join(format!("out.{}", arguments[0].replace("image", "png"))))
    .output()
    .expect("runs takumi");
  let steps = String::from_utf8(output.stderr).unwrap();

  assert!(output.status.success(), "{steps}");
  steps
}

/// The stage each timed line names, in the order printed.
fn stages(steps: &str) -> Vec<&str> {
  steps
    .lines()
    .filter_map(|line| line.split_once("ms] ")?.1.split_once(": "))
    .map(|(stage, _)| stage)
    .collect()
}

#[test]
fn info_prints_a_line_per_stage() {
  let steps = steps("info", &["image", "--log", "info"]);

  assert_eq!(
    stages(&steps),
    ["input", "stylesheet", "resource", "render", "output"],
    "{steps}"
  );
}

#[test]
fn debug_names_the_font_family_and_image_the_render_found_missing() {
  let steps = steps("debug", &["image", "--log", "debug"]);

  assert_eq!(
    steps
      .matches("font: no face is loaded for font-family \"Nowhere\"")
      .count(),
    1,
    "{steps}"
  );
  assert!(
    steps.contains("image: no image is loaded for \"missing.png\""),
    "{steps}"
  );
  assert!(steps.contains("layout: laid out "), "{steps}");
}

#[test]
fn debug_names_each_page_of_a_pdf() {
  let steps = steps("pages", &["pdf", "--log", "debug"]);

  assert!(steps.contains("layout: page 1 is "), "{steps}");
  assert!(steps.contains("layout: page 2 is "), "{steps}");
  assert!(!steps.contains("layout: page 3 is "), "{steps}");
}

#[test]
fn nothing_is_timed_without_the_flag() {
  let steps = steps("plain", &["image"]);

  assert!(stages(&steps).is_empty(), "{steps}");
  assert!(steps.contains("warning: "), "{steps}");
}
