import { Link } from "waku";
import { drawableElement, matrixCss, sceneSteps, SvgIds } from "~/components/mdx/paint/paint-svg";
import type { SceneStep } from "~/components/mdx/paint/scene";
import { Button } from "~/components/ui/button";
import { cn } from "~/lib/utils";
import { boxes, CARD, glyphRuns, scene } from "./card";
import { StageController } from "./stage-controller";
import { BACKDROP_WORD, DISPLAY, GUTTER, LINK, PRIMARY_BUTTON } from "./typography";

const STAGES = [
  {
    key: "anvil",
    stepLabel: "Anvil",
    word: "Craft",
    title: "Craft components into every output.",
  },
  {
    key: "layout",
    stepLabel: "Layout",
    word: "Layout",
    title: "Flexbox and grid, measured.",
  },
  {
    key: "text",
    stepLabel: "Text",
    word: "Text",
    title: "Every glyph, shaped.",
  },
  {
    key: "paint",
    stepLabel: "Paint",
    word: "Paint",
    title: "Every fill, drawn in order.",
  },
] as const;

const EMBER = "#ff5d2e";
const INK = "#181310";

const ANVIL_PATH =
  "M 430 672 Q 535 652 608 652 L 1232 652 L 1278 670 L 1232 688 L 1110 696 C 1055 724 1055 768 1120 792 L 1220 806 L 1238 844 L 595 844 L 615 806 L 715 792 C 780 768 780 724 725 696 L 608 690 Q 520 686 430 672 Z";

const HAMMER_HANDLE_PATH =
  "M114.3 14.1c1.1.9 3.2 2.7 4.2 4.5s.9 3.5.8 4.8-.4 2.3-2 4.3c-1.7 2-4.7 5-12.8 13.7-8.1 8.8-21.4 23.4-35.3 38.4s-28.6 30.5-36.4 38.7-8.8 8.8-10 9.2-2.5.4-4.1 0-3.5-1.2-6.5-3.8-7.1-6.9-9.4-10S.1 108.8.1 107c0-1.7.4-3.4 5.3-8.6s14.3-13.9 30.5-28.9c16.1-14.9 39-36.1 51-47.2s13.2-12 14.7-12.7 3.2-1.1 4.8-.9 2.8 1 3.9 1.9a32 32 0 0 1 2.5 2.1l.4.5z";

const HAMMER_HEAD_PATH =
  "M79 .5C65.3 3.1 46.9 23.4 56.8 36.3c3.3 4.3 5.1 6.7 9.3 9.7 10.2 7.3 39.1 31 53.1 26.9 12-3.5 9.4-16.9 5.6-25.8-1.3-3-25.7-52.8-45.8-46.6";

type DrawStep = Extract<SceneStep, { type: "draw" }>;

type PaintPlane = {
  label: string;
  steps: DrawStep[];
  dx: number;
  dy: number;
};

/** Groups the scene's real draw steps into 5 semantic layers so every plane shows a distinct stage. */
function buildPaintPlanes(): PaintPlane[] {
  const drawSteps = scene.steps.filter((step): step is DrawStep => step.type === "draw");
  const canvasAndFrame: DrawStep[] = [];
  const rustStamp: DrawStep[] = [];
  const headerAndSpecs: DrawStep[] = [];
  const equalizerBars: DrawStep[] = [];
  const headlineGlyphs: DrawStep[] = [];

  for (const step of drawSteps) {
    const node = scene.nodes[step.node];

    if (!node) {
      continue;
    }
    const maxFontSize = node.runs.reduce((max, run) => Math.max(max, run.fontSize), 0);

    if (node.depth <= 1) {
      canvasAndFrame.push(step);
    } else if (node.kind === "box" && node.bounds.x >= 850 && node.bounds.width >= 180) {
      rustStamp.push(step);
    } else if (node.kind === "box" && node.bounds.width <= 20) {
      equalizerBars.push(step);
    } else if (maxFontSize >= 32 || step.drawables.some((d) => d.type === "masked")) {
      headlineGlyphs.push(step);
    } else {
      headerAndSpecs.push(step);
    }
  }

  return [
    { label: "01 · canvas & frame", steps: canvasAndFrame, dx: -175, dy: 144 },
    { label: "02 · rust gradient", steps: rustStamp, dx: -70, dy: 92 },
    { label: "03 · header & specs", steps: headerAndSpecs, dx: 35, dy: 40 },
    { label: "04 · equalizer bars", steps: equalizerBars, dx: 140, dy: -12 },
    { label: "05 · headline glyphs", steps: headlineGlyphs, dx: 245, dy: -64 },
  ];
}

