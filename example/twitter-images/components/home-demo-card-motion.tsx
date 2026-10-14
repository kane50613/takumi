export const name = "home-demo-card-motion";

export const fonts = [];

export const width = 1200;
export const height = 630;

export const animation = { durationMs: 1200, fps: 24, quality: 82 };

export const css = [
  `@keyframes pulseBar {
    0% { transform: scaleY(0.42); }
    50% { transform: scaleY(1.12); }
    100% { transform: scaleY(0.42); }
  }
  @keyframes strikeSpark {
    0% { transform: rotate(0deg) scale(1); border-radius: 22%; }
    50% { transform: rotate(90deg) scale(0.8); border-radius: 50%; }
    100% { transform: rotate(180deg) scale(1); border-radius: 22%; }
  }`,
];

const RULER = [36, 20, 56, 28, 44, 22, 64, 32, 48, 24, 52, 30];

export default function DemoCardMotion() {
  return (
    <div tw="flex h-full w-full bg-[#181310] p-10 text-[#faf5ee]">
      <div tw="flex flex-1 flex-col justify-between rounded-[20px] border-2 border-[#332922] bg-[#211a15] p-11">
        <div tw="flex items-center justify-between">
          <div tw="flex items-center">
            <div
              tw="h-9 w-9"
              style={{
                backgroundImage: "linear-gradient(135deg, #ffa944, #ff3300)",
                animation: "strikeSpark 1200ms cubic-bezier(0.65, 0, 0.35, 1) infinite",
              }}
            />
            <span tw="ml-4 text-2xl font-bold tracking-tight text-[#f6efe6]">takumi.kane.tw</span>
          </div>
          <span tw="text-xl font-semibold text-[#b5a695]">PNG · WebP · SVG · PDF · Paint</span>
        </div>

        <div tw="my-auto flex flex-col">
          <h1
            tw="text-[78px] font-extrabold leading-[0.98] tracking-[-0.03em]"
            style={{
              backgroundClip: "text",
              backgroundImage: "linear-gradient(108deg, #fff8f0 45%, #ffa944 75%, #ff451a 100%)",
              color: "transparent",
            }}
          >
            One component, every output.
          </h1>
        </div>

        <div tw="flex items-center justify-between text-xl text-[#b5a695]">
          <span>JSX &amp; CSS rendered without a browser</span>
          <span tw="font-bold text-[#ffa944]">1200 × 630</span>
        </div>
      </div>

      <div
        tw="ml-6 flex w-[272px] flex-col justify-between rounded-[20px] p-9 text-white"
        style={{
          backgroundImage: "linear-gradient(165deg, #ff5d2e 0%, #e5341f 55%, #b81921 100%)",
        }}
      >
        <span tw="text-4xl font-extrabold leading-[1.05] tracking-tight">Crafted in Rust.</span>

        <div tw="flex h-[104px] items-end justify-between rounded-[14px] bg-[#181310]/25 px-4 py-4">
          {RULER.map((barHeight, index) => (
            <div
              key={index}
              tw="w-[9px] rounded-full bg-[#fff6ec]"
              style={{
                height: `${barHeight}px`,
                opacity: index % 2 === 0 ? 0.95 : 0.55,
                transformOrigin: "bottom",
                animation: `pulseBar 1200ms ease-in-out -${(index * 100) % 1200}ms infinite`,
              }}
            />
          ))}
        </div>

        <div tw="flex flex-col text-xl font-bold text-[#fff6ec]">
          <span>Layout · Text · Paint</span>
          <span tw="mt-1.5 text-base font-medium text-[#ffe2d1]">Node · Bun · Wasm · Rust</span>
        </div>
      </div>
    </div>
  );
}
