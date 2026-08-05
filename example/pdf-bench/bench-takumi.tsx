import { items, total } from "./invoice-data";

const t0 = performance.now();
const { render } = await import("takumi-pdf");

function Invoice() {
  return (
    <main tw="flex flex-col text-[13px] text-gray-900">
      <div tw="flex justify-between border-b border-gray-300 pb-4 mb-4">
        <h1 tw="text-2xl font-bold">Invoice INV-2026-001</h1>
        <p>Due August 31, 2026</p>
      </div>
      <div tw="flex flex-col">
        {items.map((item, i) => (
          <div key={i} tw="flex justify-between py-1" style={{ breakInside: "avoid" }}>
            <span tw="w-4/5">{item.description}</span>
            <span>{item.qty}</span>
            <span>${(item.qty * item.unit).toFixed(2)}</span>
          </div>
        ))}
      </div>
      <div tw="flex justify-between border-t border-gray-300 mt-4 pt-2 font-bold">
        <span>Total</span>
        <span>${total.toFixed(2)}</span>
      </div>
    </main>
  );
}

const options = {
  size: "a4",
  footer: (
    <div tw="flex w-full justify-center text-[10px] text-gray-500">
      Page <span className="pageNumber" /> of <span className="totalPages" />
    </div>
  ),
} as const;

const first = await render(<Invoice />, options);
const coldMs = performance.now() - t0;

const times: number[] = [];
for (let i = 0; i < 20; i++) {
  const start = performance.now();
  await render(<Invoice />, options);
  times.push(performance.now() - start);
}
times.sort((a, b) => a - b);

await Bun.write("out-takumi.pdf", first);
console.log(
  JSON.stringify({
    engine: "takumi-pdf",
    coldMs: Math.round(coldMs),
    warmMedianMs: Math.round((times[9]! + times[10]!) / 2),
    bytes: first.byteLength,
  }),
);
