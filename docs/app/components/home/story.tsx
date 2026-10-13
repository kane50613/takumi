import { statSync } from "node:fs";
import { join } from "node:path";
import type { ReactNode } from "react";
import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { cn } from "~/lib/utils";
import { animation } from "../../../../example/twitter-images/components/home-demo-card-motion";
import { boxes, CARD, layers } from "./card";
import { Code, GRID, H1, H2, LEAD, LINK, Specs, TEXT_COLUMN, VISUAL_COLUMN } from "./section";
import type { ThemedHtml } from "./shiki-html";
import { layerStyles, Stage, type StateId } from "./stage";
import { StoryController } from "./story-controller";

function kilobytes(path: string) {
  return `${Math.round(statSync(join("public", path)).size / 1024)} KB`;
}

const FRAMES = (animation.durationMs * animation.fps) / 1000;

function Step({ id, children }: { id: StateId; children: ReactNode }) {
  return (
    <div
      id={id}
      data-step={id}
      className={cn(
        TEXT_COLUMN,
        "scroll-mt-80 py-16 space-y-6 lg:min-h-[85vh] lg:flex lg:flex-col lg:justify-center lg:scroll-mt-0",
      )}
    >
      {children}
    </div>
  );
}

/** The homepage story: one card, followed from JSX to pixels, beside a stage that stays in view. */
export function Story({
  cardSource,
  keyframes,
}: {
  cardSource: ThemedHtml;
  keyframes: ThemedHtml;
}) {
  return (
    <section data-story className="story px-6">
      <style>{layerStyles()}</style>
      <div className={GRID}>
        <header
          data-step="jsx"
          className={cn(
            TEXT_COLUMN,
            "pt-16 pb-12 lg:row-start-1 lg:min-h-[calc(100vh-3.5rem)] lg:flex lg:flex-col lg:justify-center lg:py-0",
          )}
        >
          <h1 className={cn(H1, "mb-6")}>JSX to images and PDFs, without a browser.</h1>
          <p className={cn(LEAD, "mb-8 max-w-110")}>
            Create OG images, social cards, and PDFs from JSX or HTML, styled with CSS and Tailwind.
          </p>
          <div className="flex items-center gap-6 flex-wrap">
            <Button asChild size="lg" className="px-7 font-semibold active:scale-[0.97]">
              <Link to="/docs">Get started</Link>
            </Button>
            <Link to="/playground" className={cn(LINK, "text-sm")}>
              Open the playground
            </Link>
          </div>
        </header>

        <div
          className={cn(
            VISUAL_COLUMN,
            "sticky top-14 z-10 -mx-6 bg-background px-6 pt-4 pb-6 lg:row-span-6 lg:row-start-1 lg:mx-0 lg:h-[calc(100vh-3.5rem)] lg:flex lg:items-center lg:bg-transparent lg:p-0",
          )}
        >
          <Stage />
        </div>

        <Step id="jsx">
          <h2 className={H2}>Write the card as JSX.</h2>
          <p className={LEAD}>
            Use flexbox, gradients, web fonts, and Tailwind classes. Takumi reads the component as
            it is.
          </p>
          <Code html={cardSource} />
          <Specs
            rows={[
              {
                label: "Text",
                values: ["WOFF2 fonts", "emoji", "RTL scripts", "multi-span inline"],
              },
              {
                label: "Selectors",
                values: [":is()", ":where()", "::before", "::after"],
                mono: true,
              },
            ]}
          />
        </Step>

        <Step id="layout">
          <h2 className={H2}>Layout gives every box a place.</h2>
          <p className={LEAD}>
            Taffy runs flexbox, grid, and block layout in Rust. This card has {boxes.length} boxes,
            each measured before anything is drawn.
          </p>
          <Specs
            rows={[
              {
                label: "Layout",
                values: ["display: grid", "float", "position: absolute", "calc()", "z-index"],
                mono: true,
              },
            ]}
          />
        </Step>

        <Step id="paint">
          <h2 className={H2}>Paint turns boxes into draw steps.</h2>
          <p className={LEAD}>
            Each step holds resolved fills, strokes, and glyph runs, in order.{" "}
            <code className="font-mono text-[0.9em] text-foreground">takumi-paint</code> returns
            them, so you can draw them on Canvas or write them into slides.
          </p>
          <ol className="space-y-3 font-mono text-xs">
            {layers.map((layer, index) => (
              <li key={layer.step} data-row={index} className="cursor-default">
                <div className="flex items-center gap-2">
                  <span className="text-muted-foreground tabular-nums">steps[{layer.order}]</span>
                  <span className="truncate">{layer.name}</span>
                </div>
                <div className="mt-1 flex items-center gap-2 text-muted-foreground">
                  {layer.swatch && (
                    <span
                      aria-hidden
                      className="size-2.5 shrink-0 rounded-[2px] ring-1 ring-border"
                      style={{ background: layer.swatch }}
                    />
                  )}
                  <span className="truncate">
                    {layer.label} · {layer.value}
                  </span>
                </div>
              </li>
            ))}
          </ol>
          <div className="flex items-center gap-6 text-sm">
            <code className="font-mono text-[0.8125rem] select-all">npm i takumi-paint</code>
            <Link to="/docs/paint" className={LINK}>
              Read the paint docs
            </Link>
          </div>
        </Step>

        <Step id="pixels">
          <h2 className={H2}>Then the pixels.</h2>
          <p className={LEAD}>
            The rasterizer draws the steps and encodes the image. At 2x this card is{" "}
            {CARD.width * 2} × {CARD.height * 2} pixels and {kilobytes(CARD.image)} as WebP. Point
            at it, or tap it, to lift the pixels.
          </p>
          <Specs
            rows={[
              { label: "Images", values: ["PNG", "WebP", "JPEG", "ICO"] },
              { label: "Vectors", values: ["SVG", "PDF"] },
            ]}
          />
        </Step>

        <Step id="motion">
          <h2 className={H2}>Add keyframes for animation.</h2>
          <p className={LEAD}>
            Give the square a CSS animation, and Takumi renders the same tree frame by frame. This
            loop is {FRAMES} frames and {kilobytes(CARD.motion)} as animated WebP.
          </p>
          <Code html={keyframes} />
          <Specs
            rows={[
              { label: "Motion", values: ["@keyframes", "animation", "animate-*"], mono: true },
              { label: "Formats", values: ["WebP", "APNG", "GIF"] },
            ]}
          />
        </Step>
      </div>
      <StoryController />
    </section>
  );
}
