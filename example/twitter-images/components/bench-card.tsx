export const name = "bench-card";

export const width = 1200;
export const height = 675;

export const fonts = ["geist/Geist[wght].woff2"];

export const images = [{ src: "takumi.svg", path: "takumi.svg" }];

const bars = [
  { name: "takumi-pdf", ms: 26, accent: true },
  { name: "Puppeteer + Chrome", ms: 201, accent: false },
  { name: "@react-pdf/renderer", ms: 279, accent: false },
];

const max = Math.max(...bars.map((bar) => bar.ms));

export default function BenchCard() {
  return (
    <div tw="flex h-full w-full flex-col bg-[#0b0d12] p-16 text-white">
      <div tw="flex items-center">
        <img src="takumi.svg" tw="h-14 w-14" />
        <span tw="ml-5 text-5xl font-bold">takumi-pdf 0.2</span>
        <span tw="ml-6 text-3xl text-gray-400">warm render, two-page invoice</span>
      </div>
      <div tw="mt-14 flex flex-1 flex-col justify-center">
        {bars.map((bar) => (
          <div key={bar.name} tw="mb-9 flex flex-col">
            <div tw="flex items-baseline justify-between">
              <span tw="text-3xl text-gray-200">{bar.name}</span>
              <span tw={`text-3xl font-bold ${bar.accent ? "text-[#ffa944]" : "text-gray-400"}`}>
                {bar.ms} ms
              </span>
            </div>
            <div tw="mt-3 flex h-7 w-full rounded-full bg-[#1a1f2b]">
              <div
                tw={`flex h-7 rounded-full ${bar.accent ? "" : "bg-[#475569]"}`}
                style={{
                  width: `${(bar.ms / max) * 100}%`,
                  ...(bar.accent && {
                    backgroundImage: "linear-gradient(90deg, #ffa944, #ff3300)",
                  }),
                }}
              />
            </div>
          </div>
        ))}
      </div>
      <div tw="flex justify-between text-2xl text-gray-500">
        <span>same layout, same embedded Inter, identical two-page output</span>
        <span>M1 Pro · median of 20</span>
      </div>
    </div>
  );
}
