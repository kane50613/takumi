---
packages:
  takumi-core: minor
---

# Paged layout asks for each page's size as it reaches the page

`PagedLayout::new`, `PageStream::new`, and `PagedScene::lay_out` take a `PageSizer` instead of one `Size`, and `PageSizer::uniform(size)` gives every page the size passed before. The layout records the size of each page it lays out in `PageSizes`, which `PagedLayout::sizes` and `PageStream::sizes` return and `SceneRef::window` and the `PageWindows` methods take. `SceneRef::page_origin(size, page)` is now `PageSizes::origin(page)`, and `PageWindows::new(size)` is now `PageWindows::default()`.
