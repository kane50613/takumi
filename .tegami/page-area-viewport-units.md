---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Resolve viewport units against the page area in paged output

`100vh` in paged content was `0` because the content column lays out at unbounded height; it now equals the page area height, as in print media.
