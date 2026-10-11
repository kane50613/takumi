"use client";

import { useId, useState } from "react";
import type { Matrix, Rect } from "takumi-paint";
import { SvgIds, drawableElement, matrixCss, sceneSteps } from "./paint-svg";
import { Canvas, Choice, Panel, Readout, Stage, Surface, Toolbar } from "./parts";
import { round } from "~/playground/inspect-paint";
import type { PaintScene, SceneDrawable } from "./scene";

export type LocalSpaceFrame = {
  degrees: number;
  transform: Matrix;
  width: number;
  height: number;
  bounds: Rect;
  drawables: SceneDrawable[];
};

function Arrow({ x, y, label }: { x: number; y: number; label: string }) {
  const length = Math.hypot(x, y);
  const [ux, uy] = [x / length, y / length];

  return (
    <g className="stroke-neutral-900">
      <line x1={0} y1={0} x2={x} y2={y} strokeWidth={1.5} />
      <path
        d={`M${x},${y} L${x - ux * 7 - uy * 4},${y - uy * 7 + ux * 4} L${x - ux * 7 + uy * 4},${y - uy * 7 - ux * 4} Z`}
        className="fill-neutral-900"
      />
      <text
        x={x + ux * 10}
        y={y + uy * 10 + 4}
        className="fill-neutral-900 stroke-none font-mono text-[12px]"
      >
        {label}
      </text>
    </g>
  );
}

/** A node's drawables in its local space, moved onto the canvas by `node.transform`. */
export function LocalSpaceView({
  backdrop,
  frames,
}: {
  backdrop: PaintScene;
  frames: LocalSpaceFrame[];
}) {
  const [selected, setSelected] = useState(frames.length - 1);
  const ids = new SvgIds(useId());
  const frame = frames[selected] ?? frames[0];

  if (!frame) return null;

  const { bounds } = frame;

  return (
    <Surface>
      <Toolbar>
        {frames.map((item, index) => (
          <Choice
            key={item.degrees}
            pressed={index === selected}
            onClick={() => setSelected(index)}
          >
            rotate({item.degrees}deg)
          </Choice>
        ))}
      </Toolbar>
      <div className="grid md:grid-cols-[minmax(0,1fr)_16rem]">
        <Stage>
          <Canvas scene={backdrop} label="The badge's local space placed on the canvas">
            {sceneSteps(backdrop, backdrop.steps.length, ids)}
            <rect {...bounds} className="fill-none stroke-neutral-400" strokeDasharray="4 3" />
            <g
              className="transition-transform duration-300 ease-[cubic-bezier(0.77,0,0.175,1)] motion-reduce:transition-none"
              style={{ transform: matrixCss(frame.transform) }}
            >
              {frame.drawables.map((drawable, index) => (
                <g key={index}>{drawableElement(drawable, ids)}</g>
              ))}
              <Arrow x={frame.width + 16} y={0} label="x" />
              <Arrow x={0} y={frame.height + 16} label="y" />
              <circle r={3} className="fill-neutral-900" />
            </g>
          </Canvas>
        </Stage>
        <Panel>
          <Readout
            rows={[
              ["width", round(frame.width)],
              ["height", round(frame.height)],
              ["transform", `[${frame.transform.map(round).join(", ")}]`],
              [
                "bounds",
                `${round(bounds.x)}, ${round(bounds.y)}, ${round(bounds.width)} × ${round(bounds.height)}`,
              ],
            ]}
          />
        </Panel>
      </div>
    </Surface>
  );
}
