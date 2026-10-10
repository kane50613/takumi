---
packages:
  "takumi": patch
---

# Paint `background-clip: text` with a smaller layer

`background-clip: text` and the border-area background clip paint their mask on a layer the size of the box, not the whole image. A 1200×630 text-clipped paragraph peaks 35% lower and renders about 30% faster.
