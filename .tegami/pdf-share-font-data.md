---
packages:
  takumi-core: minor
  takumi-pdf: patch
---

# Embed fonts in PDF without copying their files

`takumi-pdf` now hands each font instance the bytes the font was registered with, where it used to copy the whole file once per instance. A document that uses a 15 MB CJK font at three weights holds one copy of it instead of four.

`ShapedRun::font_blob` is now public, and `FontBlob::shared_data` returns the bytes as a shared `Arc` for a backend to keep without copying.
