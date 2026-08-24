const accents = ["coral", "honey", "mint", "sky"];

const greetings = [
  { lang: "en", label: "English", text: "Hello, world" },
  { lang: "zh-Hant", label: "繁體中文", text: "你好,世界" },
  { lang: "ja", label: "日本語", text: "こんにちは世界" },
  { lang: "ko", label: "한국어", text: "안녕하세요 세계" },
  { lang: "ar", label: "العربية", text: "مرحبا بالعالم", rtl: true },
  { lang: "he", label: "עברית", text: "שלום עולם", rtl: true },
  { lang: "hi", label: "हिन्दी", text: "नमस्ते दुनिया" },
  { lang: "th", label: "ไทย", text: "สวัสดีชาวโลก" },
];

export default function Multilingual() {
  return (
    <div tw="flex h-full w-full flex-col bg-canvas p-16 text-white">
      <h1 tw="m-0 text-5xl font-bold tracking-tight">One font stack, every script 🌏</h1>
      <p tw="mt-3 mb-10 text-2xl text-subtle">
        Each line picks the face that covers it, and Arabic and Hebrew lay out right to left.
      </p>

      <div tw="flex flex-wrap">
        {greetings.map((greeting, index) => {
          const accent = accents[index % accents.length];

          return (
            <div
              key={greeting.lang}
              tw={`mr-4 mb-4 flex w-[330px] flex-col rounded-2xl border-l-4 border-${accent} bg-white/5 px-6 py-4`}
            >
              <span tw={`text-lg text-${accent}`}>{greeting.label}</span>
              <span
                lang={greeting.lang}
                dir={greeting.rtl ? "rtl" : undefined}
                tw="mt-1 text-4xl font-semibold"
              >
                {greeting.text}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
  variables: {
    "--color-canvas": "#0b1020",
    "--color-subtle": "#94a3b8",
    "--color-coral": "#fb7185",
    "--color-honey": "#fbbf24",
    "--color-mint": "#34d399",
    "--color-sky": "#38bdf8",
  },
};
