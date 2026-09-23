---
packages:
  "takumi": patch
---

# Escape text inside JSX `<svg>` elements

`fromJsx` now escapes text children of an `<svg>` element, so they can no longer inject SVG markup. It throws on element or attribute names that are not valid XML names, and on `style` entries that would end their own declaration, such as `fill: "red;stroke:blue"` or an unclosed quote. Semicolons inside quotes or `url()` still work.
