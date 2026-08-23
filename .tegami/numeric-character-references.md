---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Decode the numeric character references the HTML spec defines

`&#X41;` stayed literal text because the decoder matched only a lower-case `x`, while the spec accepts either case. A reference in the C1 range now resolves through the windows-1252 table the spec names, so `&#153;` renders as `™` rather than an invisible control character, and `&#0;` becomes the replacement character rather than a raw NUL.
