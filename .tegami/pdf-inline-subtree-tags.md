---
packages:
  takumi-pdf:
    type: patch
---

### Tag the structure inside an inline-block

An inline-block lays out in a subtree of its own, and that subtree drew without tagging anything. A heading or a list nested inside one never became a structure element, and its text was folded into the paragraph around the box. The subtree now tags its nodes where the document tree expects them.
