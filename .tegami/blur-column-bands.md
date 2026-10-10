---
packages:
  "takumi": patch
---

# Blur with less scratch memory

A blur's scratch buffers no longer grow with the image width. A full-width `blur-3xl` on a 1200×630 image peaks 38% lower.
