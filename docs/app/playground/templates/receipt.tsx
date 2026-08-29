const items = [
  { name: "Guji washed · 250g", price: 18 },
  { name: "Cold brew", price: 6 },
  { name: "Almond croissant", price: 4 },
];

const total = items.reduce((sum, item) => sum + item.price, 0);

export default function Receipt() {
  return (
    <div tw="flex w-full flex-col bg-white px-5 py-6 text-[11px] text-black">
      <span tw="text-center text-sm font-bold tracking-[6px]">KILN COFFEE</span>
      <span tw="mt-1 text-center text-[10px]">12 Kiln Street · 555-2345</span>

      <span tw="mt-4 border-t border-black/20 pt-3">Order A-1182</span>
      <span>2026-03-01 09:14</span>

      <div tw="mt-3 flex flex-col border-t border-black/20 pt-3">
        {items.map((item) => (
          <div key={item.name} tw="flex justify-between">
            <span>{item.name}</span>
            <span>${item.price}</span>
          </div>
        ))}
      </div>

      <div tw="mt-3 flex justify-between border-t border-black/20 pt-3 text-xs font-bold">
        <span>Total</span>
        <span>${total}</span>
      </div>

      <span tw="mt-6 text-center text-[10px]">Thanks, see you soon ☕</span>
    </div>
  );
}

export const options: PlaygroundOptions = {
  pdf: {
    viewport: { width: 320 },
    metadata: { title: "Kiln Coffee · Order A-1182", creationDate: "2026-03-01" },
  },
};
