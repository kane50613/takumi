---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Lay out text with parley 0.12

Text layout moves to parley 0.12. Letter spacing turns off ligatures as browsers do, and text beside floats, cloned box decorations, list markers, and `text-fit` lines sit closer to where Chrome puts them. A family's coverage subsets now load together, which lets later releases drop the per-subset family names.
