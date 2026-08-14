const items = [
  { name: "衣索比亞 古吉 · 250g", price: 560 },
  { name: "冷萃咖啡", price: 200 },
  { name: "杏仁可頌", price: 130 },
];

const total = items.reduce((sum, item) => sum + item.price, 0);

export default function Receipt() {
  return (
    <div lang="zh-Hant" tw="flex w-full flex-col bg-white px-5 py-6 text-[11px] text-black">
      <span tw="text-center text-sm font-bold tracking-[6px]">窯 咖 啡</span>
      <span tw="mt-1 text-center text-[10px]">台北市窯街 12 號 · 02-2345-6789</span>

      <span tw="mt-4 border-t border-black/20 pt-3">訂單 A-1182</span>
      <span>2026-03-01 09:14</span>

      <div tw="mt-3 flex flex-col border-t border-black/20 pt-3">
        {items.map((item) => (
          <div key={item.name} tw="flex justify-between">
            <span>{item.name}</span>
            <span>NT${item.price}</span>
          </div>
        ))}
      </div>

      <div tw="mt-3 flex justify-between border-t border-black/20 pt-3 text-xs font-bold">
        <span>合計</span>
        <span>NT${total}</span>
      </div>

      <span tw="mt-6 text-center text-[10px]">謝謝光臨,下次見 ☕</span>
    </div>
  );
}

export const options: PlaygroundOptions = {
  pdf: {
    viewport: { width: 320 },
    metadata: { title: "窯咖啡 · 訂單 A-1182", creationDate: "2026-03-01" },
    lang: "zh-Hant",
  },
};
