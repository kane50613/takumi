---
packages:
  "takumi": patch
---

# Cascade custom properties as browsers do

- `--gap: 8px !important` marks the declaration important instead of substituting `8px !important` wherever `var(--gap)` reads it.
- An `@property` rule with a typed `syntax` and no `initial-value` is ignored, so its name stays an ordinary custom property. `syntax` no longer keeps its quotes.
- An author's own `--tw-` custom property inherits like any other. Only the state the utility engine writes stops at its element.
- A `--tw-*` property registered with `@property` keeps its initial value, so Tailwind's compiled gradients that read `var(--tw-gradient-from-position)` paint.
