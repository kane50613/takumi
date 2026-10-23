---
packages:
  takumi-core: minor
---

# Paint boxes from a `LayoutView` and a pages-back bound

`TextClip::of`, `TableDecorations::of` and `paint_collapsed_borders` read a `LayoutView`, and `TextClip::of` takes the context the box paints under. `StripBackground` carries that context. `layout::paint_reach::reach_back` bounds how far above its page a tree can paint, `PagedLayout::settled_pages` says how many pages are safe to paint under that bound, and `PageWindows::refresh` measures the pages again once they stop changing. A page stays open while page areas go on after the root's last fragment.
