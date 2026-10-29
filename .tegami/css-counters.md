---
packages:
  takumi-core: minor
---

# Number content with CSS counters

`counter-reset`, `counter-increment`, and `counter-set` change named counters, and `content` writes them with `counter(name)` or `counters(name, ".")`, in any `list-style-type` style. Counters follow the document order Chrome counts in, including the implicit `list-item` counter of `ol`, `ul`, and `li`, and `contain: style` keeps the counters inside an element from reaching the rest of the page. `::after` reads a counter after the element's children change it, so a section can close with how many items it held.
