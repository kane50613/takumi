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

Gradient utilities merged into a finished gradient while parsing, so their colours could not come from the theme. They now compose through `--tw-gradient-*` custom properties, the way Tailwind compiles them: `from-brand-500` reads `var(--color-brand-500)`, `from-red-500` keeps the built-in red as its fallback, and an opacity modifier mixes the variable. The `--tw-*` state does not inherit, matching Tailwind's `@property` registrations.

Two behaviours move to match Tailwind. Stops alone no longer paint: `from-red-500` needs `bg-linear-*`, `bg-radial` or `bg-conic` to declare the gradient. And a missing `to` stop now fades to `transparent` instead of a transparent copy of the `from` colour, which interpolates the same way in the engine's Oklab default.
