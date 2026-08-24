---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Compose filters and transforms through custom properties

Filter, translate, scale and grid-line utilities merged in a parse-time builder instead of the cascade. Each one now writes its `--tw-*` variable and re-declares the property from Tailwind's fixed chain, so `blur-sm brightness-125` composes in the same order as Tailwind whichever way the classes are written. Stacked filters previously applied in class order; they now follow Tailwind's chain order, which can change the result when order matters.
