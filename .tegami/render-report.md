---
packages:
  takumi-core:
    type: minor
  takumi:
    type: minor
  "@takumi-rs/helpers":
    type: minor
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  takumi-pdf:
    type: minor
  takumi-paint:
    type: minor
  takumi-js:
    type: minor
---

# Report what a render could not draw, and load fonts for missing characters

A prepared render has a `report` that lists the characters no font in their fallback chain draws, the families `font-family` or `fontFamilies` names that no font provides, the optional fonts and `@font-face` files that failed to load, and the images left out under `images.throwOnError: false`. `ImageResponse`'s `ready` resolves to that report. Renders and `prepare()` take `onMissingGlyphs(characters)`, which runs once when characters have no font, and the fonts it returns load as part of the render's `fonts`. In Rust, `RenderNeeds` has `missing_characters` and `unresolved_families`, and is `#[non_exhaustive]`.
