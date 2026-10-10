---
packages:
  "takumi": patch
---

# Encode lossy WebP on two threads

Lossy WebP output encodes about 13% faster. The bytes it writes do not change.
