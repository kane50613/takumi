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
