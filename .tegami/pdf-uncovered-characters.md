---
packages:
  takumi-pdf:
    type: minor
---

### Reject a character no registered font covers

A character outside every registered font shaped to `.notdef`. It painted nothing and left nothing in the text layer, so the page looked finished with the character quietly gone. Rendering now fails with `MissingGlyphs`, naming each character and its codepoint.
