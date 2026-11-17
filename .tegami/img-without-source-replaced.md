---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# An `<img>` without a source stays whole across pages

In a PDF, a tall `<img>` with no `src` split across two pages as an empty box. It now moves to the next page whole when it does not fit, as Chromium prints it, and it lays out at the same size as before.
