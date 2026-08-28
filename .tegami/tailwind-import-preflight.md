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

The import line at the top of a Tailwind v4 stylesheet now works. Preflight replaces the UA preset. Margins and padding go, lists lose their markers, and `h1` through `h6` drop their font sizing. It also brings the universal border reset, link and table resets, block-level images, and `hidden` on any element. Author rules outrank Preflight, apart from `hidden`, which it marks important. Other `@import` targets stay unsupported.
