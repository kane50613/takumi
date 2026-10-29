---
packages:
  takumi-core: patch
---

# SVG images parse with takumi's own markup parser

SVG images, inline `filter: url(data:image/svg+xml,...)` references, and SVG sizing now go through the HTML parser in `takumi_core::html` instead of `roxmltree`, which leaves the binaries. An SVG that is not well-formed XML now renders what the parser recovers, where it used to fail to load, and entities a DTD declares stay unexpanded.

A referenced filter keeps only its filter primitives, the elements whose names start with `fe`, with text and attribute values escaped. A `<script>` or any other element inside the `<filter>` no longer reaches the SVG output.
