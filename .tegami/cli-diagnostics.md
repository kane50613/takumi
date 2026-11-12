---
packages:
  takumi: minor
  takumi-core: minor
  takumi-cli: patch
---

# `takumi image` warns of the characters no font draws

`takumi image` now prints a warning that names each character no loaded font draws, where it drew an empty box without a word. `uncovered_characters` in `takumi` returns the same list for a node. The command line also lists a file that is not there as `missing` in `--list-resources`, refuses a `--margin` or `--page` number without a unit, and points to `--font`, `--root`, or `takumi pdf` in the errors those fix.
