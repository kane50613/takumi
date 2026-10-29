---
packages:
  takumi-core: patch
---

# Malformed SVG images render what the parser recovers

SVG images, `filter: url(data:image/svg+xml,...)` references, and SVG sizing now go through takumi's own markup parser instead of `roxmltree`, which leaves the binaries. An SVG that is not well-formed XML renders what the parser recovers instead of failing to load, entities a DTD declares stay unexpanded, and a referenced filter keeps only its `fe*` primitives, so a `<script>` or other element inside the `<filter>` no longer reaches the SVG output.
