import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "home-usecase-now-playing";

export const width = 960;
export const height = 360;

export const googleFonts: GoogleFontFamily[] = [{ name: "Manrope", weight: [500, 800] }];

export const animation = { durationMs: 1200, fps: 20, quality: 80 };

export const css = [
  `@keyframes level {
    0% { height: 22%; }
    30% { height: 100%; }
    55% { height: 48%; }
    80% { height: 84%; }
    100% { height: 22%; }
  }`,
];

// Every duration divides the 1200 ms loop, so the last frame meets the first.
const durations = [600, 1200, 400, 600, 1200];
const bars = Array.from({ length: 34 }, (_, index) => ({
  durationMs: durations[index % durations.length] ?? 600,
  delayMs: -((index * 173) % 1200),
}));

const ACCENT = "#ff9e6d";
const MUTED = "#a99f9a";

function Cover() {
  return (
    <div tw="relative flex h-[264px] w-[264px] flex-col overflow-hidden rounded-[18px]">
      <div tw="flex h-full w-full" style={{ backgroundColor: "#f6b26b" }} />
      <div
        tw="absolute left-[76px] top-[64px] h-[112px] w-[112px] rounded-full"
        style={{ backgroundColor: "#fff1d6" }}
      />
      <div tw="absolute left-0 top-[150px] h-[114px] w-full bg-[#28466e]" />
      {[0, 1, 2].map((row) => (
        <div
          key={row}
          tw="absolute h-[6px] rounded-full bg-[#fff1d6]"
          style={{
            top: `${166 + row * 22}px`,
            left: `${88 + row * 10}px`,
            width: `${88 - row * 20}px`,
            opacity: 0.8 - row * 0.15,
          }}
        />
      ))}
      <span
        tw="absolute left-5 top-4 text-[15px] font-extrabold tracking-[0.18em]"
        style={{ color: "#28466e" }}
      >
        SALT & STATIC
      </span>
    </div>
  );
}

export default function NowPlaying() {
  return (
    <div
      tw="flex h-full w-full items-center bg-[#1b1716] p-12 text-white"
      style={{ fontFamily: "Manrope" }}
    >
      <Cover />
      <div tw="ml-12 flex h-[264px] flex-1 flex-col justify-between">
        <div tw="flex flex-col">
          <span tw="text-[16px] font-extrabold tracking-[0.2em]" style={{ color: ACCENT }}>
            NOW PLAYING
          </span>
          <span
            tw="mt-3 text-[44px] font-extrabold leading-none"
            style={{ letterSpacing: "-0.02em" }}
          >
            Low Tide, High Static
          </span>
          <span tw="mt-3 text-[22px] font-medium" style={{ color: MUTED }}>
            Juniper Hall · Salt & Static, track 4
          </span>
        </div>

        <div tw="flex h-[56px] items-end justify-between">
          {bars.map((bar, index) => (
            <div
              key={index}
              tw="w-[10px] rounded-full"
              style={{
                backgroundColor: index < 16 ? ACCENT : "#4a403c",
                animation: `level ${bar.durationMs}ms ease-in-out ${bar.delayMs}ms infinite`,
              }}
            />
          ))}
        </div>

        <div tw="flex flex-col">
          <div tw="flex h-[6px] w-full rounded-full bg-[#3a3230]">
            <div tw="h-full w-[46%] rounded-full" style={{ backgroundColor: ACCENT }} />
          </div>
          <div tw="mt-3 flex justify-between text-[17px] font-medium" style={{ color: MUTED }}>
            <span>1:47</span>
            <span>3:52</span>
          </div>
        </div>
      </div>
    </div>
  );
}
