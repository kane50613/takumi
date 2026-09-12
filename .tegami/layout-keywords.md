---
"takumi": minor
---

# Add flow-root, self-position alignment, balanced flex wrapping, and `contain`

`display: flow-root` makes a box a block formatting context root. `align-items`, `align-self`, `justify-items` and `justify-self` take `self-start` and `self-end`. `flex-wrap` takes `balance`, paired with the new `flex-line-count` longhand. `contain` takes `none`, `content`, and any of `layout`, `style` and `paint`. `contain: paint` clips descendants to the padding box, and either `layout` or `paint` makes the box a stacking context and a containing block for fixed descendants. `size`, `inline-size` and `strict` are rejected, because size containment is not implemented.
