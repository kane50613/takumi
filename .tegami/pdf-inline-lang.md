---
packages:
  takumi-pdf:
    type: patch
---

### Carry a language across an inline box

An inline box sat on a tagging path of its own, which still marked everything as the document language. A box carrying its own `lang` lost it, and so did the text that resumed after the box. Both now say what language they are in.
