---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Shape repeated text once per render

Text that several boxes repeat with the same style is shaped once per render again. A box that reused another's measurement used to drop the shared shape after drawing, so every later box shaped it anew.
