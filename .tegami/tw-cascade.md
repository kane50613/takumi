---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Let a stylesheet rule win over a `tw` utility

The `tw` prop sat above every stylesheet rule, so `#hero { background: blue }` lost to `tw="bg-red-500"` even though an id selector outranks a class. Utilities now sit in a layer of their own below all author rules, matching how `@layer utilities` loses to unlayered CSS in Tailwind, and a utility marked `!` moves to the other end where it beats an important stylesheet rule.

Templates that relied on `tw` overriding a matching `className` rule need the rule moved into a cascade layer, or the utility marked with `!`.
