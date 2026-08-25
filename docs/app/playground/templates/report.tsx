import { PageNumber, TotalPages } from "takumi-pdf/primitives";

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

const days = Array.from({ length: 90 }, (_, index) => ({
  day: new Date(2026, 0, index + 1).toLocaleDateString("en-US", {
    month: "short",
    day: "2-digit",
  }),
  median: 41 - (index % 13),
  p95: 96 - (index % 13) * 2,
  size: 78 - (index % 13) * 1.2,
}));

export default function Report() {
  return (
    <div tw="flex w-full flex-col text-ink">
      <h1 tw="m-0 text-3xl font-semibold">Rendering report</h1>
      <p tw="mt-2 mb-0 text-xs text-faint">
        Q1 2026 · <span tw="text-brand">匠 Werkstatt</span>
      </p>

      {sections.map((section) => (
        <div key={section.title} tw="mt-8 flex flex-col">
          <h2 lang={section.lang} tw="m-0 border-l-4 border-brand pl-3 text-lg font-semibold">
            {section.title}
          </h2>
          <p
            lang={section.lang}
            dir={section.rtl ? "rtl" : undefined}
            tw="mt-2 mb-0 text-sm leading-6 text-body"
          >
            {section.body}
          </p>
        </div>
      ))}

      <h2 tw="mt-8 mb-0 border-l-4 border-brand pl-3 text-lg font-semibold">Daily measurements</h2>
      <table tw="mt-3 w-full text-xs">
        <thead>
          <tr tw="text-[11px] font-semibold text-faint">
            <th tw="border-b-2 border-brand/40 pb-2 text-left">Day</th>
            <th tw="w-[110px] border-b-2 border-brand/40 pb-2 text-right">Median (ms)</th>
            <th tw="w-[110px] border-b-2 border-brand/40 pb-2 text-right">p95 (ms)</th>
            <th tw="w-[110px] border-b-2 border-brand/40 pb-2 text-right">Size (KB)</th>
          </tr>
        </thead>
        <tbody>
          {days.map((row) => (
            <tr key={row.day}>
              <td tw="pt-2">{row.day}</td>
              <td tw="pt-2 text-right text-body">{row.median}</td>
              <td tw="pt-2 text-right text-body">{row.p95}</td>
              <td tw="pt-2 text-right text-body">{row.size.toFixed(1)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export const options: PlaygroundOptions = {
  cssVariables: {
    "--color-brand": "#0d9488",
    "--color-ink": "#1f2430",
    "--color-body": "#374151",
    "--color-faint": "#6b7280",
    "--color-hairline": "#e5e7eb",
  },
  pdf: {
    size: "a4",
    margin: { right: 56, left: 56 },
    header: (
      <div tw="flex w-full flex-col px-14 py-6">
        <div tw="flex w-full items-end justify-between">
          <span tw="text-lg font-semibold text-ink">匠 Werkstatt</span>
          <span tw="text-[10px] text-faint">Rendering report · Q1 2026</span>
        </div>
        <div tw="mt-3 flex h-[3px] w-full bg-brand" />
        <span tw="mt-2 text-[10px] text-faint">Prepared for the platform team</span>
      </div>
    ),
    footer: (
      <div tw="flex w-full flex-col items-center pb-6">
        <div tw="mb-2 flex h-[1px] w-[420px] bg-hairline" />
        <span tw="text-[10px] text-faint">匠 Werkstatt · Kawagoe, Saitama · werkstatt.example</span>
        <span tw="mt-1 text-[10px] text-faint">
          第 <PageNumber format="trad-chinese-informal" /> 頁,共{" "}
          <TotalPages format="trad-chinese-informal" /> 頁
        </span>
      </div>
    ),
    outline: true,
    metadata: { title: "Rendering report — Q1 2026", creationDate: "2026-04-02" },
  },
};
