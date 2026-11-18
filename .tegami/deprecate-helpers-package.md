---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  "takumi-js": patch
  "takumi-pdf": patch
---

# Import helpers from the package you render with, as `@takumi-rs/helpers` is deprecated

`@takumi-rs/helpers` becomes internal in v3. Every export keeps working in 2.x, and editors flag it as deprecated. Each package you install now exports the helpers its users need:

- `@takumi-rs/core` and `@takumi-rs/wasm` export `fromJsx`, `googleFonts`, `fontFromUrl`, `extractEmojis`, `container`, `text`, `image`, `Bitmap`, and their types.
- `takumi-js` exports `googleFonts`, `container`, `text`, `image`, and `Bitmap`.
- `takumi-pdf` exports `googleFonts`.

`render` in `takumi-js`, `takumi-pdf`, and `takumi-paint` takes JSX and font URLs, so it needs no `fromJsx` or `fontFromUrl`.

#### Migration

```diff
- import { googleFonts } from "@takumi-rs/helpers";
- import { fromJsx } from "@takumi-rs/helpers/jsx";
+ import { fromJsx, googleFonts } from "@takumi-rs/core";
```

```diff
- import { googleFonts } from "@takumi-rs/helpers";
+ import { googleFonts } from "takumi-js";
```

Remove `@takumi-rs/helpers` from your dependencies once nothing imports it.
