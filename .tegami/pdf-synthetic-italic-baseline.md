---
packages:
  takumi-pdf: patch
---

# Keep a synthesized italic where its advance puts it

A run slanted for a face with no italic of its own was drawn about a quarter of an em to the right, into the next word in LTR text and into the previous word in RTL text. It now slants about its own baseline, as in PNG output.
