---
packages:
  "takumi": patch
---

# Write smaller lossy WebP

Lossy WebP now uses libwebp's method 2, which searches harder for each block's best encoding. On real cards the files are about a quarter smaller at the same quality, and encoding takes about a quarter longer.
