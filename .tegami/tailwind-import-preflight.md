---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Turn on Preflight through `@import "tailwindcss"`

The import line at the top of a Tailwind v4 stylesheet now works: it drops the UA preset cosmetics (element margins, list markers, heading font tweaks), the part of Preflight the renderer itself supplies. The theme defaults already back every utility, so nothing else needs importing. Other `@import` targets stay unsupported.
