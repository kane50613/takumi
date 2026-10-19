---
packages:
  takumi-core: patch
---

# Parse an image list that starts with `none`

`background-image: none, url(a.png)` and `mask-image: none, none` were dropped as invalid. A list may now start with `none`.
