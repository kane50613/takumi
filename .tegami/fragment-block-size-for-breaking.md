---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A flex row or box taller than its content's reach moves to the next page when it does not fit

A flex row whose item margin ran past the end of the page stayed on that page and spilled onto the next, because breaking measured only how far the items reached. Breaking now measures the box's own block size too, as Chromium does, so the row moves to the next page whole when it fits there.
