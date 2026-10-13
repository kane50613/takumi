import { cn } from "~/lib/utils";
import { CARD, scene } from "./card";
import { GUTTER, H2, LEAD, VALUE } from "./typography";
import { PaintBench } from "./use-cases";

const OUTPUT = "/images/twitter-images";
const THUMBNAIL = "/images/home";

/** The five real outputs reassembled from the anvil strike, staggered across the page. */
export function Outputs() {
  return (
    <section
      id="outputs"
      aria-labelledby="outputs-title"
      className="relative overflow-x-clip pb-[8svh]"
    >
      {/* Forge trajectory streams crossing the boundary from the anvil stage above */}
      <div aria-hidden className="pointer-events-none relative -mt-[4svh] h-[12svh] w-full">
        <svg
          viewBox="0 0 1440 220"
          preserveAspectRatio="none"
          className="size-full overflow-visible"
        >
          <defs>
            <linearGradient id="forge-stream" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#ffa944" stopOpacity="0.85" />
              <stop offset="60%" stopColor="#ff451a" stopOpacity="0.45" />
              <stop offset="100%" stopColor="#ff451a" stopOpacity="0" />
            </linearGradient>
          </defs>
          {[180, 480, 780, 1060, 1280].map((targetX, index) => (
            <path
              key={targetX}
              d={`M 920 0 C 920 95, ${targetX} 95, ${targetX} 220`}
              fill="none"
              stroke="url(#forge-stream)"
              strokeWidth={index === 2 ? "3.5" : "2"}
              strokeDasharray="10 8"
            />
          ))}
        </svg>
      </div>

      <div className={cn("max-w-[44rem] space-y-3", GUTTER)}>
        <h2 id="outputs-title" className={H2}>
          Five outputs from one strike.
        </h2>
        <p className={LEAD}>Open any rendered file or scrub the draw steps.</p>
      </div>

      {/* Staggered full-bleed workbench: Images (left) + Animations (right, offset) */}
      <div
        className={cn(
          "mt-[5svh] grid items-start gap-x-[4vw] gap-y-[7svh] lg:grid-cols-12",
          GUTTER,
        )}
      >
        {/* 1. Images */}
        <div className="space-y-4 lg:col-span-7">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h3 className="font-sans text-[1.75rem] font-[780] tracking-tight">Images</h3>
            <span className={VALUE}>PNG · WebP · JPEG</span>
          </div>

          <div className="relative pb-12 sm:pb-16">
            <a
              href={CARD.png}
              target="_blank"
              rel="noopener"
              className="group block overflow-hidden rounded-2xl border border-foreground/15 shadow-xl transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98]"
            >
              <img
                src={CARD.image}
                alt="Forged social card rendered by Takumi at 2400 by 1260 pixels"
                width={CARD.width}
                height={CARD.height}
                loading="lazy"
                decoding="async"
                className="h-auto w-full"
              />
            </a>

            <a
              href={`${OUTPUT}/home-usecase-blog-cover.png`}
              target="_blank"
              rel="noopener"
              className="group absolute -bottom-2 right-0 w-[56%] overflow-hidden rounded-xl border border-foreground/20 shadow-2xl transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98] sm:w-[48%]"
            >
              <img
                src={`${THUMBNAIL}/home-usecase-blog-cover.webp`}
                alt="Blog cover with a lighthouse at night rendered as PNG"
                width={1037}
                height={544}
                loading="lazy"
                decoding="async"
                className="h-auto w-full"
              />
            </a>
          </div>
        </div>

        {/* 2. Animations */}
        <div className="space-y-4 lg:col-span-5 lg:mt-[8svh]">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h3 className="font-sans text-[1.75rem] font-[780] tracking-tight">Animations</h3>
            <span className={VALUE}>Animated WebP · APNG · GIF</span>
          </div>

          <div className="space-y-5">
            <a
              href={`${OUTPUT}/home-usecase-now-playing.webp`}
              target="_blank"
              rel="noopener"
              className="block overflow-hidden rounded-2xl border border-foreground/15 shadow-xl transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98]"
            >
              <img
                src={`${OUTPUT}/home-usecase-now-playing.webp`}
                alt="Animated now-playing card with moving CSS keyframe equalizer bars"
                width={960}
                height={360}
                loading="lazy"
                decoding="async"
                className="h-auto w-full"
              />
            </a>

            <a
              href={CARD.motion}
              target="_blank"
              rel="noopener"
              className="ml-auto block w-[88%] overflow-hidden rounded-xl border border-foreground/15 shadow-lg transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98]"
            >
              <img
                src={CARD.motion}
                alt="Animated forge card rendered from CSS keyframes at 24 frames per second"
                width={CARD.width}
                height={CARD.height}
                loading="lazy"
                decoding="async"
                className="h-auto w-full"
              />
            </a>
          </div>
        </div>

        {/* 3. SVG (left-center) + 4. PDF (tall right spread) */}
        <div className="space-y-4 lg:col-span-6 lg:-mt-[2svh]">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h3 className="font-sans text-[1.75rem] font-[780] tracking-tight">SVG</h3>
            <span className={VALUE}>Vector paths</span>
          </div>

          <div className="space-y-5">
            <a
              href={`${OUTPUT}/home-usecase-repo-banner.svg`}
              target="_blank"
              rel="noopener"
              className="block overflow-hidden rounded-2xl border border-foreground/15 shadow-xl transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98]"
            >
              <img
                src={`${OUTPUT}/home-usecase-repo-banner.svg`}
                alt="Repository banner rendered as scalable vector SVG"
                width={1280}
                height={440}
                loading="lazy"
                decoding="async"
                className="h-auto w-full"
              />
            </a>

            <a
              href={CARD.svg}
              target="_blank"
              rel="noopener"
              className="block w-[84%] overflow-hidden rounded-xl border border-foreground/15 shadow-md transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98]"
            >
              <img
                src={CARD.svg}
                alt="Forged homepage card rendered as vector SVG"
                width={CARD.width}
                height={CARD.height}
                loading="lazy"
                decoding="async"
                className="h-auto w-full"
              />
            </a>
          </div>
        </div>

        {/* 4. PDF */}
        <div className="space-y-4 lg:col-span-6 lg:mt-[4svh]">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h3 className="font-sans text-[1.75rem] font-[780] tracking-tight">PDF</h3>
            <span className={VALUE}>Paged · selectable text</span>
          </div>

          <div className="relative flex items-start gap-5">
            <a
              href={`${OUTPUT}/home-usecase-invoice.pdf`}
              target="_blank"
              rel="noopener"
              className="block w-[46%] shrink-0 overflow-hidden rounded-xl border border-foreground/15 bg-white shadow-xl transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98]"
            >
              <img
                src={`${THUMBNAIL}/home-usecase-invoice.webp`}
                alt="A4 invoice PDF for Hollow Oak Studio with selectable text"
                width={385}
                height={544}
                loading="lazy"
                decoding="async"
                className="h-auto w-full"
              />
            </a>

            <div className="flex flex-1 flex-col gap-5 pt-6">
              <a
                href={`${OUTPUT}/home-usecase-ticket.pdf`}
                target="_blank"
                rel="noopener"
                className="block overflow-hidden rounded-xl border border-foreground/15 shadow-xl transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98]"
              >
                <img
                  src={`${THUMBNAIL}/home-usecase-ticket.webp`}
                  alt="HARBOR/26 Lisbon conference pass PDF with barcode stub"
                  width={1322}
                  height={544}
                  loading="lazy"
                  decoding="async"
                  className="h-auto w-full"
                />
              </a>

              <a
                href={`${OUTPUT}/home-usecase-certificate.pdf`}
                target="_blank"
                rel="noopener"
                className="block overflow-hidden rounded-xl border border-foreground/15 shadow-lg transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98]"
              >
                <img
                  src={`${THUMBNAIL}/home-usecase-certificate.webp`}
                  alt="Course completion certificate PDF with seal"
                  width={770}
                  height={544}
                  loading="lazy"
                  decoding="async"
                  className="h-auto w-full"
                />
              </a>
            </div>
          </div>
        </div>

        {/* 5. Paint Data Interactive Workbench */}
        <div className="space-y-4 lg:col-span-8 lg:col-start-3 lg:mt-[2svh]">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h3 className="font-sans text-[1.75rem] font-[780] tracking-tight">Paint</h3>
            <span className={VALUE}>Ordered draw steps for your own canvas</span>
          </div>

          <PaintBench scene={scene} />
        </div>
      </div>
    </section>
  );
}
