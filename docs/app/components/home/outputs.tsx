import type { ReactNode } from "react";
import { cn } from "~/lib/utils";
import { CARD, kilobytes, paintSteps } from "./card";
import { BODY, GUTTER, H2, VALUE } from "./typography";

const alt = `The card: “${CARD.headline}”`;

function Picture({ src, still }: { src: string; still?: string }) {
  return (
    <picture>
      {still && <source media="(prefers-reduced-motion: reduce)" srcSet={still} />}
      <img
        src={src}
        alt={alt}
        width={CARD.width}
        height={CARD.height}
        loading="lazy"
        decoding="async"
        className="h-auto w-full"
      />
    </picture>
  );
}

const outputs: { title: string; detail: string; body: ReactNode }[] = [
  {
    title: "PNG, WebP, JPEG",
    detail: `${kilobytes(CARD.image)} as WebP at 2×`,
    body: <Picture src={CARD.image} />,
  },
  {
    title: "Animated WebP, APNG, GIF",
    detail: `${CARD.frames} frames, ${kilobytes(CARD.motion)}`,
    body: <Picture src={CARD.motion} still={CARD.image} />,
  },
  { title: "SVG", detail: kilobytes(CARD.svg), body: <Picture src={CARD.svg} /> },
  {
    title: "PDF",
    detail: "takumi-pdf, A4 or any page size",
    body: (
      <div className="mx-auto aspect-[595/842] h-full bg-white p-[6%] shadow-sm">
        <Picture src={CARD.png} />
      </div>
    ),
  },
  {
    title: "Paint steps",
    detail: "takumi-paint, for your own canvas",
    body: (
      <ol className="space-y-1.5 overflow-hidden font-mono text-[0.9375rem] leading-snug">
        {paintSteps.map((step) => (
          <li key={step} className="truncate">
            {step}
          </li>
        ))}
      </ol>
    ),
  },
];

const runtimes = [
  { name: "Node.js and Bun", detail: "takumi-js, native through napi" },
  { name: "Browsers and Workers", detail: "takumi-js/wasm" },
  { name: "Rust", detail: "the takumi crate" },
];

/** What the card becomes, as a row that slides past while the page scrolls, and where it runs. */
export function Outputs() {
  return (
    <section aria-labelledby="outputs-title" className="relative py-[14svh]">
      <div className={cn("max-w-[52rem] space-y-4", GUTTER)}>
        <h2 id="outputs-title" className={H2}>
          One render, every format.
        </h2>
        <p className={BODY}>
          The same node tree becomes an image, an animation, a vector, a document, or the draw steps
          themselves.
        </p>
      </div>

      <div
        role="region"
        aria-label="Output formats"
        tabIndex={0}
        className="output-scroller mt-12 overflow-x-auto overscroll-x-contain outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
      >
        <ul className={cn("output-track flex w-max gap-6", GUTTER)}>
          {outputs.map(({ title, detail, body }) => (
            <li key={title} className="w-[clamp(18rem,34vw,32rem)] shrink-0 space-y-4">
              <div className="flex aspect-40/21 items-center overflow-hidden rounded-md border bg-muted p-[5%]">
                <div className="size-full content-center">{body}</div>
              </div>
              <div className="space-y-1">
                <h3 className="text-[1.375rem] font-[560] leading-tight">{title}</h3>
                <p className={VALUE}>{detail}</p>
              </div>
            </li>
          ))}
        </ul>
      </div>

      <dl className={cn("mt-[12svh] grid gap-x-10 gap-y-8 md:grid-cols-3", GUTTER)}>
        {runtimes.map(({ name, detail }) => (
          <div key={name} className="space-y-1">
            <dt className="text-[1.375rem] font-[560] leading-tight">{name}</dt>
            <dd className={VALUE}>{detail}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
