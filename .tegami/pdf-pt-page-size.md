---
packages:
  "cargo:takumi-pdf": minor
---

### Write page geometry in PDF points

Pages were sized in CSS px written as pt, so an A4 document came out 33% oversized when printed. Page size, annotations, and outline destinations now convert at 0.75 pt/px; layout still runs in px.
