"use client";

import { useId, useState } from "react";
import { cn } from "~/lib/utils";
import { SvgIds, matrixCss, sceneSteps } from "./paint-svg";
import { Canvas, Choice, Panel, Readout, Stage, Surface, Toolbar } from "./parts";
import { round } from "~/playground/inspect-paint";
import type { PaintScene } from "./scene";

/** A paragraph's runs, each with its baseline, ascent, and descent drawn over it. */
export function TextRunsView({ scene }: { scene: PaintScene }) {
  const [selected, setSelected] = useState(0);
  const ids = new SvgIds(useId());
  const node = scene.nodes.find((candidate) => candidate.kind === "text");
  const run = node?.runs[selected];

  if (!node || !run) return null;

  return (
    <Surface>
      <Toolbar>
        {node.runs.map((item, index) => (
          <Choice key={index} pressed={index === selected} onClick={() => setSelected(index)}>
            {JSON.stringify(item.text)}
          </Choice>
        ))}
      </Toolbar>
      <div className="grid md:grid-cols-[minmax(0,1fr)_16rem]">
        <Stage>
          <Canvas scene={scene} label="The paragraph with the selected run's metrics">
            {sceneSteps(scene, scene.steps.length, ids)}
            <g transform={matrixCss(node.transform)}>
              {node.runs.map((item, index) => (
                <rect
                  key={index}
                  x={item.x}
                  y={item.y - item.ascent}
                  width={item.width}
                  height={item.ascent + item.descent}
                  onPointerEnter={() => setSelected(index)}
                  className={cn(
                    "cursor-pointer transition-colors duration-150",
                    index === selected
                      ? "fill-fd-primary/15 stroke-fd-primary"
                      : "fill-transparent stroke-fd-primary/25",
                  )}
                  strokeWidth={1}
                />
              ))}
              <line
                x1={run.x}
                x2={run.x + run.width}
                y1={run.y}
                y2={run.y}
                className="pointer-events-none stroke-fd-primary"
                strokeWidth={1.5}
              />
              <circle cx={run.x} cy={run.y} r={3} className="pointer-events-none fill-fd-primary" />
            </g>
          </Canvas>
        </Stage>
        <Panel>
          <Readout
            rows={[
              ["text", JSON.stringify(run.text)],
              ["element", run.tagName ? `<${run.tagName}>` : "none"],
              ["line", run.line],
              ["x, y", `${round(run.x)}, ${round(run.y)}`],
              ["width", round(run.width)],
              ["ascent", round(run.ascent)],
              ["descent", round(run.descent)],
              ["fontSize", round(run.fontSize)],
              ["lineHeight", round(run.lineHeight)],
              ["font", `${run.family ?? "unnamed"} ${run.weight}`],
            ]}
          />
        </Panel>
      </div>
    </Surface>
  );
}
