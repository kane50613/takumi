---
packages:
  "takumi": patch
---

# Encode PNG faster and smaller

- A PNG of at most 256 colors, as most text cards are, is stored with a palette. Real cards come out about 37% smaller, with the same pixels.
- Large PNGs deflate in up to eight segments at once. A 1200×630 card encodes about 2.5 times as fast, and a 1200×630 photo in 6ms instead of 18ms.
- Every machine writes the same bytes.
- APNG uses the same path. A 24-frame 1200x630 animation encodes 5x faster and comes out slightly smaller.
- A shorter match search and a truer settings sample cut encoding time by 10% and 4% across real templates. Files stay within 0.2% of their size.
