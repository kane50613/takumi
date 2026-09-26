---
packages:
  "takumi": patch
---

# Keep `background-blend-mode` inside the box's background

A background layer with `background-blend-mode` blended with whatever the page had already painted behind the box. It now blends only with the layers and color beneath it, as browsers isolate the background.
