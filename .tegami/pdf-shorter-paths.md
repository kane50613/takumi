---
packages:
  takumi-pdf:
    type: patch
---

### Write shorter paths

Box decorations wrote every corner point twice and spelled out the closing edge that `h` draws anyway. Rectangles now use the `re` operator, and segments that go nowhere are dropped. A two-page invoice loses 12% of its bytes and renders about 3% faster.
