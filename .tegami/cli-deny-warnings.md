---
packages:
  takumi-cli: minor
---

# `takumi` warns of unsupported properties, and `--deny-warnings` fails on any warning

A declaration whose property takumi does not support, such as a misspelled one, now prints a warning with its line and column. `--deny-warnings` turns any warning into a failure that exits with `1` and writes no output.

```sh
$ takumi image card.html -o card.png --deny-warnings
warning: ignored the property `widht` at line 1, column 7 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.
error: `--deny-warnings` is set and the command raised 1 warning, so it wrote nothing. Fix each one, or drop the flag to write the output anyway.
```

Properties that cannot change a render, such as `cursor`, raise no warning and show only under `--log debug`.
