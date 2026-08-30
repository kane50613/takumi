import { init, use } from "echarts/core";
import { BarChart, LineChart } from "echarts/charts";
import { GridComponent } from "echarts/components";
import { SVGRenderer } from "echarts/renderers";
import { PageNumber, TotalPages } from "takumi-pdf/primitives";

use([BarChart, LineChart, GridComponent, SVGRenderer]);

const days = Array.from({ length: 72 }, (_, index) => {
  const released = index >= 30;
  const median = Math.round((44 + Math.sin(index * 0.9) * 3) * (released ? 0.72 : 1));
  const incident = index === 12 || index === 47 ? 55 : 0;
  const weekend = index % 7 >= 5 ? 0.6 : 1;
  const png = Math.round((1850 + index * 6 + Math.sin(index * 1.3) * 120) * weekend);
  const webp = Math.round((640 + index * 5) * weekend);
  const pdf = Math.round((released ? 180 + (index - 30) * 14 : 0) * weekend);

  return {
    day: new Date(2026, 0, index + 1).toLocaleDateString("en-US", {
      month: "short",
      day: "2-digit",
    }),
    median,
    p95: Math.round(median * 2.1 + Math.sin(index * 1.7) * 6 + incident),
    png,
    webp,
    pdf,
  };
});

const weeks = Array.from({ length: Math.floor(days.length / 7) }, (_, index) => {
  const chunk = days.slice(index * 7, index * 7 + 7);
  const sum = (key: "png" | "webp" | "pdf") => chunk.reduce((total, row) => total + row[key], 0);

  return { label: chunk[0].day, png: sum("png"), webp: sum("webp"), pdf: sum("pdf") };
});

const axis = {
  axisLine: { show: false },
  axisTick: { show: false },
  axisLabel: { color: "#6b7280", fontSize: 9 },
};

function chartSvg(height: number, option: object) {
  const chart = init(null, null, { renderer: "svg", ssr: true, width: 483, height });

  chart.setOption({
    animation: false,
    textStyle: { fontFamily: "Noto Sans, Geist" },
    grid: { left: 0, right: 0, top: 8, bottom: 0 },
    yAxis: {
      type: "value",
      splitLine: { lineStyle: { color: "#e5e7eb" } },
      axisLabel: axis.axisLabel,
    },
    ...option,
  });

  const svg = chart.renderToSVGString();

  chart.dispose();

  return svg;
}

const latencySvg = chartSvg(160, {
  xAxis: {
    type: "category",
    data: days.map((row) => row.day),
    ...axis,
    axisLabel: { ...axis.axisLabel, interval: 12 },
  },
  series: [
    {
      type: "line",
      data: days.map((row) => row.median),
      symbol: "none",
      lineStyle: { color: "#0d9488", width: 1.5 },
    },
    {
      type: "line",
      data: days.map((row) => row.p95),
      symbol: "none",
      lineStyle: { color: "#6b7280", width: 1, type: "dashed" },
    },
  ],
});

const volumeSvg = chartSvg(150, {
  xAxis: { type: "category", data: weeks.map((week) => week.label), ...axis },
  series: [
    {
      type: "bar",
      stack: "renders",
      data: weeks.map((week) => week.png),
      itemStyle: { color: "#0d9488" },
      barWidth: 22,
    },
    {
      type: "bar",
      stack: "renders",
      data: weeks.map((week) => week.webp),
      itemStyle: { color: "#5eead4" },
    },
    {
      type: "bar",
      stack: "renders",
      data: weeks.map((week) => week.pdf),
      itemStyle: { color: "#9ca3af" },
      barWidth: 22,
    },
  ],
});

function Legend({ entries }: { entries: [string, string][] }) {
  return (
    <div tw="mt-5 flex items-center text-[10px] text-faint">
      {entries.map(([color, label]) => (
        <div key={label} tw="mr-4 flex items-center">
          <span tw="h-2 w-2 rounded-sm" style={{ backgroundColor: color }} />
          <span tw="ml-1.5">{label}</span>
        </div>
      ))}
    </div>
  );
}

export default function Report() {
  return (
    <div tw="flex w-full flex-col text-ink">
      <h1 tw="m-0 text-3xl font-semibold">Rendering report</h1>
      <p tw="mt-2 mb-0 text-xs text-faint">
        Q1 2026 · v1.4 shipped Jan 31: layout got 28% cheaper and PDF output went live
      </p>

      <Legend
        entries={[
          ["#0d9488", "median (ms)"],
          ["#6b7280", "p95 (ms)"],
        ]}
      />
      <img src={latencySvg} width={483} height={160} tw="mt-2" />

      <Legend
        entries={[
          ["#0d9488", "PNG"],
          ["#5eead4", "WebP"],
          ["#9ca3af", "PDF"],
        ]}
      />
      <img src={volumeSvg} width={483} height={150} tw="mt-2 mb-6" />

      <table tw="w-full text-xs">
        <thead>
          <tr tw="text-[11px] font-semibold text-faint">
            <th tw="border-b-2 border-brand/40 pb-2 text-left">Day</th>
            <th tw="w-[110px] border-b-2 border-brand/40 pb-2 text-right">Median (ms)</th>
            <th tw="w-[110px] border-b-2 border-brand/40 pb-2 text-right">p95 (ms)</th>
            <th tw="w-[110px] border-b-2 border-brand/40 pb-2 text-right">Renders</th>
          </tr>
        </thead>
        <tbody>
          {days.map((row) => (
            <tr key={row.day}>
              <td tw="pt-2">{row.day}</td>
              <td tw="pt-2 text-right text-body">{row.median}</td>
              <td tw="pt-2 text-right text-body">{row.p95}</td>
              <td tw="pt-2 text-right text-body">
                {(row.png + row.webp + row.pdf).toLocaleString("en-US")}
              </td>
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
