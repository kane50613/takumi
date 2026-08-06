---
packages:
  takumi-pdf:
    type: patch
---

### Custom XMP metadata

`metadata.xmp` writes an RDF fragment into the XMP packet, for schemas the renderer knows nothing about: the `fx:` block that turns a PDF/A-3 with an attached invoice into a Factur-X file, a C2PA claim, a retention policy.

- `metadata.xmpSchemas` carries the `pdfaExtension:schemas` entries describing those properties, which PDF/A requires and a packet allows only once, so they merge into the bag the renderer writes
- both are written verbatim; a fragment that is not well-formed XML rejects the render
