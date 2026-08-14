const items = [
  { name: "設計月費 · Design retainer", qty: 1, price: 3200 },
  { name: "元件庫檢查 · Component audit", qty: 12, price: 140 },
  { name: "動效原型 · Motion prototype", qty: 6, price: 180 },
];

const TAX_RATE = 0.05;
const net = items.reduce((sum, item) => sum + item.qty * item.price, 0);
const tax = net * TAX_RATE;

const money = (value: number) => `$${value.toLocaleString("en-US")}`;

export default function Invoice() {
  return (
    <div lang="zh-Hant" tw="flex w-full flex-col text-[#30313d]">
      <div tw="flex items-start justify-between">
        <div tw="flex flex-col">
          <h1 tw="m-0 text-2xl font-semibold">發票 INV-2043</h1>
          <p tw="mt-2 mb-0 text-xs text-[#687385]">開立 2026-03-01 · 到期 2026-03-31</p>
        </div>
        <span tw="rounded-full bg-[#eef2ff] px-3 py-1 text-xs font-semibold text-[#4338ca]">
          未付款
        </span>
      </div>

      <div tw="mt-8 flex text-xs">
        <div tw="flex w-[240px] flex-col">
          <span tw="mb-1 font-semibold text-[#687385]">賣方</span>
          <span>匠 Werkstatt · 台北市窯街 12 號</span>
          <span tw="text-[#687385]">統一編號 88012345</span>
        </div>
        <div tw="flex w-[240px] flex-col">
          <span tw="mb-1 font-semibold text-[#687385]">買方</span>
          <span>Northwind Studio · Vancouver</span>
          <span tw="text-[#687385]">VAT CA-77400221</span>
        </div>
      </div>

      <div tw="mt-8 flex border-b border-[#ebeef1] pb-2 text-[11px] font-semibold text-[#687385]">
        <span tw="flex-1">品項</span>
        <span tw="w-[60px] text-right">數量</span>
        <span tw="w-[100px] text-right">單價</span>
        <span tw="w-[100px] text-right">金額</span>
      </div>
      {items.map((item) => (
        <div key={item.name} tw="flex break-inside-avoid pt-3 text-xs">
          <span tw="flex-1">{item.name}</span>
          <span tw="w-[60px] text-right text-[#687385]">{item.qty}</span>
          <span tw="w-[100px] text-right text-[#687385]">{money(item.price)}</span>
          <span tw="w-[100px] text-right">{money(item.qty * item.price)}</span>
        </div>
      ))}

      <div tw="mt-8 flex break-inside-avoid justify-end">
        <div tw="flex w-[260px] flex-col text-xs">
          <div tw="flex justify-between">
            <span tw="text-[#687385]">未稅金額</span>
            <span>{money(net)}</span>
          </div>
          <div tw="mt-2 flex justify-between">
            <span tw="text-[#687385]">營業稅 {TAX_RATE * 100}%</span>
            <span>{money(tax)}</span>
          </div>
          <div tw="mt-3 flex justify-between border-t border-[#ebeef1] pt-3 text-sm font-semibold">
            <span>應付總額</span>
            <span>{money(net + tax)}</span>
          </div>
        </div>
      </div>

      <p tw="mt-10 mb-0 text-[11px] text-[#687385]">
        文件與附件都通過 PDF/A-3b 驗證。中文字是真的文字,可以選取,不是圖片。
      </p>
    </div>
  );
}

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
      title: "發票 INV-2043",
      authors: ["匠 Werkstatt"],
      creationDate: "2026-03-01",
    },
    lang: "zh-Hant",
  },
};
