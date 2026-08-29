import { PageNumber, TotalPages } from "takumi-pdf/primitives";

const days = Array.from({ length: 72 }, (_, index) => ({
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
      <p tw="mt-2 mb-6 text-xs text-faint">Median render time and output size, day by day</p>

      <table tw="w-full text-xs">
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
  css: {
    selector: ":root",
    style: {
      "--color-brand": "#0d9488",
      "--color-ink": "#1f2430",
      "--color-body": "#374151",
      "--color-faint": "#6b7280",
    },
  },
  pdf: {
    size: "a4",
    margin: { right: 56, left: 56 },
    footer: (
      <div tw="flex w-full justify-center pb-6 text-[10px] text-faint">
        <span>
          <PageNumber /> / <TotalPages />
        </span>
      </div>
    ),
    metadata: { title: "Rendering report — Q1 2026", creationDate: "2026-04-02" },
  },
};
