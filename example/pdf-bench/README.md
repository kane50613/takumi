# Compare PDF rendering performance

Render the same invoice with `takumi-pdf`, `@react-pdf/renderer`, and Puppeteer using system Chrome. The benchmark reports cold-start time, the median of 20 warm renders, and PDF size.

```bash
bun install
bun bench
```

Puppeteer uses `channel: "chrome"`, so a local Google Chrome install is required. The [comparison page](https://takumi.kane.tw/docs/pdf/comparison) records results for specific package versions and hardware. Run this benchmark with your own templates to evaluate your workload.
