const items = [
  { name: "Ethiopia Guji · 250g", price: 18 },
  { name: "Cold brew bottle", price: 6.5 },
  { name: "Almond croissant", price: 4.25 },
];

const total = items.reduce((sum, item) => sum + item.price, 0);

const money = (value: number) => `$${value.toFixed(2)}`;

export default function Receipt() {
  return (
    <div tw="flex w-full flex-col bg-white px-5 py-6 font-mono text-[11px] text-black">
      <span tw="text-center text-sm font-bold tracking-widest">KILN COFFEE</span>
      <span tw="mt-1 text-center text-[10px]">12 Kiln Street · Taipei</span>
      <span tw="mt-4 border-t border-black/20 pt-3">Order #A-1182</span>
      <span>2026-03-01 09:14</span>

      <div tw="mt-3 flex flex-col border-t border-black/20 pt-3">
        {items.map((item) => (
          <div key={item.name} tw="flex justify-between">
            <span>{item.name}</span>
            <span>{money(item.price)}</span>
          </div>
        ))}
      </div>

      <div tw="mt-3 flex justify-between border-t border-black/20 pt-3 text-xs font-bold">
        <span>TOTAL</span>
        <span>{money(total)}</span>
      </div>

      <span tw="mt-6 text-center text-[10px]">Thank you — see you next time</span>
    </div>
  );
}

export const options: PlaygroundOptions = {
  // A viewport without a height gives one page sized to the content, like a
  // thermal receipt roll.
  pdf: {
    viewport: { width: 320 },
    metadata: { title: "Kiln Coffee — order A-1182", creationDate: "2026-03-01" },
  },
};
