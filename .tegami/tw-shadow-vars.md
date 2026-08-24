---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve shadow colours through theme variables

`shadow-*` and `text-shadow-*` colour utilities recoloured the shadow while parsing, so the colour could not come from the theme. They now set `--tw-shadow-color` and `--tw-text-shadow-color`, and every shadow layer reads the variable with its own colour as the fallback, the way Tailwind compiles it. `shadow-brand-500` reads `--color-brand-500`; the preset shapes (`--shadow-*`, `--text-shadow-*`) still come from the built-in scale.
