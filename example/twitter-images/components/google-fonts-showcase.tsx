import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "google-fonts-showcase";

export const width = 1600;
export const height = 900;

export const fonts = [];

export const images = [{ src: "takumi.svg", path: "takumi.svg" }];

type Greeting = { text: string; family: string; weight: number; size: number; accent?: boolean };

// Greetings across scripts, each in a different Google Font. Latin chrome (brand, tagline)
// runs in Inter. `googleFonts` (see index.tsx) loads every family in one css2 request, and
// `render` keeps only the coverage subsets this content actually draws.
const greetings: Greeting[] = [
  { text: "Hello", family: "Poppins", weight: 700, size: 92, accent: true },
  { text: "你好", family: "Noto Sans SC", weight: 700, size: 100 },
  { text: "Olá", family: "Playfair Display", weight: 700, size: 96 },
  { text: "こんにちは", family: "Noto Sans JP", weight: 500, size: 76 },
  { text: "Привет", family: "Montserrat", weight: 700, size: 84 },
  { text: "안녕하세요", family: "Noto Sans KR", weight: 500, size: 76 },
  { text: "مرحبا", family: "Noto Naskh Arabic", weight: 700, size: 92 },
  { text: "नमस्ते", family: "Noto Sans Devanagari", weight: 600, size: 84 },
  { text: "สวัสดี", family: "Noto Sans Thai", weight: 600, size: 84 },
  { text: "Ciao", family: "Abril Fatface", weight: 400, size: 96 },
  { text: "Xin chào", family: "Inter", weight: 700, size: 80 },
];

const UI_FAMILY = "Inter";

export const googleFonts: GoogleFontFamily[] = [
  { name: UI_FAMILY, weight: 600 },
  ...greetings.map((g) => ({ name: g.family, weight: g.weight })),
];

const INK = "#1d1d1f";
const MUTED = "#86868b";
const ACCENT = "#ff3b30";

export default function GoogleFontsShowcase() {
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
        fontFamily: `${UI_FAMILY}, sans-serif`,
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
        }}
      >
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            justifyContent: "center",
            alignItems: "center",
            maxWidth: "1440px",
            columnGap: "72px",
            rowGap: "40px",
          }}
        >
          {greetings.map((g) => (
            <span
              key={g.text + g.family}
              style={{
                fontFamily: `${g.family}, sans-serif`,
                fontWeight: g.weight,
                fontSize: `${g.size}px`,
                lineHeight: 1,
                color: g.accent ? ACCENT : INK,
              }}
            >
              {g.text}
            </span>
          ))}
        </div>
      </div>

      <p
        style={{
          fontSize: "34px",
          fontWeight: 600,
          color: INK,
          margin: 0,
          textAlign: "center",
          letterSpacing: "-0.015em",
        }}
      >
        It loads only the subsets your content actually renders.
      </p>
    </div>
  );
}
