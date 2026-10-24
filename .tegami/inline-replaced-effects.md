---
packages:
  takumi-core: patch
---

# Apply effects to inline images

An `<img>` or `<svg>` that sits in a line now paints as an atomic inline, as in Chrome, so its `opacity`, `filter`, `transform`, `clip-path`, `mask`, and `mix-blend-mode` take effect. They were ignored before unless the element was `inline-block` or a block.
