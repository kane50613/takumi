---
packages:
  takumi-core: patch
  "@takumi-rs/core": patch
---

# Loading a font no longer blocks rendering

`FontResource::into_resolved` now also limits the `cmap` to the `unicode_range` and hashes the font, so registering a resolved font takes the same short time whatever its size. `@takumi-rs/core` registers fonts on the JavaScript thread after decompressing them in the background, and renders no longer wait on a lock while fonts load. The font cache budget now counts a `unicode-range` subset by the bytes it keeps.
