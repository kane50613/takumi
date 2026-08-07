const items = [
  { description: "Design retainer — March", quantity: 1, unitPrice: 3200 },
  { description: "Component library audit", quantity: 12, unitPrice: 140 },
  { description: "Motion system prototype", quantity: 6, unitPrice: 180 },
  { description: "Handoff workshop", quantity: 2, unitPrice: 450 },
];

const TAX_RATE = 0.05;
const net = items.reduce((sum, item) => sum + item.quantity * item.unitPrice, 0);
const tax = net * TAX_RATE;

const money = (value: number) =>
  value.toLocaleString("en-US", { style: "currency", currency: "USD" });

function Party({ label, name, lines }: { label: string; name: string; lines: string[] }) {
  return (
    <div tw="flex w-[220px] flex-col text-xs">
      <span tw="mb-1 font-semibold text-[#687385]">{label}</span>
      <span tw="font-medium">{name}</span>
      {lines.map((line) => (
        <span key={line} tw="text-[#687385]">
          {line}
        </span>
      ))}
    </div>
  );
}

export default function Invoice() {
  return (
    <div tw="flex w-full flex-col text-[#30313d]">
      <div tw="flex items-start justify-between">
        <div tw="flex flex-col">
          <h1 tw="m-0 text-2xl font-semibold">Invoice INV-2043</h1>
          <p tw="mt-2 mb-0 text-xs text-[#687385]">Issued 2026-03-01 · Due 2026-03-31</p>
        </div>
        <span tw="rounded-full bg-[#eef2ff] px-3 py-1 text-xs font-semibold text-[#4338ca]">
          Unpaid
        </span>
      </div>

      <div tw="mt-8 flex">
        <Party
          label="From"
          name="Takumi Werkstatt"
          lines={["12 Kiln Street, Taipei", "VAT TW-88012345"]}
        />
        <div tw="w-12" />
        <Party
          label="Bill to"
          name="Northwind Studio"
          lines={["490 Harbour Road, Vancouver", "VAT CA-77400221"]}
        />
      </div>

      <h2 tw="mt-8 mb-3 text-sm font-semibold">Lines</h2>
      <div tw="flex border-b border-[#ebeef1] pb-2 text-[11px] font-semibold text-[#687385]">
        <span tw="flex-1">Description</span>
        <span tw="w-[60px] text-right">Qty</span>
        <span tw="w-[100px] text-right">Unit price</span>
        <span tw="w-[100px] text-right">Amount</span>
      </div>
      {items.map((item) => (
        <div key={item.description} tw="flex break-inside-avoid pt-3 text-xs">
          <span tw="flex-1">{item.description}</span>
          <span tw="w-[60px] text-right text-[#687385]">{item.quantity}</span>
          <span tw="w-[100px] text-right text-[#687385]">{money(item.unitPrice)}</span>
          <span tw="w-[100px] text-right">{money(item.quantity * item.unitPrice)}</span>
        </div>
      ))}

      <div tw="mt-8 flex break-inside-avoid justify-end">
        <div tw="flex w-[280px] flex-col text-xs">
          <div tw="flex justify-between">
            <span tw="text-[#687385]">Taxable base</span>
            <span>{money(net)}</span>
          </div>
          <div tw="mt-2 flex justify-between">
            <span tw="text-[#687385]">VAT {TAX_RATE * 100}%</span>
            <span>{money(tax)}</span>
          </div>
          <div tw="mt-3 flex justify-between border-t border-[#ebeef1] pt-3 text-sm font-semibold">
            <span>Total due</span>
            <span>{money(net + tax)}</span>
          </div>
        </div>
      </div>

      <p tw="mt-10 mb-0 text-[11px] text-[#687385]">
        Pay to IBAN TW00 1234 5678 9012, reference INV-2043. You can select the text in the preview,
        because it is text in the PDF rather than an image.
      </p>
    </div>
  );
}

// The same numbers as the page, in the form an accounting system can parse.
// Real e-invoices use the full Factur-X schema.
const invoiceXml = `<?xml version="1.0" encoding="UTF-8"?>
<Invoice>
  <ID>INV-2043</ID>
  <IssueDate>2026-03-01</IssueDate>
  <Currency>USD</Currency>
  <PayableAmount>${(net + tax).toFixed(2)}</PayableAmount>
</Invoice>`;

export const options: PlaygroundOptions = {
  pdf: {
    size: "a4",
    margin: 56,
    // PDF/A-3b archives the document and lets it carry the XML. The render
    // fails if the output does not conform.
    pdfa: "3b",
    attachments: [
      {
        name: "invoice.xml",
        data: invoiceXml,
        mimeType: "application/xml",
        description: "Invoice data",
        relationship: "alternative",
      },
    ],
    metadata: {
      title: "Invoice INV-2043",
      authors: ["Takumi Werkstatt"],
      creationDate: "2026-03-01",
    },
  },
};
