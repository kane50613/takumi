const sections = [
  {
    title: "Summary",
    body: "Render throughput held steady through the quarter while the output size dropped by a fifth. The gains came from the layout cache and a tighter content stream, not from lowering quality.",
  },
  {
    title: "Throughput",
    body: "Median render time fell from 41 ms to 28 ms on the reference document. Cold starts still dominate the first request, so the shared renderer is worth keeping alive between calls.",
  },
  {
    title: "Output size",
    body: "Font subsetting now drops unused glyphs per document instead of per family. A four-page invoice ships 62 KB, down from 78 KB.",
  },
  {
    title: "Next quarter",
    body: "Tagged output moves to the default path, and the archival levels get a validator run in CI so a regression fails the build instead of a customer's upload.",
  },
];

const weeks = Array.from({ length: 13 }, (_, index) => ({
  week: `W${String(index + 1).padStart(2, "0")}`,
  median: 41 - index,
  p95: 96 - index * 2,
  size: 78 - index * 1.2,
}));

export default function Report() {
  return (
    <div tw="flex w-full flex-col text-[#1f2430]">
      <h1 tw="m-0 text-3xl font-semibold">Rendering report</h1>
      <p tw="mt-2 mb-0 text-xs text-[#6b7280]">Q1 2026 · Takumi Werkstatt</p>

      {sections.map((section) => (
        <div key={section.title} tw="mt-8 flex flex-col">
          <h2 tw="m-0 text-lg font-semibold">{section.title}</h2>
          <p tw="mt-2 mb-0 text-sm leading-6 text-[#374151]">{section.body}</p>
          <p tw="mt-3 mb-0 text-sm leading-6 text-[#374151]">
            {section.body} {section.body}
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
    margin: { top: 72, right: 56, bottom: 72, left: 56 },
    header: (
      <div tw="flex w-full justify-between px-14 pt-6 text-[10px] text-[#9ca3af]">
        <span>Rendering report</span>
        <span>Q1 2026</span>
      </div>
    ),
    footer: (
      <div tw="flex w-full justify-center pb-6 text-[10px] text-[#9ca3af]">
        Page <span className="pageNumber" /> of <span className="totalPages" />
      </div>
    ),
    // Turns the h1/h2 headings into PDF bookmarks.
    outline: true,
    metadata: { title: "Rendering report — Q1 2026", creationDate: "2026-04-02" },
  },
};
