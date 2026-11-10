//! Runs `takumi` on a document that warns, with and without `--deny-warnings`.

use std::{
  env, fs,
  path::PathBuf,
  process::{self, Command, Output},
};

const PAGE: &str = r#"<style>
  p { colr: red; cursor: pointer; -webkit-font-smoothing: antialiased }
</style>
<p style="widht: 1px">One</p><p style="widht: 1px">Two</p>"#;

fn render(name: &str, html: &str, flags: &[&str]) -> (Output, PathBuf) {
  let dir = env::temp_dir().join(format!("takumi-cli-warnings-{name}-{}", process::id()));
  let image = dir.join("out.png");

  fs::create_dir_all(&dir).unwrap();
  fs::write(dir.join("page.html"), html).unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .arg("image")
    .arg(dir.join("page.html"))
    .arg("-o")
    .arg(&image)
    .args(flags)
    .output()
    .expect("runs takumi");

  (output, image)
}

#[test]
fn an_unsupported_property_warns_once_with_its_place() {
  let (output, image) = render("plain", PAGE, &[]);
  let messages = String::from_utf8(output.stderr).unwrap();

  assert!(output.status.success(), "{messages}");
  assert!(image.exists());
  assert_eq!(
    messages.lines().collect::<Vec<_>>(),
    [
      "warning: ignored the property `widht` at line 1, column 7 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.",
      "warning: ignored the property `colr` at line 2, column 12 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.",
    ]
  );
}

#[test]
fn deny_warnings_fails_before_writing() {
  let (output, image) = render("deny", PAGE, &["--deny-warnings"]);
  let messages = String::from_utf8(output.stderr).unwrap();

  assert_eq!(output.status.code(), Some(1), "{messages}");
  assert!(!image.exists());
  assert!(
    messages.contains("error: `--deny-warnings` is set and the command raised 2 warnings"),
    "{messages}"
  );
}

#[test]
fn deny_warnings_counts_the_warnings_quiet_hides() {
  let (output, image) = render("quiet", PAGE, &["--deny-warnings", "-q"]);
  let messages = String::from_utf8(output.stderr).unwrap();

  assert_eq!(output.status.code(), Some(1), "{messages}");
  assert!(!image.exists());
  assert!(!messages.contains("warning: "), "{messages}");
}

#[test]
fn deny_warnings_counts_a_character_no_font_draws() {
  let (output, _) = render("glyph", "<p>你</p>", &["--deny-warnings"]);
  let messages = String::from_utf8(output.stderr).unwrap();

  assert_eq!(output.status.code(), Some(1), "{messages}");
  assert!(messages.contains("raised 1 warning,"), "{messages}");
}

#[test]
fn deny_warnings_passes_a_document_that_raises_none() {
  let (output, image) = render(
    "clean",
    "<p style=\"cursor: pointer\">One</p>",
    &["--deny-warnings"],
  );

  assert!(output.status.success());
  assert!(image.exists());
}
