# takumi — Domain Glossary

Names for the seams that matter. Architecture vocabulary (module/depth/seam/adapter) from the `/codebase-design` skill; the terms below name takumi's own concepts.

## Render pipeline

- **Stacking-context scene** — `takumi-core/src/scene.rs::build_stacking_contexts` produces one `Vec<StackingContextNode>` (paint order, z-buckets, hoisted out-of-flow, device bounds). The single deep seam both the raster and SVG backends walk. Do not re-derive stacking/z-index logic in a backend.
- **Inline layout engine** — `takumi-core/src/layout/inline.rs`, driven through `InlineLayoutRequest` with a `Measure`/`Draw` mode flag. Shared by both backends; not two implementations.
- **BoxDecorationPlan** — backend-agnostic box-decoration geometry in `takumi-core/src/layout/decoration.rs`, resolved once and consumed by both backends (no compositing inside). `ClipBox` covers the `background-clip` regions (border/padding/content) and the inset-`box-shadow` hole; `OutlineGeometry` covers the outline ring. Raster translates them to tiny-skia ops; SVG to vector paths. Removes the drift the `node_paint.rs` "candidate for promotion" note predicted. Outset-shadow spread already rides the shared `BorderProperties::outset_shadow_box`, so only its two-line offset convention is still mirrored — not worth a wrapper.

## Bindings

- **Options lowering** — the platform-agnostic step turning raw binding fields into a `takumi_raster::RenderOptions`: stylesheet+keyframes → `StyleSheet`, images → decode map, lang → `Lang`, font list → `FontFamily`, embedded-font bootstrap. Lives in **`takumi-bindings-common`** _(planned — A2)_, consumed by both napi and wasm. Bindings keep only platform-specific glue: napi's rayon pool teardown (`pool.rs`), wasm's borrow-flag reentrancy guard, JS type coercion.
  - **WebP lossless policy is deliberately platform-conditional**, not a bug: napi resolves `lossless.unwrap_or(quality.is_none())`; wasm hardcodes `WebPLossless` because its `image-webp` backend has no lossy encoder. Keep the branch when consolidating.

## Resources & caches

- **ImageCache** — `takumi-core/src/resources/image.rs`. Byte-budgeted (`quick_cache` + weighter, 64 MiB), single-flighted, holds encoded bytes _and_ per-size decodes in one budget. The reference for good cache hygiene.
- **GIF decode-on-demand** _(landed — M1)_ — `GifInner` retains only encoded `bytes`, the first frame, and pixel-free per-frame timing (`gif_frame_durations`, a `skip_frame_decoding` pass memoized in a `OnceLock`). Every later frame is decoded at draw size on demand and dropped — **no cache, no lock**, retention is a single frame. Chosen over a ring cache because the user's constraints were "smallest possible, no global Mutex (keep the rayon pool parallel), unbounded feels like leaking": a no-cache design can't leak and needs no lock. GIF disposal is stateful, so a later frame replays 0..N (O(N) per sample) — fine for the dominant single-sample static render; a GIF re-encoded to an animation is O(N²), upgrade path documented at the call site (thread-local resumable decode cursor).
- **Font glyph module** _(planned — A5)_ — split glyph rasterization (`ColorPainter`, `OutlinePen`, `GlyphOutlinePen`) out of `resources/font.rs`, leaving the `Fonts` registry behind. Pure move, no behavior change.

## Deferred (documented, not built)

- **resvg vendor boundary (A3)** — vendored resvg reaches back into `resources/image_decoder` at 3 sites; upstream's `image_href_resolver` hook is the supported seam. Gated on the vendor-update campaign staying live.
- **CSS/Tailwind registry drift (A4)** — the `define_style!` longhand table and the Tailwind phf map are hand-synced with no compile link. Fix is a coverage assertion (missing mapping = build error), not a merge (mappings aren't 1:1).
- **Glyph cache byte-weighting (M2)** / **SVG raster cache byte budget (M3)** — workload-gated; only worth it for emoji-heavy or large-SVG-at-many-sizes usage. Measure first.
