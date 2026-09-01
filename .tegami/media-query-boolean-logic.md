---
packages:
  "takumi-core":
    type: minor
---

### Combine media queries with `or` and nested groups

`@media (min-width: 600px) or (min-height: 900px)` and nested groups such as
`(width > 100px) and ((height < 500px) or (orientation: portrait))` now parse.
`not` applies inside a group as well.
