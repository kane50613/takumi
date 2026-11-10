---
packages:
  takumi-cli: minor
---

# `--log` prints what a render did

`--log info` prints a timed line to standard error for each stage of a render, and `--log debug` adds each resource loaded, each page and why it ended, the CSS the parser dropped, and each font family or image the render found missing.
