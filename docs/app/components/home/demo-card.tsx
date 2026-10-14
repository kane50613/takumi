export const width = 1200;
export const height = 630;

const RULER = [36, 20, 56, 28, 44, 22, 64, 32, 48, 24, 52, 30];

export default function DemoCard() {
  return (
    <div tw="flex h-full w-full bg-[#181310] p-10 text-[#faf5ee]">
      <div tw="flex flex-1 flex-col justify-between rounded-[20px] border-2 border-[#332922] bg-[#211a15] p-11">
        <div tw="flex items-center justify-between">
          <div tw="flex items-center">
            <svg tw="h-10 w-10" viewBox="0 0 128 128">
              <defs>
                <linearGradient
                  id="handle"
                  gradientUnits="userSpaceOnUse"
                  x1="59.7"
                  y1="8.6"
                  x2="59.7"
                  y2="128"
                >
                  <stop offset="0" stopColor="#ffa944" />
                  <stop offset="1" stopColor="#ff3300" />
                </linearGradient>
                <linearGradient
                  id="head"
                  gradientUnits="userSpaceOnUse"
                  x1="90.7"
                  y1="0"
                  x2="90.7"
                  y2="73.7"
                >
                  <stop offset="0" stopColor="#ff3535" />
                  <stop offset="1" stopColor="#d71d36" />
                </linearGradient>
              </defs>
              <path
                fill="url(#handle)"
                d="M114.3 14.1c1.1.9 3.2 2.7 4.2 4.5s.9 3.5.8 4.8-.4 2.3-2 4.3c-1.7 2-4.7 5-12.8 13.7-8.1 8.8-21.4 23.4-35.3 38.4s-28.6 30.5-36.4 38.7-8.8 8.8-10 9.2-2.5.4-4.1 0-3.5-1.2-6.5-3.8-7.1-6.9-9.4-10S.1 108.8.1 107c0-1.7.4-3.4 5.3-8.6s14.3-13.9 30.5-28.9c16.1-14.9 39-36.1 51-47.2s13.2-12 14.7-12.7 3.2-1.1 4.8-.9 2.8 1 3.9 1.9a32 32 0 0 1 2.5 2.1l.4.5z"
              />
              <path
                fill="url(#head)"
                d="M79 .5C65.3 3.1 46.9 23.4 56.8 36.3c3.3 4.3 5.1 6.7 9.3 9.7 10.2 7.3 39.1 31 53.1 26.9 12-3.5 9.4-16.9 5.6-25.8-1.3-3-25.7-52.8-45.8-46.6"
              />
            </svg>
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
                opacity: index % 2 === 0 ? 0.95 : 0.5,
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
