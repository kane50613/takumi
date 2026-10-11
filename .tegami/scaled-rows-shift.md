---
packages:
  "takumi": patch
---

# Draw cropped images faster

An image drawn at its own size but shifted by whole pixels, as `object-fit: cover` crops it, now copies its source rows instead of sampling every pixel. The pixels are the same, and real cards with a cropped photo render up to 4 times faster.
