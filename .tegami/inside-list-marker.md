---
packages:
  takumi-core: patch
---

# Space inside list markers as Chromium does

A `list-style-position: inside` marker now lays out as an inline box, as Chromium's does. A `disc`, `circle`, or `square` marker starts 1px earlier. A `disclosure-open` or `disclosure-closed` marker keeps a 0.4em gap from the text instead of 1em.
