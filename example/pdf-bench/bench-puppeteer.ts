import { items, total } from "./invoice-data";

const t0 = performance.now();
const puppeteer = (await import("puppeteer-core")).default;

const rows = items
  .map(
    (item) => `
    <div class="row" style="break-inside: avoid">
      <span class="description">${item.description}</span>
      <span>${item.qty}</span>
      <span>$${(item.qty * item.unit).toFixed(2)}</span>
    </div>`,
  )
  .join("");

const html = `<!doctype html>
<html><head><style>
  body { font-size: 13px; color: #111827; font-family: system-ui, sans-serif; margin: 0 }
  .header { display: flex; justify-content: space-between; border-bottom: 1px solid #d1d5db; padding-bottom: 16px; margin-bottom: 16px }
  h1 { font-size: 24px; margin: 0 }
  .row { display: flex; justify-content: space-between; padding: 4px 0 }
  .description { width: 80% }
  .total { display: flex; justify-content: space-between; border-top: 1px solid #d1d5db; margin-top: 16px; padding-top: 8px; font-weight: 700 }
</style></head><body>
  <div class="header"><h1>Invoice INV-2026-001</h1><p>Due August 31, 2026</p></div>
  ${rows}
  <div class="total"><span>Total</span><span>$${total.toFixed(2)}</span></div>
</body></html>`;

const browser = await puppeteer.launch({
  channel: "chrome",
  headless: true,
});

async function renderOnce(): Promise<Uint8Array> {
  const page = await browser.newPage();
  await page.setContent(html, { waitUntil: "load" });
  const pdf = await page.pdf({
    format: "a4",
    margin: { top: "48px", right: "48px", bottom: "48px", left: "48px" },
    displayHeaderFooter: true,
    headerTemplate: "<span></span>",
    footerTemplate:
      '<div style="width:100%;text-align:center;font-size:10px;color:#6b7280">Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>',
  });
  await page.close();
  return pdf;
}

const first = await renderOnce();
const coldMs = performance.now() - t0;

const times: number[] = [];
for (let i = 0; i < 20; i++) {
  const start = performance.now();
  await renderOnce();
  times.push(performance.now() - start);
}
times.sort((a, b) => a - b);

await Bun.write("out-puppeteer.pdf", first);
console.log(
  JSON.stringify({
    engine: "puppeteer + chrome",
    coldMs: Math.round(coldMs),
    warmMedianMs: Math.round((times[9]! + times[10]!) / 2),
    bytes: first.byteLength,
  }),
);

await browser.close();
