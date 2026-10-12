---
packages:
  "takumi-core": patch
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out paragraphs with inline elements faster

A paragraph holding spans no longer lays its text out a second time to find its baseline. Large documents lay out about 8% faster.
