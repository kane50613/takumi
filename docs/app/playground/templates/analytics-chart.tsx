import { init, use } from "echarts/core";
import { BarChart } from "echarts/charts";
import { GridComponent } from "echarts/components";
import { SVGRenderer } from "echarts/renderers";

use([BarChart, GridComponent, SVGRenderer]);

const chart = init(null, null, { renderer: "svg", ssr: true, width: 1072, height: 320 });

chart.setOption({
  animation: false,
  grid: { left: 0, right: 0, top: 10, bottom: 0, containLabel: true },
  xAxis: {
    type: "category",
    data: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    axisLine: { show: false },
    axisTick: { show: false },
    axisLabel: { color: "#666666", fontSize: 13, margin: 14 },
  },
  yAxis: {
    type: "value",
    splitLine: { lineStyle: { color: "#1f1f1f" } },
    axisLabel: { color: "#666666", fontSize: 13 },
  },
  series: [
    {
      type: "bar",
      data: [420, 561, 483, 610, 745, 380, 290],
      itemStyle: { color: "#0070f3", borderRadius: [4, 4, 0, 0] },
      barWidth: 28,
    },
  ],
});

const svg = chart.renderToSVGString();

export default function AnalyticsChart() {
  return (
    <div tw="flex h-full w-full bg-black p-10 text-white">
      <div tw="flex h-full w-full flex-col rounded-xl border border-[#1f1f1f] bg-[#0a0a0a] p-10">
        <span tw="text-sm font-medium tracking-widest text-[#888888] uppercase">Renders</span>
        <div tw="mt-2 flex items-baseline">
          <span tw="text-5xl font-semibold tracking-tight">3,489</span>
          <span tw="ml-4 text-lg text-[#666666]">past 7 days</span>
        </div>
        <img src={svg} width={1072} height={320} tw="mt-8" />
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = { width: 1200, height: 630, format: "png" };
