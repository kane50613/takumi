# generate-invoice

Generates a paged A4 invoice and a single-page receipt from the same fake data, using [takumi-pdf](../../takumi-pdf-js).

Build the wasm package first (needs [Rust](https://www.rust-lang.org/tools/install) and [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)):

```bash
cd takumi-pdf-js
bun run build
```

Then run the example:

```bash
cd example/generate-invoice
bun index.tsx
```

Open `output/invoice.pdf` and `output/receipt.pdf` to see the results.

The invoice uses the paged mode: content flows across A4 pages, and the footer repeats with `pageNumber` / `totalPages` counters. The receipt uses `viewport` for a fixed 80mm-style single page.
