---
packages:
  "cargo:takumi-core": patch
  "cargo:takumi-raster": patch
---

### Stop redoing two pieces of per-node and per-glyph work

Every node deep-cloned its parent's `ComputedStyle`, a struct covering every CSS longhand plus two `HashMap`s, so that a loop over `@property` rules could conditionally mutate it. Stylesheets that declare no `@property` rule, which is most of them, paid the clone and got an exact copy back. The function returns a `Cow` now and borrows in that case.

Underlined text rasterized each glyph outline twice: once through the shared glyph-mask cache for the actual paint, and once through a direct uncached call that only measured skip-ink bounds. The second pass neither read nor filled the cache, so it repeated for every occurrence of the glyph. It now goes through the same cache at subpixel bucket 0, which a new test pins as identical to the untransformed rasterization. `text-decoration-skip-ink: auto` is the CSS default, so this was the ordinary path for links and headings.

Rendered output is unchanged.
