---
"takumi": minor
---

# Add balanced flex wrapping, flow-root, self-alignment, and containment

Spread flex items across balanced lines and set a minimum line count:

```css
.cards {
  display: flex;
  flex-wrap: balance;
  flex-line-count: 2;
}
```

`balance` also combines with `wrap` or `wrap-reverse`, in either order. It cannot combine with `nowrap`.

- `display: flow-root` makes the box a block formatting context root.
- `align-items`, `align-self`, `justify-items`, and `justify-self` accept `self-start` and `self-end` to align to the item's own start or end. `justify-content` and `align-content` reject them.
- `contain` accepts `none`, `content`, or a space-separated combination of `layout`, `style`, and `paint`, without duplicates. `content` means `layout style paint`.

`contain: paint` clips descendants to the padding box. Either `layout` or `paint` creates a stacking context and a containing block for absolute and fixed descendants.

Size containment is not implemented, so `size`, `inline-size`, and `strict` cause a parse error. `style` parses but has nothing to scope: Takumi has no author-facing counters, and list-item ordinals do not restart at the boundary.

Containment is a correctness feature here, not a performance hint. Takumi renders once, so there is no relayout to skip. Paint containment adds a stacking context and a clip layer per box.
