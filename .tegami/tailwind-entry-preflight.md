---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Read Preflight from every Tailwind entry point

`@import "tailwindcss"` now turns Preflight on when it carries modifiers such as `source(none)` or `important`. `@import "tailwindcss/preflight.css"` turns it on too, while `tailwindcss/theme.css` and `tailwindcss/utilities.css` leave it off, as in Tailwind's setup without Preflight.
