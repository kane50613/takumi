---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve gradient stops through theme variables

`from-brand-500` reads `var(--color-brand-500)`; built-in colours stay as fallbacks. Two behaviours move to match Tailwind: stops alone no longer paint without `bg-linear-*`, `bg-radial` or `bg-conic`, and a missing `to` stop fades to `transparent`.
