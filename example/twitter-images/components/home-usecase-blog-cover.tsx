import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "home-usecase-blog-cover";

export const width = 1200;
export const height = 630;

export const outputs = [{ ratio: 1, format: "png" }] as const;

export const googleFonts: GoogleFontFamily[] = [
  { name: "Fraunces", weight: 600 },
  { name: "Inter", weight: [400, 600] },
];

const PAPER = "#f2ede3";
const INK = "#1c1a17";
const MUTED = "#6f6a60";
const NIGHT = "#13233a";
const BEAM = "#f6c453";

function Lighthouse() {
  return (
    <div tw="relative flex h-full w-[420px] overflow-hidden" style={{ backgroundColor: NIGHT }}>
      <div
        tw="absolute left-[-140px] top-[92px] h-[300px] w-[560px]"
        style={{
          clipPath: "polygon(100% 18%, 52% 50%, 100% 82%)",
          backgroundImage: `linear-gradient(270deg, ${BEAM}00, ${BEAM}66)`,
        }}
      />
      <div
        tw="absolute left-[176px] top-[214px] h-[56px] w-[56px] rounded-full"
        style={{ backgroundColor: BEAM, boxShadow: `0 0 60px 24px ${BEAM}55` }}
      />
      <div tw="absolute left-[176px] top-[236px] flex h-[34px] w-[56px] bg-[#0b1626]" />
      <div
        tw="absolute left-[164px] top-[270px] flex h-[220px] w-[80px] flex-col"
        style={{ clipPath: "polygon(22% 0, 78% 0, 100% 100%, 0 100%)" }}
      >
        <div tw="h-[44px] w-full bg-[#e9e4d8]" />
        <div tw="h-[44px] w-full bg-[#c4473a]" />
        <div tw="h-[44px] w-full bg-[#e9e4d8]" />
        <div tw="h-[44px] w-full bg-[#c4473a]" />
        <div tw="h-[44px] w-full bg-[#e9e4d8]" />
      </div>
      <div tw="absolute bottom-0 left-0 h-[140px] w-full bg-[#0b1626]" />
      <div tw="absolute bottom-[112px] left-[60px] h-[2px] w-[72px] bg-[#2c4466]" />
      <div tw="absolute bottom-[84px] left-[268px] h-[2px] w-[96px] bg-[#2c4466]" />
      <div tw="absolute bottom-[52px] left-[120px] h-[2px] w-[140px] bg-[#2c4466]" />
    </div>
  );
}

export default function BlogCover() {
  return (
    <div
      tw="flex h-full w-full"
      style={{ backgroundColor: PAPER, color: INK, fontFamily: "Inter" }}
    >
      <div tw="flex flex-1 flex-col justify-between py-16 pl-16 pr-14">
        <div tw="flex items-center text-[22px] font-semibold">
          <span>Field Notes</span>
          <span tw="mx-3" style={{ color: MUTED }}>
            /
          </span>
          <span style={{ color: MUTED }}>Reliability</span>
        </div>
        <h1
          tw="m-0 text-[64px] leading-[1.04]"
          style={{
            fontFamily: "Fraunces",
            fontWeight: 600,
            letterSpacing: "-0.02em",
            textWrap: "balance",
          }}
        >
          What a lighthouse keeper taught us about being on call
        </h1>
        <div tw="flex items-center text-[22px]">
          <div
            tw="flex h-14 w-14 items-center justify-center rounded-full text-[20px] font-semibold text-white"
            style={{ backgroundColor: "#c4473a" }}
          >
            MO
          </div>
          <div tw="ml-4 flex flex-col">
            <span tw="font-semibold">Mara Okafor</span>
            <span style={{ color: MUTED }}>Oct 2, 2026 · 12 min read</span>
          </div>
        </div>
      </div>
      <Lighthouse />
    </div>
  );
}
