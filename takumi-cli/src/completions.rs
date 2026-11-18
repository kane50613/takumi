//! `takumi completions`: writes the script that completes takumi's commands and flags in a shell.

use std::{
  error::Error,
  io::{self, Write},
};

use clap::Command;
use clap_complete::{Generator, Shell};

/// Writes a script that completes takumi's commands and flags in a shell.
#[derive(clap::Args)]
#[command(
  after_help = "Save the script where your shell reads completions, then open a new shell:

  bash  takumi completions bash > ~/.local/share/bash-completion/completions/takumi
  zsh   takumi completions zsh > ~/.zfunc/_takumi
        and add `fpath+=~/.zfunc` before `compinit` in ~/.zshrc
  fish  takumi completions fish > ~/.config/fish/completions/takumi.fish
  pwsh  takumi completions powershell > ~/takumi.ps1
        and add `. ~/takumi.ps1` to $PROFILE"
)]
pub(crate) struct CompletionsArgs {
  /// The shell to complete in.
  shell: Shell,
}

impl CompletionsArgs {
  /// Writes the completion script for `command` to standard output.
  pub(crate) fn run(&self, command: Command) -> Result<(), Box<dyn Error>> {
    Ok(self.write(command, &mut io::stdout().lock())?)
  }

  /// Writes the completion script for `command` to `out`, failing as the write fails.
  fn write(&self, mut command: Command, out: &mut dyn Write) -> io::Result<()> {
    let name = command.get_name().to_owned();

    command.set_bin_name(name);
    command.build();
    self.shell.try_generate(&command, out)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A destination every write to fails, as a full disk does.
  struct Full;

  impl Write for Full {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
      Err(io::Error::from(io::ErrorKind::StorageFull))
    }

    fn flush(&mut self) -> io::Result<()> {
      Ok(())
    }
  }

  #[test]
  fn a_failed_write_is_an_error() {
    for shell in [
      Shell::Bash,
      Shell::Zsh,
      Shell::Fish,
      Shell::PowerShell,
      Shell::Elvish,
    ] {
      let args = CompletionsArgs { shell };

      assert!(
        args.write(Command::new("takumi"), &mut Full).is_err(),
        "{shell}"
      );
    }
  }
}
