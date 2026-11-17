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

A prepared render now has a `report` of what it could not draw, and `ImageResponse`'s `ready` resolves to that report. Renders and `prepare()` also take `onMissingGlyphs(characters)`, which runs once when characters have no font. The fonts it returns load as part of the render's `fonts`.

```tsx
const prepared = await renderer.prepare("<div>Hello 中文</div>", { width: 1200, height: 630 });
console.log(prepared.report.missingCharacters); // ["中", "文"] without a CJK font

const image = await render(<div>{title}</div>, {
  width: 1200,
  height: 630,
  onMissingGlyphs: (characters) =>
    googleFonts({ families: ["Noto Sans JP"], text: characters.join("") }),
});
```

The report lists:

- `missingCharacters`, the characters no font in their fallback chain draws
- `unresolvedFamilies`, the families `font-family` or `fontFamilies` names that no font provides
- `failedFonts`, the optional fonts and `@font-face` files that failed to load
- `failedImages`, the images left out under `images.throwOnError: false`

In Rust, `RenderNeeds` has `missing_characters` and `unresolved_families`, and is now `#[non_exhaustive]`.
