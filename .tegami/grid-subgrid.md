---
packages:
  takumi-core: minor
---

# Grids lay their items out in subgrids

`grid-template-columns: subgrid` and `grid-template-rows: subgrid` used to lay out as `none`, so a nested grid sized its own tracks. A subgrid now takes the tracks it spans from its parent, its items size those tracks alongside the parent's, and its line names, areas, and `<line-name-list>` resolve against them. A subgrid's own `gap` replaces the parent's in its span. `grid-row` and `grid-column` also accept named lines, such as `span a` and `a 2`.

```css
.cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
}
.card {
  display: grid;
  grid-row: span 3;
  grid-template-rows: subgrid;
}
```
