---
packages:
  "takumi-pdf": minor
  "takumi-paint": minor
---

# Remove the duplicate exports from `takumi-pdf` and `takumi-paint`

Each removed name has one replacement:

- `NodeInput` from both packages. Use `DocumentInput`, which both packages now export.
- `PageNumber`, `TotalPages`, `TargetPageNumber`, `CounterProps`, and `CounterStyle` from the `takumi-pdf` root. Import them from `takumi-pdf/primitives`, which skips loading the WebAssembly module.
- `MeasuredSize` from `takumi-pdf`. `measure` returns `Dimensions`, which has the same `width` and `height`.

#### Migration

```diff
- import { render, PageNumber, type NodeInput } from "takumi-pdf";
+ import { render, type DocumentInput } from "takumi-pdf";
+ import { PageNumber } from "takumi-pdf/primitives";
```
