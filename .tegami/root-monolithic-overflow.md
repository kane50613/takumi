---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# Content after a box that overflows a page starts below the overflow

In a paged PDF, a box at the top of a page can reach past the page's end, such as a flex row whose item margin runs past it. The content after that box started at the top of the next page and drew over the overflow. It now starts below the overflow, as in Chromium.
