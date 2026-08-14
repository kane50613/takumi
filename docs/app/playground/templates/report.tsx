const sections = [
  {
    title: "Summary",
    lang: "en",
    body: "Render throughput held steady through the quarter while the output size dropped by a fifth. The gains came from the layout cache and a tighter content stream, not from lowering quality.",
  },
  {
    title: "本季重點",
    lang: "zh-Hant",
    body: "中位數渲染時間從 41 毫秒降到 28 毫秒。冷啟動仍然主導第一個請求,所以共用的 renderer 值得留著不要關掉。字型子集化改成逐份文件丟掉沒用到的字符,四頁的發票從 78 KB 降到 62 KB。",
  },
  {
    title: "この四半期の成果",
    lang: "ja",
    body: "レイアウトキャッシュにより、同じ文書を二度組む必要がなくなりました。日本語の本文も欧文と同じ経路で組版され、行分割は言語ごとの規則に従います。",
  },
  {
    title: "ملخص",
    lang: "ar",
    rtl: true,
    body: "يجري النص العربي من اليمين إلى اليسار، ويحتفظ الجدول أدناه باتجاهه الأصلي.",
  },
];

const weeks = Array.from({ length: 84 }, (_, index) => ({
  week: `W${String(index + 1).padStart(2, "0")}`,
  median: 41 - (index % 13),
  p95: 96 - (index % 13) * 2,
  size: 78 - (index % 13) * 1.2,
}));

export default function Report() {
  return (
    <div tw="flex w-full flex-col text-[#1f2430]">
      <h1 tw="m-0 text-3xl font-semibold">Rendering report</h1>
      <p tw="mt-2 mb-0 text-xs text-[#6b7280]">Q1 2026 · 匠 Werkstatt</p>

      {sections.map((section) => (
        <div key={section.title} tw="mt-8 flex flex-col">
          <h2 lang={section.lang} tw="m-0 text-lg font-semibold">
            {section.title}
          </h2>
          <p
            lang={section.lang}
            dir={section.rtl ? "rtl" : undefined}
            tw="mt-2 mb-0 text-sm leading-6 text-[#374151]"
          >
            {section.body}
          </p>
        </div>
      ))}

      <h2 tw="mt-8 mb-0 text-lg font-semibold">Weekly measurements</h2>
      <div tw="mt-3 flex border-b border-[#e5e7eb] pb-2 text-[11px] font-semibold text-[#6b7280]">
        <span tw="flex-1">Week</span>
        <span tw="w-[110px] text-right">Median (ms)</span>
        <span tw="w-[110px] text-right">p95 (ms)</span>
        <span tw="w-[110px] text-right">Size (KB)</span>
      </div>
      {weeks.map((row) => (
        <div key={row.week} tw="flex break-inside-avoid pt-2 text-xs">
          <span tw="flex-1">{row.week}</span>
          <span tw="w-[110px] text-right text-[#374151]">{row.median}</span>
          <span tw="w-[110px] text-right text-[#374151]">{row.p95}</span>
          <span tw="w-[110px] text-right text-[#374151]">{row.size.toFixed(1)}</span>
        </div>
      ))}
    </div>
  );
}

export const options: PlaygroundOptions = {
  pdf: {
    size: "a4",
    margin: { right: 56, left: 56 },
    header: (
      <div tw="flex w-full flex-col px-14 py-6">
        <div tw="flex w-full items-end justify-between">
          <span tw="text-lg font-semibold text-[#1f2430]">匠 Werkstatt</span>
          <span tw="text-[10px] text-[#9ca3af]">Rendering report · Q1 2026</span>
        </div>
        <div tw="mt-3 flex h-[3px] w-full bg-[#1f2430]" />
        <span tw="mt-2 text-[10px] text-[#9ca3af]">Prepared for the platform team</span>
      </div>
    ),
    footer: (
      <div tw="flex w-full flex-col items-center pb-6">
        <div tw="mb-2 flex h-[1px] w-[420px] bg-[#e5e7eb]" />
        <span tw="text-[10px] text-[#9ca3af]">
          匠 Werkstatt · Kawagoe, Saitama · werkstatt.example
        </span>
        <span tw="mt-1 text-[10px] text-[#9ca3af]">
          第 <span className="pageNumber trad-chinese-informal" /> 頁,共{" "}
          <span className="totalPages trad-chinese-informal" /> 頁
        </span>
      </div>
    ),
    outline: true,
    metadata: { title: "Rendering report — Q1 2026", creationDate: "2026-04-02" },
  },
};
