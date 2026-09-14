---
"takumi": minor
---

# Report each text run's ascent from `measure()`

A run's `height` is its ascent plus its descent with no way to recover the split, so a consumer handing the text to another layout engine could not find the baseline; `y + ascent` is now that baseline.
