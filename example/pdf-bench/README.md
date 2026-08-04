# pdf-bench

Renders the same two-page invoice with takumi-pdf, @react-pdf/renderer, and Puppeteer (system Chrome), printing cold start, warm median over 20 runs, and output size.

```bash
bun install
bun bench
```

Puppeteer uses `channel: "chrome"`, so a local Google Chrome install is required. Numbers feed the [comparison page](https://takumi.kane.tw/docs/pdf/comparison).
