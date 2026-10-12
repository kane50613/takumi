---
packages:
  "takumi": patch
---

# Scale images faster

Bilinear image scaling mixes eight pixels at a time in vector lanes, with the same pixels as before. Real templates render about 5% faster.
