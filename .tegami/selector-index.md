---
packages:
  "@takumi-rs/core":
    type: patch
---

### Match selectors against the rules that could apply

Stylesheet rules are grouped by what their rightmost selector requires, so a node only runs the matcher against rules naming one of its classes, its id or its tag. Rendering 800 nodes against 800 rules drops from 67ms to 2.5ms; the cost now grows with the rules that could match rather than with the whole stylesheet.
