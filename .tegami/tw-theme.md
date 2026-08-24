---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve `tw` utilities through theme variables

`bg-red-500` now resolves `var(--color-red-500)` and `p-4` resolves `calc(var(--spacing) * 4)`, with the built-in value as the fallback. Define tokens in `:root` or the `variables` option; `--color-brand-500` makes `bg-brand-500` work.

### Let stylesheet rules win over `tw` utilities

Utilities now sit in the last cascade layer: below unlayered CSS, above rules in a named `@layer`. Templates that relied on `tw` beating a matching rule need the rule moved into a layer, or the utility marked `!`.
