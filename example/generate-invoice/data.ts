export const invoice = {
  number: "INV-2026-0128",
  issuedAt: "2026-08-04",
  dueAt: "2026-09-03",
  seller: {
    name: "Takumi Woodworks",
    address: "12F, 88 Songren Rd, Xinyi District, Taipei 110",
    email: "workshop@takumi.kane.tw",
  },
  buyer: {
    name: "Puppeteer & Sons Marionette Co.",
    address: "404 Headless Way, DevTools District",
    email: "ap@puppeteer.example",
  },
  items: [
    { description: "Kerning chisel, hand-ground", quantity: 2, unitPrice: 1280 },
    { description: "Baseline alignment jig", quantity: 1, unitPrice: 3600 },
    { description: "Subpixel sanding block, 1/256 grit", quantity: 3, unitPrice: 480 },
    { description: "Glyph cache, walnut, 512 × 512", quantity: 1, unitPrice: 5120 },
    { description: "Serif plane, No. 4", quantity: 1, unitPrice: 2350 },
    { description: "Ligature glue, 200 ml", quantity: 2, unitPrice: 360 },
    { description: "Font subsetting saw, WOFF2 blade", quantity: 1, unitPrice: 4200 },
    { description: "Brotli compression clamp", quantity: 4, unitPrice: 590 },
    { description: "Anti-aliasing rasp, fine", quantity: 1, unitPrice: 880 },
    { description: "Bézier curve template set", quantity: 1, unitPrice: 1650 },
    { description: "Whitespace, archival grade, 1 ream", quantity: 2, unitPrice: 250 },
    { description: "Line-height spacer set, metric", quantity: 1, unitPrice: 720 },
    { description: "Orphan & widow guard rail", quantity: 2, unitPrice: 940 },
    { description: "Page break wedge, clean cut", quantity: 3, unitPrice: 310 },
    { description: "Flexbox joinery workshop, 2 hr", quantity: 1, unitPrice: 3200 },
    { description: "Grid layout dovetail template", quantity: 1, unitPrice: 1480 },
    { description: "Border-radius router bit, 8 px", quantity: 2, unitPrice: 675 },
    { description: "Overflow clamp, heavy duty", quantity: 1, unitPrice: 1120 },
    { description: "Text-shadow stain, 30% gray", quantity: 1, unitPrice: 540 },
    { description: "CJK line-breaking ruler", quantity: 1, unitPrice: 980 },
    { description: "Tofu remover, missing-glyph filler", quantity: 1, unitPrice: 460 },
    { description: "Chromium removal service", quantity: 1, unitPrice: 0 },
  ],
  taxRate: 0.05,
  notes:
    "Hand-finished in Taipei. Payment by bank transfer within 30 days; please reference the invoice number. No Chromium was used.",
};

export type Invoice = typeof invoice;

const twd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "TWD",
  currencyDisplay: "code",
  minimumFractionDigits: 0,
});

export function money(value: number): string {
  return twd.format(value);
}

export function subtotal(data: Invoice): number {
  return data.items.reduce((sum, item) => sum + item.quantity * item.unitPrice, 0);
}
