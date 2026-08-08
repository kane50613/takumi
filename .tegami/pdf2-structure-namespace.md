---
packages:
  takumi-pdf:
    type: patch
---

### Correct the PDF 2.0 structure namespace

A tagged PDF/A-4 document now names its structure namespace `http://iso.org/pdf2/ssn`, the identifier ISO 32000-2 defines. The old one matched no known namespace, so PDF/UA-2 validators rejected every structure element in the file.
