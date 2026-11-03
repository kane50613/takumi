---
packages:
  takumi-core: minor
---

# Paged layout takes the size of each page

`PagedLayout::new`, `PageStream::new`, `PageWindows::new`, `PagedScene::lay_out`, and `SceneRef::window` take a `PageSizes` instead of one `Size`. `PageSizes::uniform(size)` gives every page the size passed before. `SceneRef::page_origin(size, page)` is now `PageSizes::origin(page)`.
