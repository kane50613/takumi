---
packages:
  takumi-cli: minor
---

# `takumi` warns of unsupported properties, and `--deny-warnings` fails on any warning

A declaration whose property takumi does not support, such as a misspelled one, now prints a warning with its line and column. Properties that cannot change a render, such as `cursor`, stay in `--log debug`. `--deny-warnings` exits with `1` and writes no output when the command raised a warning.
