export const name = "home-demo-card-motion";

export const fonts = [];

export const width = 1200;
export const height = 630;

export const animation = { durationMs: 1000, fps: 30, quality: 80 };

export const css = [
  `@keyframes morph {
    from { border-radius: 12%; transform: rotate(0deg) scale(1); }
    50% { border-radius: 50%; transform: rotate(90deg) scale(0.72); }
    to { border-radius: 12%; transform: rotate(180deg) scale(1); }
  }`,
];

// home-demo-card.tsx with the square animated.
export default function DemoCardMotion() {
  return (
    <div tw="flex h-full w-full flex-col justify-between bg-[#16130f] p-14 text-white">
      <div tw="flex items-center justify-between">
        <span tw="text-2xl text-[#a8a29a]">takumi.kane.tw</span>
        <span
          tw="h-10 w-10 bg-[#ff4d4d]"
          style={{ animation: "morph 1000ms cubic-bezier(0.65, 0, 0.35, 1) infinite" }}
        />
      </div>
      <h1
        tw="text-7xl font-bold leading-tight"
        style={{
          backgroundClip: "text",
          backgroundImage: "linear-gradient(110deg, #fff 60%, #ff4d4d)",
          color: "transparent",
        }}
      >
        One component, every output.
      </h1>
      <div tw="flex items-center justify-between text-2xl text-[#a8a29a]">
        <span>Rendered without a browser</span>
        <span>1200 × 630</span>
      </div>
    </div>
  );
}
