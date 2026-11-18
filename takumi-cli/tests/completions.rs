//! Runs `takumi completions` for each shell.

use std::process::Command;

#[test]
fn each_shell_gets_a_script_naming_every_command() {
  for shell in ["bash", "elvish", "fish", "powershell", "zsh"] {
    let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
      .args(["completions", shell])
      .output()
      .expect("runs takumi");
    let script = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{shell}: {output:?}");
    for command in ["image", "pdf", "paint", "upgrade", "completions"] {
      assert!(script.contains(command), "{shell} completes no `{command}`");
    }
  }
}

#[test]
fn zsh_completes_only_directories_for_the_directories_a_document_may_read() {
  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .args(["completions", "zsh"])
    .output()
    .expect("runs takumi");
  let script = String::from_utf8_lossy(&output.stdout);
  let completes_directories = |flag: &str| {
    script
      .lines()
      .filter(|line| line.contains(&format!("{flag}=")))
      .all(|line| line.ends_with(":_files -/' \\"))
  };

  assert!(completes_directories("--root"));
  assert!(completes_directories("--allow-read"));
}

#[test]
fn an_unknown_shell_is_a_usage_error() {
  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .args(["completions", "tcsh"])
    .output()
    .expect("runs takumi");

  assert_eq!(output.status.code(), Some(2));
}
