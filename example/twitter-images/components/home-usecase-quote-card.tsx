import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "home-usecase-quote-card";

export const width = 1080;
export const height = 1080;

export const outputs = [{ ratio: 1, format: "webp" }] as const;

export const googleFonts: GoogleFontFamily[] = [
  { name: "Noto Serif TC", weight: [500, 900] },
  { name: "Noto Serif JP", weight: 500 },
  { name: "Noto Serif KR", weight: 500 },
  { name: "Cormorant Garamond", weight: 500, style: "italic" },
  { name: "Inter", weight: 500 },
];

// Read top to bottom, columns right to left.
const columns = ["千里之行", "始於足下"];

const translations = [
  { lang: "日本語", text: "千里の行も足下より始まる", family: "Noto Serif JP" },
  { lang: "한국어", text: "천 리 길도 한 걸음부터", family: "Noto Serif KR" },
];

const PAPER = "#efe7d6";
const INK = "#211d18";
const MUTED = "#7d7262";
const SEAL = "#b3342b";

export default function QuoteCard() {
  return (
    <div
      tw="flex h-full w-full p-24"
      style={{ backgroundColor: PAPER, color: INK, fontFamily: "Inter" }}
    >
      <div tw="flex flex-1 flex-col justify-between pr-16">
        <span tw="text-[22px] tracking-[0.2em]" style={{ color: MUTED }}>
          DAILY PROVERB · NO. 64
        </span>
        <div tw="flex flex-col">
          <span
            tw="text-[60px] leading-[1.12]"
            style={{ fontFamily: "Cormorant Garamond", fontStyle: "italic", textWrap: "balance" }}
          >
            “A journey of a thousand miles begins beneath one’s feet.”
          </span>
          {translations.map((line) => (
            <div key={line.lang} tw="mt-10 flex flex-col">
              <span tw="text-[18px] tracking-[0.12em]" style={{ color: MUTED }}>
                {line.lang}
              </span>
              <span tw="mt-2 text-[34px]" style={{ fontFamily: line.family }}>
                {line.text}
              </span>
            </div>
          ))}
        </div>
        <div tw="flex flex-col text-[22px]">
          <span style={{ fontFamily: "Noto Serif TC" }}>老子《道德經》第六十四章</span>
          <span tw="mt-1" style={{ color: MUTED }}>
            Laozi, Tao Te Ching, chapter 64
          </span>
        </div>
      </div>

      <div tw="flex flex-col items-end justify-between">
        <div tw="flex flex-row-reverse">
          {columns.map((column) => (
            <div key={column} tw="ml-6 flex flex-col">
              {[...column].map((char, index) => (
                <span
                  key={`${char}${index}`}
                  tw="text-[128px] leading-[1.16]"
                  style={{ fontFamily: "Noto Serif TC", fontWeight: 900 }}
                >
                  {char}
                </span>
              ))}
            </div>
          ))}
        </div>
        <div
          tw="flex h-[104px] w-[104px] flex-col items-center justify-center rounded-[10px]"
          style={{
            backgroundColor: SEAL,
            color: PAPER,
            fontFamily: "Noto Serif TC",
            fontWeight: 900,
          }}
        >
          {["足", "下"].map((char) => (
            <span key={char} tw="text-[40px] leading-none">
              {char}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
