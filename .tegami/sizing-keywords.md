---
"takumi": minor
---

# Size boxes from their content or available space

`width` and `height` now accept `min-content`, `max-content`, `fit-content`, `fit-content(<length-percentage>)`, and `stretch`.

```css
.card {
  width: fit-content(20rem);
}
```

This fits the card to its content with a preferred limit of `20rem`. Its min-content size remains the lower bound.

`flex-basis` accepts the same values, plus `content` to size a flex item from its content regardless of its main size property.

Tailwind utilities map to the new values:

| Value         | Utilities                    |
| ------------- | ---------------------------- |
| `min-content` | `w-min`, `h-min`, `size-min` |
| `max-content` | `w-max`, `h-max`, `size-max` |
| `fit-content` | `w-fit`, `h-fit`, `size-fit` |
| `content`     | `basis-content`              |

`min-width`, `max-width`, `min-height`, and `max-height` do not accept the new keywords. Their existing length and percentage values still work.
