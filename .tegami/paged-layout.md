---
packages:
  takumi-core: minor
---

# Lay out and paint a document a page at a time

`PagedLayout` lays a render tree out one `next_page` call at a time, `LayoutView` reads it while it goes on, and `PageStream` hands out the scene of each page once no later page can paint onto it. `Scene::enter`, `PagedScene::page_origin` and the `LayoutResults` methods `is_repeated_fragment`, `table_fragment`, `is_hidden_for_paint` and `is_line_clamp_clipped_float` are gone, in favor of `Scene::view`, `SceneRef::page_origin` and the same methods on `LayoutView`. `TextClip::of`, `TableDecorations::of` and `paint_collapsed_borders` read a `LayoutView`, and the last two take the `NodePaint`.
