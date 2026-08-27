import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "google-fonts-showcase";

// Logical size; dpr 1.2 renders straight to 1920x1080.
export const width = 1600;

export const height = 900;

const SEG_MS = 1150;
const INK = "#1d1d1f";
const MUTED = "#86868b";
const UI = "Inter";
const UI_TEXT = "On-demand Google Fonts, rendered without a browser";

// A line of well-known verse per language, each in a Google Font chosen to suit the verse's
// mood — brush scripts for the East-Asian classics, literary serifs for the Western ones.
const segments = [
  { text: "To be, or not to be", family: "Playfair Display", weight: 500, size: 120 },
  { text: "床前明月光", family: "Ma Shan Zheng", weight: 400, size: 208 },
  { text: "古池や蛙飛びこむ", family: "Yuji Syuku", weight: 400, size: 150 },
  { text: "별 헤는 밤", family: "Nanum Myeongjo", weight: 700, size: 176 },
  { text: "Я вас любил", family: "Lora", weight: 500, size: 152 },
  { text: "سجِّل أنا عربي", family: "Amiri", weight: 400, size: 156 },
  { text: "सारे जहाँ से अच्छा", family: "Rozha One", weight: 400, size: 132 },
  { text: "ความรักเหมือนโรคา", family: "Mali", weight: 500, size: 128 },
  { text: "Nel mezzo del cammin", family: "Cormorant", weight: 600, size: 144 },
  { text: "Navegar é preciso", family: "Cinzel", weight: 500, size: 116 },
  { text: "Trăm năm trong cõi người ta", family: "Playfair Display", weight: 500, size: 96 },
];

export const fonts = [];

export const googleFonts: GoogleFontFamily[] = [
  ...segments.map((seg) => ({ name: seg.family, weight: seg.weight })),
  { name: UI, weight: 600 },
];

export const images = [{ src: "takumi.svg", path: "takumi.svg" }];

export const css = [
  `@keyframes slide {
    0% { opacity: 0; transform: translateY(34px) scale(0.99); }
    24% { opacity: 1; transform: translateY(0) scale(1); }
    76% { opacity: 1; transform: translateY(0) scale(1); }
    100% { opacity: 0; transform: translateY(-34px) scale(0.99); }
  }`,
];

export const video = { durationMs: segments.length * SEG_MS, fps: 60, dpr: 1.2 };

function segmentAt(ms: number) {
  return Math.min(Math.floor(ms / SEG_MS), segments.length - 1);
}

export function fontFamilies(ms: number) {
  const seg = segments[segmentAt(ms)];

  return seg ? [seg.family, UI] : [UI];
}

export function frame(ms: number) {
  return <Still index={segmentAt(ms)} />;
}

export default function GoogleFontsVideoStill() {
  return <Still index={0} />;
}

// The entrance animation is delayed to the segment's slot on the global
// timeline, so the shared pipeline's single timeMs drives every segment.
function Still({ index }: { index: number }) {
  const seg = segments[index] ?? segments[0];

  if (!seg) return null;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        padding: "72px 96px",
        backgroundColor: "#f5f5f7",
        backgroundImage: "radial-gradient(120% 90% at 50% 0%, #ffffff 0%, #ededf0 100%)",
        color: INK,
        fontFamily: `${UI}, sans-serif`,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "16px" }}>
          <img src="takumi.svg" alt="" style={{ width: "44px", height: "44px" }} />
          <span style={{ fontSize: "34px", fontWeight: 600, letterSpacing: "-0.02em" }}>
            Takumi
          </span>
        </div>
        <span style={{ fontSize: "26px", fontWeight: 600, color: MUTED, letterSpacing: "0.04em" }}>
          On-demand Google Fonts
        </span>
      </div>

      <div
        style={{
          display: "flex",
          flex: 1,
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          gap: "36px",
          animation: `slide ${SEG_MS}ms cubic-bezier(0.4, 0, 0.2, 1) both`,
          animationDelay: `${index * SEG_MS}ms`,
        }}
      >
        <span
          style={{
            fontFamily: `${seg.family}, sans-serif`,
            fontWeight: seg.weight,
            fontSize: `${seg.size}px`,
            lineHeight: 1,
          }}
        >
          {seg.text}
        </span>
        <span
          style={{
            fontSize: "30px",
            fontWeight: 600,
            color: MUTED,
            letterSpacing: "0.1em",
            textTransform: "uppercase",
          }}
        >
          {seg.family}
        </span>
      </div>

      <div style={{ display: "flex", justifyContent: "center" }}>
        <span style={{ fontSize: "30px", fontWeight: 600, color: MUTED }}>{UI_TEXT}</span>
      </div>
    </div>
  );
}
