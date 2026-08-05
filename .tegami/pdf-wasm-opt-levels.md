---
packages:
  takumi-pdf:
    type: patch
---

### Shrink the WebAssembly binary by 5%

Size-optimize the PDF serialization and font subsetting crates. The shipped wasm drops about 220KB with render speed and output bytes unchanged.
