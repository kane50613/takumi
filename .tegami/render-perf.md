---
packages:
  "takumi": patch
---

# Render faster

Real cards render up to:

- 69% faster with downscaled photos, and 3 times faster with photos cropped by `object-fit: cover`.
- 65% faster with translucent overlays.
- 50% faster with nested clips or large blurred backgrounds.
- 30% faster when text-heavy.
- 14-32% faster with big blurred backgrounds, which blur on several threads in native builds.
- 11% faster when the card ends translucent.

Also:

- Text is shaped and measured once per node.
- Rotated and scaled glyphs come from the glyph cache. Their edges may move by up to 1/8px, as they now snap the way Chrome's do.
- Shapes and glyphs outside the image are skipped.
- A stylesheet is indexed once when parsed, so passing a whole site's CSS no longer slows every render.