/**
 * The homepage's full-bleed Hammer & Anvil forge: scrolling or clicking Strike swings Takumi's
 * orange-red hammer onto the component on the anvil, splitting it into real layout boxes,
 * shaped glyph outlines, and ordered draw steps.
 */
export function Stage() {
  const intactIds = new SvgIds("forge-intact");
  const paintPlanes = buildPaintPlanes();

  return (
    <section
      data-stage
      aria-labelledby="home-title"
      className="relative h-[420svh] select-none motion-reduce:h-[200svh]"
    >
      <div
        data-stage-view
        className="sticky top-14 flex h-[calc(100svh-3.5rem)] w-full flex-col justify-between overflow-hidden"
      >
        <StageController
          data={{ image: CARD.image, width: CARD.width, height: CARD.height, boxes }}
        />

        {/* Hollow backdrop word for the current strike, widening as it rises out of its mask */}
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 grid place-items-center overflow-hidden"
        >
          {STAGES.map((item, index) => (
            <div
              key={item.key}
              data-stage-copy={index}
              data-current={index === 0 ? "" : undefined}
              className="col-start-1 row-start-1 overflow-hidden"
            >
              <span
                className={cn(
                  BACKDROP_WORD,
                  "home-rise block translate-y-[calc(100%+0.2em)] [font-stretch:70%] transition-[transform,font-stretch] duration-1000 ease-out-strong in-data-current:translate-y-0 in-data-current:[font-stretch:125%]",
                )}
              >
                {item.word}
              </span>
            </div>
          ))}
        </div>

        <div className={cn("relative z-10 grid pt-[3svh] lg:pt-[5svh]", GUTTER)}>
          {STAGES.map((item, index) => {
            const HeadingTag = index === 0 ? "h1" : "h2";

            return (
              <HeadingTag
                key={item.key}
                id={index === 0 ? "home-title" : undefined}
                data-stage-copy={index}
                data-current={index === 0 ? "" : undefined}
                className={cn(
                  DISPLAY,
                  "home-enter col-start-1 row-start-1 max-w-[17ch] transition-[opacity,transform,filter] duration-500 ease-out-strong not-data-current:translate-y-3 not-data-current:opacity-0 not-data-current:blur-sm",
                )}
              >
                {item.title}
              </HeadingTag>
            );
          })}
        </div>

        {/* Full-bleed SVG Forge Canvas: Hammer, Anvil, Component, and Exploded Real Parts */}
        <div className="home-enter pointer-events-none relative min-h-0 flex-1 [--enter-delay:120ms] max-lg:order-1 lg:absolute lg:inset-0 lg:bottom-20">
          <svg
            data-forge
            role="img"
            aria-label="Takumi's orange-red hammer suspended over an anvil where a component splits into layout boxes, glyph outlines, and paint layers."
            viewBox="200 -40 1440 940"
            preserveAspectRatio="xMidYMid meet"
            className="size-full overflow-visible text-foreground"
          >
            <defs>
              <linearGradient
                id="forge-hammer-handle"
                gradientUnits="userSpaceOnUse"
                x1="59.7"
                y1="8.6"
                x2="59.7"
                y2="128"
              >
                <stop offset="0" stopColor="#ffa944" />
                <stop offset="1" stopColor="#ff3300" />
              </linearGradient>
              <linearGradient
                id="forge-hammer-head"
                gradientUnits="userSpaceOnUse"
                x1="90.7"
                y1="0"
                x2="90.7"
                y2="73.7"
              >
                <stop offset="0" stopColor="#ff3535" />
                <stop offset="1" stopColor="#d71d36" />
              </linearGradient>
              <filter id="forge-mute">
                <feColorMatrix type="saturate" values="0.12" />
              </filter>
              {/* A soft drop shadow lifting hammer and anvil off light and dark pages alike */}
              <filter id="forge-shadow" x="-10%" y="-10%" width="120%" height="130%">
                <feDropShadow
                  dx="0"
                  dy="8"
                  stdDeviation="10"
                  floodColor={INK}
                  floodOpacity="0.22"
                />
              </filter>
            </defs>

            {/* Heavy Cast-Iron Anvil anchored at x=920, face at y=652 */}
            <g data-anvil className="will-change-transform" filter="url(#forge-shadow)">
              <path d={ANVIL_PATH} className="fill-[var(--anvil)]" />
            </g>

            {/* The Component on the Anvil (1200×630 scaled by 0.516 -> 619×325 at x=610..1229, y=323..648) */}
            <g data-card-bounds transform="translate(610, 323) scale(0.516)">
              {/* 0. Intact component before the first strike */}
              <g
                data-anvil-card
                className="will-change-[transform,opacity]"
                style={{ transformOrigin: "600px 630px" }}
              >
                {sceneSteps(scene, scene.steps.length, intactIds)}
              </g>

              {/* 1. Strike I — Real Layout Boxes from takumi-paint */}
              <g
                data-layer="layout"
                className="will-change-opacity"
                style={{ opacity: 0, visibility: "hidden" }}
              >
                <rect
                  x="0"
                  y="0"
                  width={CARD.width}
                  height={CARD.height}
                  rx="16"
                  fill="none"
                  stroke="currentColor"
                  strokeOpacity="0.5"
                  strokeWidth="2"
                  strokeDasharray="10 8"
                />
                {boxes.map((box) => {
                  const isAccent = box.depth <= 2 || box.width >= 240;
                  const showLabel = box.width >= 180 && box.height >= 48;

                  return (
                    <g
                      key={box.index}
                      data-box
                      data-dx={box.dx}
                      data-dy={box.dy}
                      data-rot={box.rotate}
                      className="will-change-transform"
                      style={{
                        transformOrigin: `${box.x + box.width / 2}px ${box.y + box.height / 2}px`,
                      }}
                    >
                      <rect
                        x={box.x}
                        y={box.y}
                        width={box.width}
                        height={box.height}
                        rx={box.depth <= 2 ? 18 : 6}
                        fill="none"
                        stroke="currentColor"
                        strokeOpacity={isAccent ? 0.9 : 0.45}
                        strokeWidth={isAccent ? 3 : 2}
                      />
                      {showLabel && (
                        <g transform={`translate(${box.x + 18}, ${box.y + 38})`}>
                          <text fill="currentColor" className="font-label text-[30px]">
                            {Math.round(box.x)},{Math.round(box.y)} · {Math.round(box.width)}×
                            {Math.round(box.height)}
                          </text>
                        </g>
                      )}
                    </g>
                  );
                })}
              </g>

              {/* 2. Strike II — Real Shaped Glyph Outlines & Metrics from takumi-paint */}
              <g
                data-layer="text"
                className="will-change-opacity"
                style={{ opacity: 0, visibility: "hidden" }}
              >
                {glyphRuns.map((run) => (
                  <g
                    key={run.id}
                    data-glyph-run
                    data-dx={run.dx}
                    data-dy={run.dy}
                    data-scale={run.scale}
                    className="will-change-transform"
                    style={{
                      transformOrigin: `${run.x + run.width / 2}px ${run.y - run.ascent / 2}px`,
                    }}
                  >
                    <rect
                      x={run.x - 10}
                      y={run.y - run.ascent - 8}
                      width={run.width + 20}
                      height={run.ascent + run.descent + 16}
                      rx="8"
                      fill="none"
                      stroke="currentColor"
                      strokeOpacity="0.35"
                      strokeWidth="2"
                    />
                    <line
                      x1={run.x - 18}
                      x2={run.x + run.width + 18}
                      y1={run.y}
                      y2={run.y}
                      stroke={EMBER}
                      strokeWidth="3.5"
                    />
                    <circle cx={run.x} cy={run.y} r="6.5" fill={EMBER} />
                    <path
                      d={run.outline}
                      transform={`translate(${run.x}, ${run.y})`}
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.5"
                    />
                    <text
                      x={run.x}
                      y={run.y + run.descent + 34}
                      fill="currentColor"
                      className="font-label text-[31px]"
                    >
                      {run.fontSize}px · w {Math.round(run.width)}
                    </text>
                  </g>
                ))}
              </g>

              {/* 3. Strike III — Real Ordered Paint Draw Steps from takumi-paint */}
              <g
                data-layer="paint"
                className="will-change-opacity"
                style={{ opacity: 0, visibility: "hidden" }}
              >
                {paintPlanes.map((plane, planeIndex) => {
                  const planeIds = new SvgIds(`forge-plane-${planeIndex}`);

                  return (
                    <g
                      key={plane.label}
                      data-paint-plane
                      data-dx={plane.dx}
                      data-dy={plane.dy}
                      className="will-change-transform"
                      style={{ transformOrigin: "600px 315px" }}
                    >
                      <rect
                        x="0"
                        y="0"
                        width={CARD.width}
                        height={CARD.height}
                        rx="22"
                        fill="none"
                        stroke="currentColor"
                        strokeOpacity="0.5"
                        strokeWidth="2"
                      />
                      <g filter="url(#forge-mute)" opacity="0.7">
                        {plane.steps.map((step, sIdx) => {
                          const node = scene.nodes[step.node];

                          return (
                            <g key={sIdx} transform={node ? matrixCss(node.transform) : undefined}>
                              {step.drawables.map((drawable, dIdx) => (
                                <g key={dIdx}>{drawableElement(drawable, planeIds)}</g>
                              ))}
                            </g>
                          );
                        })}
                      </g>
                      <g transform="translate(20, -16)">
                        <text x="0" y="-4" fill="currentColor" className="font-label text-[30px]">
                          {plane.label}
                        </text>
                      </g>
                    </g>
                  );
                })}
              </g>
            </g>

            {/* Impact Shockwave Burst at (920, 323) where the Hammer meets the Component */}
            <g
              data-impact-burst
              className="pointer-events-none will-change-[transform,opacity]"
              style={{ opacity: 0, transformOrigin: "920px 323px" }}
            >
              <ellipse
                cx="920"
                cy="323"
                rx="145"
                ry="38"
                fill="none"
                stroke={EMBER}
                strokeWidth="4"
              />
              {[
                [-170, -85],
                [-120, -135],
                [0, -150],
                [125, -130],
                [175, -75],
                [-195, -15],
                [195, -15],
              ].map(([rx, ry], idx) => (
                <line
                  key={idx}
                  x1={920 + (rx ?? 0) * 0.35}
                  y1={323 + (ry ?? 0) * 0.35}
                  x2={920 + (rx ?? 0)}
                  y2={323 + (ry ?? 0)}
                  stroke={EMBER}
                  strokeWidth="5"
                  strokeLinecap="round"
                />
              ))}
            </g>

            {/* The Giant Takumi Brand Hammer (pivoting from (1315, 165) onto (920, 323)) */}
            <g
              className="home-swing"
              style={{ transformOrigin: "1315px 165px" }}
              filter="url(#forge-shadow)"
            >
              <g
                data-hammer
                className="will-change-transform"
                style={{ transformOrigin: "1315px 165px", transform: "rotate(11deg)" }}
              >
                <g transform="translate(1315, 165) rotate(-47.9) scale(-3.3, 3.3) translate(-8, -110)">
                  <path fill="url(#forge-hammer-handle)" d={HAMMER_HANDLE_PATH} />
                  <path fill="url(#forge-hammer-head)" d={HAMMER_HEAD_PATH} />
                </g>
              </g>
            </g>
          </svg>
        </div>

        {/* Bottom Stage Rail: Keyboard-accessible buttons for each strike */}
        <div
          className={cn(
            "relative z-10 mt-auto flex flex-wrap items-center justify-between gap-4 pb-[2.5svh] max-lg:order-2",
            GUTTER,
          )}
        >
          <nav aria-label="Forge stages" className="flex flex-wrap items-center gap-2 sm:gap-3">
            {STAGES.map((item, index) => (
              <button
                key={item.key}
                type="button"
                data-step-button
                data-step-index={index}
                aria-current={index === 0 ? "step" : undefined}
                className="raised raised-quiet cursor-pointer rounded-lg bg-background px-3.5 py-1.5 font-label text-base font-medium text-muted-foreground transition-[color,background-color,box-shadow,transform] duration-150 ease-out-strong [font-stretch:87.5%] hover:text-foreground active:scale-[0.97] aria-[current=step]:bg-muted aria-[current=step]:text-foreground sm:px-4 sm:py-2"
              >
                {item.stepLabel}
              </button>
            ))}
          </nav>

          <div className="home-enter flex flex-wrap items-center gap-x-5 gap-y-3 [--enter-delay:240ms]">
            <Button
              type="button"
              size="lg"
              data-strike-trigger
              className={cn(PRIMARY_BUTTON, "cursor-pointer gap-3")}
            >
              <img
                src="/logo.svg"
                alt=""
                aria-hidden
                width={22}
                height={22}
                className="size-5 brightness-200"
              />
              <span data-strike-label>Strike the anvil</span>
            </Button>
            <Link to="/docs" className={cn(LINK, "text-lg")}>
              Get started
            </Link>
            <Link to="/playground" className={cn(LINK, "text-lg max-sm:hidden")}>
              Playground
            </Link>
          </div>
        </div>
      </div>
    </section>
  );
}
