---
packages:
  "takumi": patch
---

# Escape text inside JSX `<svg>` elements

`fromJsx` escapes text inside `<svg>`, so it cannot inject markup. It throws on invalid XML names and on `style` values that would end their own declaration, such as `fill: "red;stroke:blue"`.
