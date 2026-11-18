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

# Renders warn about what they could not draw, and load fonts for missing characters

A render now warns when it goes without a character's glyph, a font family, an optional font, or an image under `images.throwOnError: false`. Each warning prints through `console.warn` as `takumi: <message>`, the same form as the warnings for unsupported CSS, every time a render raises it. To collect the warnings somewhere else, wrap `console.warn`.

```txt
takumi: No font draws "世界". Add a font that covers them to `fonts`, or return one from `onMissingGlyphs`.
```

- `prepare()` prints the warnings it finds, and a later render of the prepared source warns only about a font that fails when it loads again.
- A `render()` without `prepare()` checks glyphs and families only when it passes `onMissingGlyphs`.
- `onMissingGlyphs(characters)` runs once when characters have no font, and the fonts it returns load with the render.
- In Rust, `RenderNeeds` has `missing_characters` and `unresolved_families`, and is now `#[non_exhaustive]`.
