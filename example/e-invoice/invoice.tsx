import { type Invoice, money, totals } from "./facturx";

function Party({ label, party }: { label: string; party: Invoice["seller"] }) {
  return (
    <div tw="flex w-[220px] flex-col gap-0.5 text-xs">
      <span tw="mb-1 font-semibold text-[#687385]">{label}</span>
      <span>{party.name}</span>
      <span tw="text-[#687385]">{party.address}</span>
      <span tw="text-[#687385]">VAT {party.vatId}</span>
    </div>
  );
}

export function InvoiceDocument({ data }: { data: Invoice }) {
  const { net, tax, gross } = totals(data);

  return (
    <div tw="flex w-full flex-col gap-8 text-[#30313d]">
      <div tw="flex items-start justify-between">
        <div tw="flex flex-col">
          <h1 tw="m-0 text-2xl font-semibold">Invoice {data.number}</h1>
          <p tw="mt-2 mb-0 text-xs text-[#687385]">
            Issued {data.issuedAt} · Due {data.dueAt}
          </p>
        </div>
        <img src="logo.svg" alt="Takumi Werkstatt" tw="h-10 w-10" />
      </div>

      <div tw="flex gap-12">
        <Party label="From" party={data.seller} />
        <Party label="Bill to" party={data.buyer} />
      </div>

      <div tw="flex flex-col">
        <h2 tw="m-0 mb-3 text-sm font-semibold">Lines</h2>
        <div tw="flex gap-3 border-b border-[#ebeef1] pb-2 text-[11px] font-semibold text-[#687385]">
          <span tw="flex-1">Description</span>
          <span tw="w-[60px] text-right">Qty</span>
          <span tw="w-[100px] text-right">Unit price</span>
          <span tw="w-[100px] text-right">Amount</span>
        </div>
        {data.items.map((item) => (
          <div key={item.description} tw="flex break-inside-avoid gap-3 pt-3 text-xs">
            <span tw="flex-1">{item.description}</span>
            <span tw="w-[60px] text-right text-[#687385]">{item.quantity}</span>
            <span tw="w-[100px] text-right text-[#687385]">
              {money(item.unitPrice, data.currency)}
            </span>
            <span tw="w-[100px] text-right">
              {money(item.quantity * item.unitPrice, data.currency)}
            </span>
          </div>
        ))}
      </div>

      <div tw="flex break-inside-avoid justify-end">
        <div tw="flex w-[280px] flex-col gap-2.5 text-xs">
          <div tw="flex justify-between">
            <span tw="text-[#687385]">Taxable base</span>
            <span>{money(net, data.currency)}</span>
          </div>
          <div tw="flex justify-between">
            <span tw="text-[#687385]">VAT ({data.taxRate * 100}%)</span>
            <span>{money(tax, data.currency)}</span>
          </div>
          <div tw="flex justify-between border-t border-[#ebeef1] pt-2.5 text-[13px] font-semibold">
            <span>Amount due</span>
            <span>{money(gross, data.currency)}</span>
          </div>
        </div>
      </div>

      <p tw="m-0 text-[11px] text-[#687385]">
        The machine-readable copy of this invoice is attached as factur-x.xml.
      </p>
    </div>
  );
}
