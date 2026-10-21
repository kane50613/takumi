---
packages:
  takumi-core: patch
---

# Decode images without the `image` crate

PNG and JPEG images now decode straight into RGBA through `png` and `zune-jpeg`, so `takumi-core` no longer depends on `image`. A 16-bit PNG channel now keeps its high byte instead of rounding to the nearest 8-bit value, and decode errors read differently. Raw bitmaps that SVG output embeds as PNG now compress with `png`'s fast setting, trading some size for encode time.
