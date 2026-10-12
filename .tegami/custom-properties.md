---
packages:
  "takumi": patch
---

# Cascade custom properties as browsers do

- `--gap: 8px !important` marks the declaration important instead of substituting `8px !important`.
- An `@property` rule with a typed `syntax` and no `initial-value` is ignored. `syntax` no longer keeps its quotes.
- An author's own `--tw-` property inherits like any other.
- A `--tw-*` property registered with `@property` keeps its initial value, so Tailwind's compiled gradients paint.
