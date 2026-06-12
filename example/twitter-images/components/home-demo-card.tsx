export const name = "home-demo-card";

export const persistentImages = [];
export const fonts = [];

export const width = 1200;
export const height = 630;

// Shown verbatim as the code sample on the docs homepage; keep it in sync.
export default function DemoCard() {
  return (
    <div tw="flex h-full w-full flex-col justify-between bg-[#16130f] p-14 text-white">
      <div tw="flex items-center justify-between">
        <span tw="text-2xl text-[#a8a29a]">takumi.kane.tw</span>
        <span tw="h-10 w-10 bg-[#ff4d4d]" />
      </div>
      <h1
        tw="text-7xl font-bold leading-tight"
        style={{
          backgroundClip: "text",
          backgroundImage: "linear-gradient(110deg, #fff 60%, #ff4d4d)",
          color: "transparent",
        }}
      >
        This card is the code beside it.
      </h1>
      <div tw="flex items-center justify-between text-2xl text-[#a8a29a]">
        <span>Rendered without a browser</span>
        <span>1200 × 630</span>
      </div>
    </div>
  );
}
