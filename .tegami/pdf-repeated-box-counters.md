---
packages:
  takumi-pdf:
    type: minor
  "@takumi-rs/core":
    type: patch
---

### Count the pages a repeated box prints on

A page counter filled only inside a `header` or `footer` band. A footer written into the document itself, as a `fixed` box, printed empty hooks, so the numbers had to come from a render option standing beside the document. A repeated box now lays out again for every page it draws on, with that page's numbers, so a component can carry its own footer.
