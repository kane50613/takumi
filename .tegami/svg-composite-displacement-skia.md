---
packages:
  takumi-core: patch
---

# Run arithmetic `feComposite` and `feDisplacementMap` as Chrome does

The arithmetic operator of `feComposite` now saturates every channel and rounds its result as Skia does, instead of truncating. `feDisplacementMap` reads its map as straight colors in floats and samples the source pixel nearest each displaced pixel center. Arithmetic compositing also runs about 20% faster.
