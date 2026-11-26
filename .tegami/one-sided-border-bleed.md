---
packages:
  takumi-core: patch
---

# A rounded background tucks under a one-sided border

A rounded box with a border on only some sides now shrinks its background halfway under those borders, as Chrome does, so no background colour shows through the antialiased edge of the border.

```css
.bar {
  border-radius: 4px;
  border-bottom: 6px solid green;
  background: blue;
}
/* Before: a thin blue fringe could show along the green border's rounded edge */
/* After: the blue stops halfway under the green border */
```
