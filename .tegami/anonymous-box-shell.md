---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Stop anonymous text boxes repainting their parent's background in SVG and PDF

A block's loose text no longer paints the block's background a second time, so a negative `z-index` child behind that text stays visible, as it does in the raster output.
