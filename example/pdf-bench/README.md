# Compare PDF rendering performance

Render the same invoice with `takumi-pdf`, `@react-pdf/renderer`, and Puppeteer, then compare time and file size.

## Run

Install Google Chrome; Puppeteer launches it with `channel: "chrome"`. Build the [PDF example packages](../README.md#pdf-examples). Then run the benchmark:

```bash
cd example/pdf-bench
bun bench
```

The first run downloads Inter from Google Fonts into `fonts/`. Later runs reuse it.

## Results

Each engine prints one JSON line:

| Field          | Meaning                                                 |
| -------------- | ------------------------------------------------------- |
| `coldMs`       | Time from importing the renderer through the first PDF. |
| `warmMedianMs` | The median time of 20 renders after the first.          |
| `bytes`        | The size of the PDF.                                    |

Each engine also writes its PDF to `out-takumi.pdf`, `out-reactpdf.pdf`, and `out-puppeteer.pdf`.

The [comparison page](https://takumi.kane.tw/docs/pdf/comparison) records results for specific package versions and hardware. Run this benchmark with your own templates to measure your workload.
