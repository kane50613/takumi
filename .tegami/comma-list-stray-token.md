---
packages:
  "takumi": patch
---

# Reject a stray token after a background list

`background-repeat: repeat bogus` and the same tail on `background-size`, `background-position`, and `background-blend-mode` used to parse as valid, because the list parser swallowed the token after the last item. The declaration is now invalid and dropped, as in browsers.
