---
packages:
  takumi-cli: minor
---

# `takumi paint` marks a box the canvas cuts off

A node that crosses an edge of the canvas now ends with `cut off at the <side> edge`, or names each edge it crosses. Before, only a node wholly past an edge was marked, as `offscreen`.

```diff
- box div#cta 1100,600 200x60 background #4338ca
+ box div#cta 1100,600 200x60 background #4338ca cut off at the right and bottom edges
```
