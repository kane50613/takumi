---
packages:
  "takumi-pdf":
    type: minor
---

### Name takumi in the PDF's `/Producer`

Rendered PDFs now carry `takumi-pdf` and its version in the info dictionary's
`/Producer` and in XMP's `pdf:Producer`, which identifies the renderer that
wrote the file. Set `metadata.producer` to write your own value instead.
