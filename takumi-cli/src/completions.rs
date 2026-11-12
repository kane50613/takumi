//! `takumi completions`: writes the script that completes takumi's commands and flags in a shell.

use std::{error::Error, io};

use clap::Command;
use clap_complete::Shell;

/// Writes a script that completes takumi's commands and flags in a shell.
#[derive(clap::Args)]
#[command(after_help = "Save the script where your shell reads completions:

  bash  takumi completions bash > ~/.local/share/bash-completion/completions/takumi
  zsh   takumi completions zsh > \"${fpath[1]}/_takumi\"
  fish  takumi completions fish > ~/.config/fish/completions/takumi.fish
  pwsh  takumi completions powershell >> $PROFILE")]
pub(crate) struct CompletionsArgs {
  /// The shell to complete in.
  shell: Shell,
}

impl CompletionsArgs {
  /// Writes the completion script for `command` to standard output.
  pub(crate) fn run(&self, mut command: Command) -> Result<(), Box<dyn Error>> {
    let name = command.get_name().to_owned();

    clap_complete::generate(self.shell, &mut command, name, &mut io::stdout());
    Ok(())
  }
}
