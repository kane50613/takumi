---
"takumi": minor
---

# Add the sizing keywords to `width`, `height` and `flex-basis`

`width` and `height` take `min-content`, `max-content`, `fit-content`, `fit-content(<length-percentage>)` and `stretch`, and `flex-basis` takes those plus `content`. The Tailwind `w-min`, `w-max`, `w-fit`, `h-min`, `h-max`, `h-fit`, `size-*` and `basis-content` utilities map onto them. `min-width`, `max-width`, `min-height` and `max-height` still take only lengths.
