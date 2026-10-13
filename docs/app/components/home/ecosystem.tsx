import type { ReactNode } from "react";
import { Link } from "waku";
import { SvgIds, sceneSteps } from "~/components/mdx/paint/paint-svg";
import { Button } from "~/components/ui/button";
import { cn } from "~/lib/utils";
import { boxes, CARD, kilobytes, layers, scene, source } from "./card";
import { EcosystemCanvas } from "./ecosystem-canvas";
import { H1, H2, LINE, LINK, VALUE } from "./typography";
import type { EcosystemData } from "./ecosystem-data";

const data: EcosystemData = {
  card: { image: CARD.image, motion: CARD.motion, svg: CARD.svg },
  source,
  boxes,
  steps: layers.map(({ label }) => label),
  outputs: [
    { title: "PNG · WebP · JPEG", detail: `${kilobytes(CARD.image)} at 2x` },
    { title: "WebP · APNG · GIF", detail: `${CARD.frames} frames · ${kilobytes(CARD.motion)}` },
    { title: "SVG", detail: kilobytes(CARD.svg) },
    { title: "PDF", detail: "takumi-pdf" },
    { title: "takumi-paint", detail: `${layers.length} draw steps` },
  ],
  runtimes: [
    { name: "Node.js · Bun", detail: "napi" },
    { name: "Browsers · Workers", detail: "wasm" },
    { name: "Rust", detail: "crate" },
  ],
};

const ids = new SvgIds("layer");

/** One viewport of scroll. Its text sits left of the card on wide screens and under it on narrow ones. */
function Chapter({
  children,
  backdrop,
  className,
}: {
  children: ReactNode;
  backdrop?: ReactNode;
  className?: string;
}) {
  return (
    <section data-chapter className={cn("relative h-[calc(100svh-3.5rem)]", className)}>
      {backdrop}
      <div className="absolute inset-x-6 bottom-[8svh] z-10 space-y-4 lg:inset-x-auto lg:bottom-auto lg:left-[max(6vw,2rem)] lg:top-1/2 lg:w-[26rem] lg:-translate-y-1/2">
        {children}
      </div>
    </section>
  );
}

/**
 * The card's outline where the scene first draws it, so the hero has its shape before three.js
 * loads. Mirrors `cardFrame` in ecosystem-data.ts.
 */
function Poster() {
  return (
    <svg
      aria-hidden
      viewBox={`0 0 ${CARD.width} ${CARD.height}`}
      className="ecosystem-poster pointer-events-none absolute left-[7cqw] top-[calc(32cqh-86cqw*0.2625)] w-[86cqw] overflow-visible text-foreground opacity-90 transition-opacity duration-500 ease-out lg:left-[calc(70cqw-min(46cqw,720px)/2)] lg:top-[calc(50cqh-min(46cqw,720px)*0.2625)] lg:w-[min(46cqw,720px)]"
    >
      <rect
        width={CARD.width}
        height={CARD.height}
        fill="none"
        stroke="currentColor"
        strokeWidth={1}
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

/** The homepage: one card, followed through Takumi's whole ecosystem as the reader scrolls. */
export function Ecosystem() {
  return (
    <div data-ecosystem className="relative">
      <EcosystemCanvas data={data} />
      <div hidden data-layer-svg>
        {layers.map(({ step }) => {
          const draw = scene.steps[step];

          return (
            <svg key={step} viewBox={`0 0 ${CARD.width} ${CARD.height}`}>
              {draw && sceneSteps({ ...scene, steps: [draw] }, 1, ids)}
            </svg>
          );
        })}
      </div>

      <Chapter className="[container-type:size]" backdrop={<Poster />}>
        <h1 className={H1}>JSX to images. No browser.</h1>
        <p className={LINE}>
          Takumi renders OG images, social cards, and PDFs from JSX and CSS, in Rust.
        </p>
        <p className="font-mono text-sm select-all">bun i takumi-js</p>
        <div className="flex items-center gap-6 pt-2">
          <Button asChild size="lg" className="px-7 font-semibold active:scale-[0.97]">
            <Link to="/docs">Get started</Link>
          </Button>
          <Link to="/playground" className={cn(LINK, "text-sm")}>
            Playground
          </Link>
        </div>
      </Chapter>

      <Chapter>
        <h2 className={H2}>Write a component.</h2>
        <p className={LINE}>This card is plain JSX with Tailwind classes.</p>
        <p className={VALUE}>home-demo-card.tsx · {source.length} lines</p>
      </Chapter>

      <Chapter>
        <h2 className={H2}>Lay it out.</h2>
        <p className={LINE}>
          Taffy gives every box a position and a size before anything is drawn.
        </p>
        <p className={VALUE}>{boxes.length} boxes · flexbox</p>
      </Chapter>

      <Chapter>
        <h2 className={H2}>Paint it.</h2>
        <p className={LINE}>
          Each box turns into draw steps with every CSS value resolved. takumi-paint hands you that
          list.
        </p>
        <p className={VALUE}>
          {layers.length} steps ·{" "}
          <Link to="/docs/paint" className={LINK}>
            npm i takumi-paint
          </Link>
        </p>
      </Chapter>

      <Chapter>
        <h2 className={H2}>Fill the pixels.</h2>
        <p className={LINE}>The rasterizer draws the steps into an image.</p>
        <p className={VALUE}>
          {CARD.width * 2} × {CARD.height * 2} · WebP · {kilobytes(CARD.image)}
        </p>
      </Chapter>

      <Chapter>
        <h2 className={H2}>Pick an output.</h2>
        <p className={LINE}>
          The same tree becomes an image, an animation, an SVG, a PDF, or paint data.
        </p>
      </Chapter>

      <Chapter>
        <h2 className={H2}>Run it anywhere.</h2>
        <p className={LINE}>
          Native on Node.js and Bun. WebAssembly in browsers and Workers. A crate for Rust.
        </p>
      </Chapter>
    </div>
  );
}
