---
"takumi": patch
---

Skip painting zero-sized nodes instead of isolating them on a full-viewport offscreen canvas. Zero-sized nodes with `opacity` previously forced a full-canvas clear and composite each; a 1200x630 scene with a few such nodes rendered ~5x slower (much worse on WASM).
