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

A render that goes without a character's glyph, a font family, an optional font, or an image under `images.throwOnError: false` now raises a warning. Takumi prints each distinct warning once through `console.warn`. A `render()` without `prepare()` checks for missing glyphs and families only when it passes `onWarning`, `strict`, or `onMissingGlyphs`, so the default path stays as fast as before. Pass `onWarning` to handle the warnings yourself, or `strict` to reject the render.

```tsx
import { isTakumiError, render } from "takumi-js";

try {
  await render(<div>Hello 世界</div>, {
    onWarning(warning) {
      if (warning.code === "missing-glyphs") {
        console.warn(warning.message, warning.characters);
      }
    },
    strict: process.env.NODE_ENV !== "production",
  });
} catch (error) {
  if (isTakumiError(error, "ERR_TAKUMI_STRICT")) {
    console.error(error.warnings);
  }
}
```

- `RenderWarning` tells its kinds apart by `code`, which is `missing-glyphs`, `unresolved-family`, `font-load-failed`, or `image-load-failed`. Each has a `message` and the fields its code adds, such as `characters` or `url`.
- A strict render rejects with a `TakumiError` of code `ERR_TAKUMI_STRICT`, whose `warnings` lists them. `isTakumiError(error, "ERR_TAKUMI_STRICT")` narrows to it.
- `prepare()` keeps the warnings in `prepared.report.warnings`, and `ImageResponse`'s `ready` resolves to the same report.
- Renders and `prepare()` take `onMissingGlyphs(characters)`, which runs once when characters have no font. The fonts it returns load as part of the render's `fonts`.
- In Rust, `RenderNeeds` has `missing_characters` and `unresolved_families`, and is `#[non_exhaustive]`.
