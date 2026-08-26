---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Re-size `tw` breakpoints through CSS variables

An unconditional `:root` (or `@theme`) `--breakpoint-*` declaration now re-sizes the `sm:`–`2xl:` variants, and defines new ones like `3xl:`. Variants gate before the cascade runs, so a declaration behind a media query or another selector cannot move them.
