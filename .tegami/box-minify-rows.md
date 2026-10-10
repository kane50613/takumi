---
packages:
  "takumi": patch
---

# Draw downscaled images faster

An image or background drawn smaller than its source now box-filters one row at a time, with each column's source span worked out once, instead of resolving every pixel through the general sampler. The pixels are the same, and real cards with downscaled photos render up to 69% faster.
