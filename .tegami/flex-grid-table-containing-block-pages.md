---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# Break positioned boxes across the pages of flex, grid and table containers

In paged output, an absolutely positioned box in a flex, grid or table container now breaks across pages with its container, and one placed in a grid area that starts on a later page lands on that page.
