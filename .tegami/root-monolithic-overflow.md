---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# Content after a box that overflows a page starts below the overflow

When a box at the top of a page reached past its end, such as a flex row whose item margin ran past the page, the content after it started at the top of the next page and drew over the overflow. It now starts below the overflow, as in Chromium.
