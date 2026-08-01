# takumi-pdf

Standalone product on the takumi engine: JSX → multi-page vector PDF. Different
audience than image rendering (documents: invoices, reports, statements), so it
ships as its own npm package with a smaller API, while living in this monorepo
and sharing takumi-core.

## Decisions (settled 2026-08-01)

- **Form**: OSS library first. Hosted API only after adoption proves demand.
- **Repo**: this monorepo. Crates `takumi-pdf` (backend) + `takumi-pdf-wasm` (binding).
- **npm**: single package `takumi-pdf`, wasm bundled inside, wasm-only (no napi).
  JSX conversion reuses `@takumi-rs/helpers`.
- **API**: `render(element, options) → Uint8Array`. No `PdfResponse` in v1.
  Options: `page` (`"a4"` | `{ width, height, margin }`), `fonts`,
  `header` / `footer` (JSX, `{page}` / `{pages}` placeholders).
- **Pagination**: single layout at unbounded height, then window-slicing per
  page. Atoms = text lines, images, `break-inside: avoid` boxes. Supported CSS:
  `break-before/after: page`, `break-inside: avoid`. Container backgrounds and
  borders clip across pages (browser print semantics). Page size presets mirror the CSS `@page`
  size keywords; the `@page` at-rule itself, widows, and orphans are not in v1.
- **Visual scope v1** (M2, stacked PR): background, border, radius, text,
  decorations, images, opacity, clip, transform, gradients (krilla native,
  incl. sweep/conic).
  Deferred: box-shadow / filter / backdrop-filter (PDF has no blur primitive —
  needs rasterization).
- **Text**: embedded subset fonts via krilla, selectable/searchable.
  Per-glyph cluster ranges done correctly in v1 (thread parley cluster data
  through `ShapedRun`, replacing the naive 1 char : 1 glyph mapping).
- **Runtimes** (tested + documented): Node, Cloudflare Workers. Others "should
  work", untested.
- **Docs**: section in the existing docs site. No landing page yet; no separate
  domain.
- **Testing**: PDF byte-goldens (deterministic serialize settings, no
  timestamps) + pdfjs text-extraction e2e smoke. Visual pixel tests (hayro)
  deferred.
- **Publishing**: npm only; crates stay `publish = false` until the API
  settles. Silent 0.x releases; one announcement when M1–M3 are done.

## Milestones

1. **M1 Pagination** (the differentiator): unbounded-height layout, atom
   collection, cut-point solver, per-page window emission
   (translate + clip), break properties in takumi-css, header/footer with
   page counters.
2. **M2 Visuals**: port box chrome (border/radius path emission), gradients,
   images (JPEG DCT passthrough), text decorations, opacity/clip/transform
   from the takumi-svg walker shape.
3. **M3 Packaging**: `takumi-pdf` npm package (wasm loader for Node +
   Workers), jsx-runtime wiring, docs section, examples, byte-golden CI job
   (build-std flags, wasm-opt simd).

## Size budget

3.3 MB raw / 1.35 MB gzip today (fits Workers free tier). Backlog, in order of
trigger: krilla release aligning skrifa (−30 KB gz), pdf-writer direct +
GID-identity glyf pruning subsetter (−250 KB gz, also unlocks streaming
output) — only if the 3 MB Workers limit actually bites or krilla becomes a
liability.

## Out of scope for now

Tagged PDF / PDF/A / PDF/UA (enterprise moat, after v1), display: table with
repeating headers, widows/orphans, linearized (fast web view) output,
hosted API.
