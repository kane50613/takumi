---
packages:
  takumi-core: minor
---

# Share a shaped run's font bytes without copying them

`ShapedRun::font_blob` is now public, and `FontBlob::shared_data` returns the font's bytes as a shared `Arc`, so a backend can keep them without copying the file.
