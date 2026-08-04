import { type Invoice, money, subtotal } from "./data";

function Meta({ label, value }: { label: string; value: string }) {
  return (
    <div tw="flex gap-3 text-xs">
      <span tw="w-[90px] text-[#687385]">{label}</span>
      <span>{value}</span>
    </div>
  );
}

function Party({ label, party }: { label?: string; party: Invoice["seller"] }) {
  return (
    <div tw="flex w-[220px] flex-col gap-0.5 text-xs">
      <span tw="mb-1 font-semibold">{label ?? party.name}</span>
      {label && <span>{party.name}</span>}
      <span tw="text-[#687385]">{party.address}</span>
      <span tw="text-[#687385]">{party.email}</span>
    </div>
  );
}

export function InvoiceDocument({ data }: { data: Invoice }) {
  const net = subtotal(data);
  const tax = net * data.taxRate;
  const total = net + tax;

  return (
    <div tw="flex w-full flex-col gap-9 text-[#30313d]">
      <div tw="flex items-start justify-between">
        <div tw="flex flex-col gap-4">
          <span tw="text-2xl font-semibold">Invoice</span>
          <div tw="flex flex-col gap-1">
            <Meta label="Invoice number" value={data.number} />
            <Meta label="Date of issue" value={data.issuedAt} />
            <Meta label="Date due" value={data.dueAt} />
          </div>
        </div>
        <img src="logo.svg" tw="h-10 w-10" />
      </div>

      <div tw="flex gap-12">
        <Party party={data.seller} />
        <Party label="Bill to" party={data.buyer} />
      </div>

      <span tw="text-[19px] font-semibold">
        {money(total)} due {data.dueAt}
      </span>

      <div tw="flex flex-col">
        <div tw="flex gap-3 border-b border-[#ebeef1] pb-2 text-[11px] font-semibold text-[#687385]">
          <span tw="flex-1">Description</span>
          <span tw="w-[50px] text-right">Qty</span>
          <span tw="w-[100px] text-right">Unit price</span>
          <span tw="w-[100px] text-right">Amount</span>
        </div>
        {data.items.map((item) => (
          <div key={item.description} tw="flex break-inside-avoid gap-3 pt-3 text-xs">
            <span tw="flex-1">{item.description}</span>
            <span tw="w-[50px] text-right text-[#687385]">{item.quantity}</span>
            <span tw="w-[100px] text-right text-[#687385]">{money(item.unitPrice)}</span>
            <span tw="w-[100px] text-right">{money(item.quantity * item.unitPrice)}</span>
          </div>
        ))}
      </div>

      <div tw="flex break-inside-avoid justify-end">
        <div tw="flex w-[280px] flex-col gap-2.5 text-xs">
          <div tw="flex justify-between">
            <span tw="text-[#687385]">Subtotal</span>
            <span>{money(net)}</span>
          </div>
          <div tw="flex justify-between">
            <span tw="text-[#687385]">Tax ({data.taxRate * 100}%)</span>
            <span>{money(tax)}</span>
          </div>
          <div tw="flex justify-between border-t border-[#ebeef1] pt-2.5 text-[13px] font-semibold">
            <span>Amount due</span>
            <span>{money(total)}</span>
          </div>
        </div>
      </div>

      <span tw="text-[11px] text-[#687385]">{data.notes}</span>
    </div>
  );
}
