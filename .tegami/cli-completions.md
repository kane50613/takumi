---
packages:
  takumi-cli: minor
---

# `takumi completions` writes a shell completion script

`takumi completions <shell>` writes a script that completes takumi's commands, flags, and their values when you press Tab. It supports `bash`, `zsh`, `fish`, `powershell`, and `elvish`, and `--root` and `--allow-read` complete only directories.

```sh
takumi completions zsh > ~/.zfunc/_takumi
```
