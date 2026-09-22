---
packages:
  "takumi": patch
---

# Escape text inside JSX `<svg>` elements

`fromJsx` now escapes text children of an `<svg>` element, so they can no longer inject SVG markup. It throws on element or attribute names that are not valid XML names, and on `style` entries whose property name is invalid or whose value contains `;`.
