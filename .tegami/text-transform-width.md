---
packages:
  takumi-core: minor
---

# Support `full-width` and `full-size-kana` in `text-transform`

`textTransform` takes `full-width`, which puts characters in their full-width forms, and `full-size-kana`, which puts small kana in their full-size forms, alone or with a case: `uppercase full-width`. A space becomes U+3000 only where white space is preserved, as in Chrome.

In Rust, `TextTransform` is now a struct of a `TextTransformCase` and the two flags; `TextTransformCase` converts into it.
