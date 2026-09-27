---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Break the line at `<br>` whatever the styles say

A `<br>` now always starts a new line, as Chrome's does, even with the built-in style presets turned off or `white-space` set to collapse newlines.
