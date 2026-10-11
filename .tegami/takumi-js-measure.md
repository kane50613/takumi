---
packages:
  "takumi-js": minor
---

# Measure elements with `measure()`

`takumi-js` exports `measure()`, which lays out JSX, an HTML string, or a node tree without painting it. It takes the same input and options as `render()` and runs on the same default renderer, so you no longer convert JSX, merge CSS, or create a `Renderer` yourself to measure.
