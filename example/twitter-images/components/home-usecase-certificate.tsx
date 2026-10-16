import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "home-usecase-certificate";

export const title = "Certificate of Completion, Lin Wei-Ting";

// A4 landscape at 96 dpi.
export const width = 1123;
export const height = 794;

export const outputs = [] as const;

export const svg = true;

export const googleFonts: GoogleFontFamily[] = [
  { name: "Cormorant Garamond", weight: [500, 600] },
  { name: "Cormorant Garamond", weight: 500, style: "italic" },
  { name: "Mrs Saint Delafield", weight: 400 },
  { name: "Inter", weight: [400, 600] },
];

const IVORY = "#f8f4ea";
const INK = "#22302a";
const MUTED = "#6e7570";
const GREEN = "#1f3a2e";
const GOLD = "#b08d57";

type Signer = { signature: string; name: string; role: string };

const lead: Signer = { signature: "Adaeze Nwosu", name: "Adaeze Nwosu", role: "Course lead" };
const director: Signer = {
  signature: "T. Herrera",
  name: "Tomás Herrera",
  role: "Program director",
};

function Signature({ signature, name, role }: Signer) {
  return (
    <div tw="flex w-[240px] flex-col items-center">
      <span
        tw="text-[44px] leading-none"
        style={{ fontFamily: "Mrs Saint Delafield", color: GREEN }}
      >
        {signature}
      </span>
      <div tw="mt-1 h-px w-full" style={{ backgroundColor: GOLD }} />
      <span tw="mt-3 text-[14px] font-semibold">{name}</span>
      <span tw="text-[13px]" style={{ color: MUTED }}>
        {role}
      </span>
    </div>
  );
}

function Seal() {
  return (
    <div
      tw="flex h-[132px] w-[132px] items-center justify-center rounded-full"
      style={{ backgroundImage: `repeating-conic-gradient(${GOLD} 0deg 6deg, #c9a874 6deg 12deg)` }}
    >
      <div
        tw="flex h-[104px] w-[104px] flex-col items-center justify-center rounded-full border-2"
        style={{ backgroundColor: GREEN, borderColor: "#d9c08f", color: "#efe2c2" }}
      >
        <span tw="text-[10px] font-semibold tracking-[0.2em]">WITH</span>
        <span tw="text-[10px] font-semibold tracking-[0.2em]">DISTINCTION</span>
        <span tw="mt-1 text-[22px]" style={{ fontFamily: "Cormorant Garamond", fontWeight: 600 }}>
          2026
        </span>
      </div>
    </div>
  );
}

export default function Certificate() {
  return (
    <div
      tw="relative flex h-full w-full flex-col items-center p-16"
      style={{ backgroundColor: IVORY, color: INK, fontFamily: "Inter" }}
    >
      <div tw="absolute inset-7 border-[5px] border-double" style={{ borderColor: GOLD }} />
      <div tw="absolute inset-10 border" style={{ borderColor: "#e3d6bb" }} />

      <span tw="mt-6 text-[13px] font-semibold tracking-[0.34em]" style={{ color: GREEN }}>
        NORTHLIGHT SCHOOL OF DESIGN
      </span>
      <span
        tw="mt-7 text-[56px] leading-none"
        style={{ fontFamily: "Cormorant Garamond", fontWeight: 500 }}
      >
        Certificate of Completion
      </span>
      <span tw="mt-9 text-[15px]" style={{ color: MUTED }}>
        This certifies that
      </span>
      <span
        tw="mt-3 text-[76px] leading-none"
        style={{ fontFamily: "Cormorant Garamond", fontWeight: 600, color: GREEN }}
      >
        Lin Wei-Ting
      </span>
      <span tw="mt-7 text-[15px]" style={{ color: MUTED }}>
        has completed
      </span>
      <span
        tw="mt-2 text-[36px] leading-none"
        style={{ fontFamily: "Cormorant Garamond", fontStyle: "italic", fontWeight: 500 }}
      >
        Typography for Interfaces
      </span>
      <span tw="mt-3 text-[15px]" style={{ color: MUTED }}>
        A six-week course of 18 lessons and four critiqued projects
      </span>

      <div tw="mt-auto flex w-full items-end justify-between px-20">
        <Signature {...lead} />
        <Seal />
        <Signature {...director} />
      </div>
      <span tw="mt-7 text-[12px] tracking-[0.04em]" style={{ color: MUTED }}>
        Awarded October 9, 2026 · Credential TFI-2026-0418
      </span>
    </div>
  );
}
