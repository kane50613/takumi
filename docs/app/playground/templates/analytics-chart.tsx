import { init, use } from "echarts/core";
import { BarChart, LineChart } from "echarts/charts";
import { GridComponent } from "echarts/components";
import { SVGRenderer } from "echarts/renderers";

use([BarChart, LineChart, GridComponent, SVGRenderer]);

const start = new Date(2026, 7, 3);
const renders = Array.from({ length: 28 }, (_, index) => {
  const weekend = index % 7 >= 5;
  const base = (2400 + index * 65) * (weekend ? 0.55 : 1);

  return Math.round(base + Math.sin(index * 1.3) * 140);
});
const average = renders.map((_, index) => {
  const window = renders.slice(Math.max(0, index - 6), index + 1);

  return Math.round(window.reduce((sum, value) => sum + value, 0) / window.length);
});
const labels = renders.map((_, index) => {
  const date = new Date(start);

  date.setDate(start.getDate() + index);

  return date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
});
const total = renders.reduce((sum, value) => sum + value, 0);

const chart = init(null, null, { renderer: "svg", ssr: true, width: 1072, height: 300 });

chart.setOption({
  animation: false,
  grid: { left: 0, right: 0, top: 10, bottom: 0 },
  xAxis: {
    type: "category",
    data: labels,
    axisLine: { show: false },
    axisTick: { show: false },
    axisLabel: { color: "#666666", fontSize: 13, margin: 14, interval: 6 },
  },
  yAxis: {
    type: "value",
    splitLine: { lineStyle: { color: "#1f1f1f" } },
    axisLabel: { color: "#666666", fontSize: 13 },
  },
  series: [
    {
      type: "bar",
      data: renders,
      itemStyle: { color: "#0070f3", borderRadius: [3, 3, 0, 0] },
      barWidth: 16,
    },
    {
      type: "line",
      data: average,
      symbol: "none",
      lineStyle: { color: "#ededed", width: 2 },
    },
  ],
});

const svg = chart.renderToSVGString();

chart.dispose();

export default function AnalyticsChart() {
  return (
    <div tw="flex h-full w-full bg-black p-10 text-white">
      <div tw="flex h-full w-full flex-col rounded-xl border border-[#1f1f1f] bg-[#0a0a0a] p-10">
        <div tw="flex items-center justify-between">
          <span tw="text-sm font-medium tracking-widest text-[#888888] uppercase">Renders</span>
          <div tw="flex items-center text-sm text-[#888888]">
            <span tw="h-2.5 w-2.5 rounded-sm bg-[#0070f3]" />
            <span tw="ml-2">daily</span>
            <span tw="ml-5 h-0.5 w-4 bg-[#ededed]" />
            <span tw="ml-2">7-day average</span>
          </div>
        </div>
        <div tw="mt-2 flex items-baseline">
          <span tw="text-5xl font-semibold tracking-tight">{total.toLocaleString("en-US")}</span>
          <span tw="ml-4 text-lg text-[#666666]">past 4 weeks · weekends dip, trend climbs</span>
        </div>
        <img src={svg} width={1072} height={300} tw="mt-8" />
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = { width: 1200, height: 630, format: "png" };
