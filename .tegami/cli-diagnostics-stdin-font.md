---
packages:
  takumi-cli: patch
---

# `takumi` names standard input and `--font` in what it prints

- `--log info` prints `input: read standard input` for `-`, where it printed the URL of the working directory.
- A `--font` file that is missing or not a font fails with an error that starts with the flag, such as `` `--font nope.ttf`: No such file or directory ``.
- `--list-resources --help` names the tab-separated columns of each line.
