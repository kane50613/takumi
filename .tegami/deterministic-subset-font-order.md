---
packages:
  cargo:takumi-core:
    replay:
      - "exit prerelease: cargo:takumi-core"
  npm:@takumi-rs/core:
    replay:
      - "exit prerelease: npm:@takumi-rs/core"
  npm:@takumi-rs/wasm:
    replay:
      - "exit prerelease: npm:@takumi-rs/wasm"
---

### Make subset-group font selection deterministic

Subsets registered under one logical family (via `FontResource::subset_of`) were kept
in registration order. Callers commonly register fonts concurrently, so that order — and
therefore which subset won for a codepoint covered by more than one (e.g. overlapping
weight subsets, where the loser is faux-bolded) — varied per process. Identical input
could render to different bytes run to run.

Subsets are now held in a `BTreeSet`, ordered by their family name, so expansion and
selection no longer depend on registration timing. Same input renders identically.
