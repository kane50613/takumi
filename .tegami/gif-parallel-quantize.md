---
packages:
  "takumi": patch
---

# Encode animated GIF faster

Native builds reduce each batch of GIF frames to their palettes in parallel, so a 24-frame 1200x630 animation encodes 2.5x faster. The output is byte-identical.
