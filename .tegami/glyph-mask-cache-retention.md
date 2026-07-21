---
packages:
  "cargo:takumi-raster": patch
---

### Stop the glyph mask cache from retaining more memory than its budget

Cached glyph masks were charged `mask.len()` bytes but stored buffers recycled from the canvas buffer pool, which hands out any larger bucket — so a KB-sized mask could pin a much larger allocation and the 8 MiB budget under-enforced by a pool-state-dependent factor (#1023). A/B benchmarks showed the pool itself has no measurable win over the allocator on the render suites, so scratch buffers are now plain allocations: cached masks own exactly-sized buffers charged by capacity, and the buffer pool is gone. A retention test renders unique text over gradient cards and asserts live heap bytes stay near the budget.
