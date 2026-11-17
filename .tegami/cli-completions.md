---
packages:
  takumi-cli: minor
---

# `takumi completions` writes a shell completion script

`takumi completions <shell>` writes a script that completes takumi's commands, flags, and their values when you press Tab. It supports `bash`, `zsh`, `fish`, `powershell`, and `elvish`. In bash, zsh, and fish, `--root` and `--allow-read` complete only directories.

```sh
mkdir -p ~/.zfunc
takumi completions zsh > ~/.zfunc/_takumi
# Then add `fpath+=~/.zfunc` before `compinit` in ~/.zshrc
```

`takumi completions --help` lists where each shell reads the script.
