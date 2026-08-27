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

The import line at the top of a Tailwind v4 stylesheet now works. It drops the UA preset cosmetics that Preflight replaces: element margins, list markers and heading font tweaks. The theme defaults already back every utility, so nothing else needs importing. Other `@import` targets stay unsupported.
