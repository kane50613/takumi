---
packages:
  takumi-core: minor
---

# Support `unicode-bidi`

`unicodeBidi` with `direction` sets how a box's text joins the bidi algorithm: `embed` and `isolate` give an inline box its own direction, `bidi-override` and `isolate-override` lay its characters out in that direction whatever they are, and `plaintext` lets each paragraph's first strong character pick its direction.
