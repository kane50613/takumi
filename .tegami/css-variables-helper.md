---
packages:
  "@takumi-rs/helpers":
    type: minor
---

### Add the `cssVariables` helper

Flattens a nested tree into the flat map the `cssVariables` option takes: `{ color: { brand: { 500: "#5b21b6" } } }` becomes `{ "--color-brand-500": "#5b21b6" }`.
