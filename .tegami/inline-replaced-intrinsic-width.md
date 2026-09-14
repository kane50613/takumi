---
"takumi": patch
---

# Size a replaced element the way its source states it

An `<svg>` carrying only a `viewBox` states a ratio and no size, and as an inline box it took a size the ratio never stated. `box-sizing: border-box` counted an image's padding and border twice, so the picture came out taller than its box. A source stating neither a size nor a ratio had one synthesised from the default object size, so `width` alone moved `height` with it.
