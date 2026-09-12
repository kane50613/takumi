---
"takumi": minor
---

# Render canvases up to 64 megapixels

The canvas budget rises from 16 to 64 megapixels, so 8K wide banners and long 4K pages render instead of failing with `InvalidViewport`. The error message now names the budget.
