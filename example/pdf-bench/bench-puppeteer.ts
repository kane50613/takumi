import { interFonts } from "./fonts";
import { items, total } from "./invoice-data";

const fonts = await interFonts();
const regular = Buffer.from(await Bun.file(fonts.regular).arrayBuffer()).toString("base64");
const bold = Buffer.from(await Bun.file(fonts.bold).arrayBuffer()).toString("base64");

const t0 = performance.now();
const puppeteer = (await import("puppeteer-core")).default;

const rows = items
  .map(
    (item) => `
    <div class="row" style="break-inside: avoid">
      <span class="description">${item.description}</span>
      <span class="qty">${item.qty}</span>
      <span class="price">$${(item.qty * item.unit).toFixed(2)}</span>
    </div>`,
  )
  .join("");

const html = `<!doctype html>
<html><head><style>
  @font-face { font-family: Inter; font-weight: 400; src: url(data:font/ttf;base64,${regular}) }
  @font-face { font-family: Inter; font-weight: 700; src: url(data:font/ttf;base64,${bold}) }
  body { font-size: 13px; color: #111827; font-family: Inter, sans-serif; margin: 0 }
  .header { display: flex; justify-content: space-between; border-bottom: 1px solid #d1d5db; padding-bottom: 16px; margin-bottom: 16px }
  h1 { font-size: 24px; margin: 0 }
  p { margin: 0 }
  .row { display: flex; padding: 4px 0 }
  .description { flex: 1; padding-right: 16px }
  .qty { width: 32px; text-align: right }
  .price { width: 90px; text-align: right }
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
  await page.evaluateHandle("document.fonts.ready");
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
