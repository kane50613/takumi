# SVG rendering for language bindings

## Goal

Expose the `takumi-svg` backend through the napi (Node) and wasm (browser)
bindings as a first-class `renderSvg()` method that returns SVG markup.

## Surface

A new method on `Renderer`, parallel to `render()`, returning **text** rather
than encoded image bytes.

| Binding | Signature                                             | Sync                                 |
| ------- | ----------------------------------------------------- | ------------------------------------ |
| napi    | `renderSvg(node, options?, signal?): Promise<string>` | async `AsyncTask`, matching `render` |
| wasm    | `renderSvg(node, options?): string`                   | sync, matching `render`              |

SVG is not another `OutputFormat` variant: it produces a string, not a
`Bitmap` encoding, and honours a different option subset. A separate method
keeps each return type and option set honest (most-specific type; misuse made
impossible rather than ignored).

## Options

A dedicated `SvgRenderOptions`, not `Omit<RenderOptions, ...>`. The type carries
only fields SVG honours.

Kept: `width`, `height`, `images`, `stylesheets`, `keyframes`, `timeMs`,
`fontFamilies`, plus wrapper-level `fonts` and (napi) `signal`.

Dropped: `format`, `quality`, `lossless`, `dithering`, `drawDebugBorder`,
`devicePixelRatio` — SVG is vector, so DPR is meaningless and the raster-only
knobs do not apply.

## Implementation

Both binding crates gain a `takumi-svg` path dependency.

### napi (`takumi-napi-core`)

- `#[napi(object)] struct SvgRenderOptions` — the kept fields.
- `SvgRenderTask` mirrors `RenderTask::compute`: read renderer state,
  `state.decode_images`, build `takumi_svg::SvgOptions` from the renderer's
  `fonts` + viewport + stylesheet + images + time, call `takumi_svg::render`
  → `String`. No `write_image`, no format branch.
- `Renderer::render_svg` builds the task and returns it as an `AsyncTask`.

### wasm (`takumi-wasm`)

- `SvgRenderOptions` model struct.
- `Renderer::render_svg` calls `takumi_svg::render` directly (sync), returns
  `String` / `Result<String, JsValue>`.

### JS wrappers (`export.ts`, both)

- `renderSvg(node, options?)`: same font-prepare + image-resolve flow as
  `render()`, delegates to `inner.renderSvg`, returns `string`.
- Export `SvgRenderOptions` TS type from the generated internal type.

## Out of scope

- Animation (the svg crate has no animation path).
- `renderSvgDataUrl()` convenience helper.

## Testing

Per binding: render a known fixture node tree to SVG, assert well-formed XML
containing the expected elements, snapshot the markup.

## Changeset

One imperative line per binding package: add `renderSvg`.
