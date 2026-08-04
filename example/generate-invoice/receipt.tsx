import { type Invoice, money, subtotal } from "./data";

function Divider() {
  return <div tw="border-b border-dashed border-[#d1d5db]" />;
}

function Row({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div tw={`flex justify-between ${strong ? "text-[15px] font-bold" : "text-[#6b7280]"}`}>
      <span>{label}</span>
      <span tw="text-[#111827]">{value}</span>
    </div>
  );
}

export function ReceiptDocument({ data }: { data: Invoice }) {
  const net = subtotal(data);

  return (
    <div tw="flex w-full flex-col gap-3 bg-white p-5 text-[11px] text-[#111827]">
      <div tw="flex flex-col items-center gap-1.5">
        <img src="logo.svg" tw="h-8 w-8" />
        <span tw="text-[15px] font-bold uppercase tracking-[2px]">{data.seller.name}</span>
        <span tw="text-center text-[10px] text-[#6b7280]">{data.seller.address}</span>
      </div>

      <div tw="flex justify-between text-[10px] text-[#6b7280]">
        <span>{data.issuedAt}</span>
        <span>{data.number}</span>
      </div>

      <Divider />

      <div tw="flex flex-col gap-2">
        {data.items.map((item) => (
          <div key={item.description} tw="flex flex-col">
            <span tw="font-medium">{item.description}</span>
            <div tw="flex justify-between text-[#6b7280]">
              <span>
                {item.quantity} × {money(item.unitPrice)}
              </span>
              <span tw="text-[#111827]">{money(item.quantity * item.unitPrice)}</span>
            </div>
          </div>
        ))}
      </div>

      <div tw="flex flex-col gap-1">
        <Row label="Subtotal" value={money(net)} />
        <Row label={`Tax (${data.taxRate * 100}%)`} value={money(net * data.taxRate)} />
      </div>

      <Divider />

      <Row label="Total" value={money(net * (1 + data.taxRate))} strong />

      <span tw="mt-2 text-center text-[10px] tracking-[1px] text-[#6b7280]">
        THANK YOU FOR YOUR BUSINESS
      </span>
    </div>
  );
}
