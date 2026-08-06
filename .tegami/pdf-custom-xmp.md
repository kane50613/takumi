---
packages:
  takumi-pdf:
    type: patch
---

### Custom XMP metadata

`metadata.xmp` takes namespaces to write into the XMP packet, for metadata the renderer knows nothing about. One is the `fx:` schema that turns a PDF/A-3 with an attached invoice into a Factur-X file.

- each schema carries a prefix, a namespace URI, and its properties
- every property is written as a value and described in the `pdfaExtension:schemas` entry PDF/A requires, so the two cannot drift apart
- a prefix, property name or namespace the XMP writer cannot serialize rejects the render instead of writing a broken packet
