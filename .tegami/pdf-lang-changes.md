---
packages:
  takumi-pdf:
    type: patch
---

### Declare a passage written in another language

A `lang` attribute reached shaping and line breaking but never the output, so a document carrying Arabic or Hindi inside an English page declared only the document language. A screen reader read every passage in the document voice. Content whose language differs from the document's is now marked with that language.
