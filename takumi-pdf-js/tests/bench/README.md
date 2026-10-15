# Compare PDF WebAssembly builds in Node.js

Build the baseline and candidate with the same Rust, wasm-bindgen, wasm-opt, helpers, and Node versions. Keep the generated JavaScript from the same source: the two binaries must have the same wasm-bindgen interface. Run builds and benchmarks sequentially, without another compiler or benchmark competing for CPU.

Before applying the build change, from the workspace root:

```sh
bun install --frozen-lockfile
bun run --filter ./takumi-helpers build
bun run --filter ./takumi-pdf-js build
cp takumi-pdf-js/pkg/takumi_pdf_wasm_bg.wasm /tmp/pdf-baseline.wasm
```

Apply the candidate change, rebuild through the same release pipeline (including wasm-opt), and run:

```sh
bun run --filter ./takumi-pdf-js build
cp takumi-pdf-js/pkg/takumi_pdf_wasm_bg.wasm /tmp/pdf-candidate.wasm
node takumi-pdf-js/tests/bench/index.mjs \
  /tmp/pdf-baseline.wasm /tmp/pdf-candidate.wasm > /tmp/pdf-bench.jsonl
gzip -9 < /tmp/pdf-baseline.wasm | wc -c
gzip -9 < /tmp/pdf-candidate.wasm | wc -c
```

Record `rustc --version`, `wasm-pack --version`, `wasm-opt --version`, `bun --version`, `node --version`, the source revision, and CPU model with the results. The script's gzip sizes use Node's zlib; the commands above separately measure the shipped binary with GNU gzip.

## Read the results

The script uses only the repository's local variable Archivo font. No network is used while measuring. It checks both a small control document and a synthetic 96-row, eight-column ledger.

- Each comparison discards two process warmups, then records nine fresh processes per variant, alternating their order. `wallMs` includes startup **and termination**, not just the promise returning PDF bytes.
- Cold children render once. Warm children reuse one renderer for two unmeasured renders and nine measured renders. The warm `renderMs` summary pools those 81 samples; its `wallMs` covers the whole 11-render process, including the unmeasured renders.
- Each child records JS import, WASM read/init, renderer construction, font read/registration, render time, PDF size, and SHA-256. The driver fails if PDF bytes differ between the variants or repetitions for the same case and mode. Raw per-process samples accompany the median and range.
- `rssKiBAtExit` is the child's high-water RSS **when its JS exit handler runs**, not the native process's final peak. Use `/usr/bin/time -v` around a child to include memory consumed during native teardown.

A short render can finish while V8 is still optimizing WASM functions. Time only `await renderer.render(...)` and that work is missed. This matters particularly when wasm-opt inlines the renderer into one very large export. Diagnose compilation separately; do not enable trace flags in timed runs:

```sh
/usr/bin/time -v node takumi-pdf-js/tests/bench/index.mjs \
  --child /tmp/pdf-candidate.wasm ledger cold
node --trace-wasm-compilation-times takumi-pdf-js/tests/bench/index.mjs \
  --child /tmp/pdf-baseline.wasm control warm > /tmp/pdf-compilation.log
```

Compare unchanged controls and warm render times as well as complete process time. This is not a general throughput benchmark or a replacement for the full JavaScript benchmark required by `CONTRIBUTING.md`.
