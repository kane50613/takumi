export const width = 1200;
export const height = 675;

export const name = "v1";
export const fonts = [
  "geist/Geist[wght].woff2",
  { path: "geist/GeistMono[wght].woff2", generic: "monospace" as const },
];

const Logo = () => (
  <svg width="52" height="52" viewBox="0 0 128 128" tw="mr-8">
    <path
      fill="#18181b"
      d="M114.3 14.1c1.1.9 3.2 2.7 4.2 4.5s.9 3.5.8 4.8-.4 2.3-2 4.3c-1.7 2-4.7 5-12.8 13.7-8.1 8.8-21.4 23.4-35.3 38.4s-28.6 30.5-36.4 38.7-8.8 8.8-10 9.2-2.5.4-4.1 0-3.5-1.2-6.5-3.8-7.1-6.9-9.4-10S.1 108.8.1 107c0-1.7.4-3.4 5.3-8.6s14.3-13.9 30.5-28.9c16.1-14.9 39-36.1 51-47.2s13.2-12 14.7-12.7 3.2-1.1 4.8-.9 2.8 1 3.9 1.9a32 32 0 0 1 2.5 2.1l.4.5z"
    />
    <path
      fill="#18181b"
      d="M79 .5C65.3 3.1 46.9 23.4 56.8 36.3c3.3 4.3 5.1 6.7 9.3 9.7 10.2 7.3 39.1 31 53.1 26.9 12-3.5 9.4-16.9 5.6-25.8-1.3-3-25.7-52.8-45.8-46.6"
    />
  </svg>
);

export default function V1() {
  return (
    <div tw="w-full h-full bg-[#09090b] flex items-center justify-center relative overflow-hidden">
      <div
        tw="absolute flex flex-col justify-center"
        style={{ width: 2400, height: 1600, left: -600, top: -450, transform: "rotate(-12deg)" }}
      >
        {Array.from({ length: 18 }).map((_, i) => (
          <div
            key={i}
            tw={`m-0 text-[64px] leading-[0.9] font-black text-[#18181b] whitespace-nowrap flex items-center`}
            style={{
              marginLeft: `-${(i * 137) % 600}px`,
            }}
          >
            {Array.from({ length: 15 }).map((_, j) => (
              <div key={j} tw="flex items-center shrink-0 -ml-4">
                <span tw="mr-4">TAKUMI</span>
                <span tw="mr-4">TAKUMI</span>
                <Logo />
              </div>
            ))}
          </div>
        ))}
      </div>

      <div tw="flex flex-col items-center justify-center relative z-10">
        <h1
          tw="m-0 text-[#fafafa] text-[120px] font-bold tracking-tighter leading-none"
          style={{ textShadow: "0 12px 40px rgba(0, 0, 0, 0.8)" }}
        >
          V1 Released.
        </h1>
        <div
          tw="mt-12 flex items-center px-8 py-4 bg-[#18181b] rounded-full border border-[#27272a]"
          style={{ boxShadow: "0 8px 32px rgba(0, 0, 0, 0.6)" }}
        >
          <span tw="font-mono text-[#71717a] text-[32px] mr-5">$</span>
          <span tw="font-mono text-[#e4e4e7] text-[32px] tracking-tight">bun i takumi-js@1</span>
        </div>
      </div>
    </div>
  );
}
