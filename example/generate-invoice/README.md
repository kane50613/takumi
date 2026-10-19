# Generate PDF invoices and receipts

Render a paged A4 invoice and a single-page receipt from the same sample data with [`takumi-pdf`](../../takumi-pdf-js).

## Run

Build the [PDF example packages](../README.md#pdf-examples) first. Then render both documents:

```bash
cd example/generate-invoice
bun index.tsx
```

The script writes `output/invoice.pdf` and `output/receipt.pdf`. It loads Inter and Space Mono from Google Fonts, so it needs network access.

## How the two documents differ

- **Invoice:** `size: "a4"` flows content across A4 pages. The footer repeats on every page, and its `pageNumber` and `totalPages` spans receive the page counters.
- **Receipt:** `viewport: { width: 302 }` renders one page 80 mm wide at 96 dpi. Without a `height`, the page grows to fit the content.
