---
packages:
  "takumi": patch
---

# Write smaller lossy WebP

Lossy WebP files are about a quarter smaller at the same quality, with libwebp's method 2. It encodes about a quarter slower, then 13% faster again on two threads.
