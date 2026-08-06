---
packages:
  takumi-pdf:
    type: minor
---

### PDF/A output

Set `pdfa` to `"2b"`, `"2u"`, `"3b"`, `"3u"` or `"4"` to emit archival PDFs with an sRGB output intent and XMP metadata. Documents that cannot conform reject the render instead of writing a broken file.
