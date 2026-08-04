export const invoice = {
  number: "INV-2026-0042",
  issuedAt: "2026-08-04",
  dueAt: "2026-09-03",
  seller: {
    name: "Kagi Studio Co.",
    address: "12F, 88 Songren Rd, Xinyi District, Taipei 110",
    email: "billing@kagi.studio",
  },
  buyer: {
    name: "Northwind Trading Ltd.",
    address: "3 Harbour View St, Central, Hong Kong",
    email: "ap@northwind.example",
  },
  items: [
    { description: "Design system audit", quantity: 1, unitPrice: 2400 },
    { description: "Landing page implementation", quantity: 3, unitPrice: 1150 },
    { description: "PDF export integration", quantity: 1, unitPrice: 1800 },
    { description: "CI pipeline setup", quantity: 2, unitPrice: 650 },
    { description: "Monthly maintenance (July)", quantity: 1, unitPrice: 900 },
  ],
  taxRate: 0.05,
  notes: "Payment by bank transfer within 30 days. Please reference the invoice number.",
};

export type Invoice = typeof invoice;

const usd = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });

export function money(value: number): string {
  return usd.format(value);
}

export function subtotal(data: Invoice): number {
  return data.items.reduce((sum, item) => sum + item.quantity * item.unitPrice, 0);
}
