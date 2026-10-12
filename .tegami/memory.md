---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Render in less memory

- An 11,000-row table renders in about 110MB less memory, since the layout tree is freed once layout finishes.
- Anonymous boxes share one layout style, and scenes give back unused list room, so large tables use less memory.
- A clip that covers the whole image, such as `overflow: hidden` on the root, no longer keeps a second mask.
- A full-width `blur-3xl` on a 1200×630 image peaks 38% lower.
