---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Decode the numeric character references the HTML spec defines

`&#X41;` was left as literal text because only a lower-case `x` was matched, though the spec allows either case. References in the C1 range now resolve through the windows-1252 table the spec names, so `&#153;` renders as `™` instead of an invisible control character, and `&#0;` becomes the replacement character rather than a raw NUL.
