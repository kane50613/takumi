---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Position absolute boxes against a positioned inline parent

An absolutely positioned box inside a `position: relative` span now takes its offsets from that span's box, from the start of its first line to the end of its last, as Chrome does. Before, it used the nearest positioned block instead. An absolute box also no longer lets a line break inside the word around it.
