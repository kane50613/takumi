import { cn } from "~/lib/utils";
import { scene } from "./card";
import { Gallery } from "./gallery";
import { PaintBench } from "./paint-bench";
import { GUTTER, H2, H3, VALUE } from "./typography";

const OUTPUT = "/images/twitter-images";
const THUMBNAIL = "/images/home";
const PANEL = "w-[min(84vw,58rem)] shrink-0 space-y-5";

/** The outputs the anvil strike forges, slid past one by one, then its draw steps replayed. */
export function Outputs() {
  return (
    <section id="outputs" aria-labelledby="outputs-title" className="relative pb-[8svh]">
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

      <h2 id="outputs-title" className={cn(H2, GUTTER)}>
        Five outputs from one strike.
      </h2>

      <Gallery>
        <section aria-label="Images" className={PANEL}>
          <PanelTitle title="Images" formats="PNG · WebP · JPEG" />
          <div className="relative pb-12">
            <OutputImage
              href={`${OUTPUT}/home-usecase-blog-cover.png`}
              src={`${THUMBNAIL}/home-usecase-blog-cover.webp`}
              alt="Blog cover with a lighthouse at night rendered as PNG"
              width={1037}
              height={544}
              className="w-[82%]"
            />
            <OutputImage
              href={`${OUTPUT}/home-usecase-quote-card.webp`}
              src={`${THUMBNAIL}/home-usecase-quote-card.webp`}
              alt="Square quote card rendered as WebP"
              width={544}
              height={544}
              className="absolute -bottom-2 right-0 w-[34%] shadow-2xl"
            />
          </div>
        </section>

        <section aria-label="Animations" className={PANEL}>
          <PanelTitle title="Animations" formats="Animated WebP · APNG · GIF" />
          <OutputImage
            href={`${OUTPUT}/home-usecase-now-playing.webp`}
            src={`${OUTPUT}/home-usecase-now-playing.webp`}
            alt="Animated now-playing card with moving CSS keyframe equalizer bars"
            width={960}
            height={360}
          />
        </section>

        <section aria-label="SVG" className={PANEL}>
          <PanelTitle title="SVG" />
          <OutputImage
            href={`${OUTPUT}/home-usecase-repo-banner.svg`}
            src={`${OUTPUT}/home-usecase-repo-banner.svg`}
            alt="Repository banner rendered as scalable vector SVG"
            width={1280}
            height={440}
          />
        </section>

        <section aria-label="PDF" className={PANEL}>
          <PanelTitle title="PDF" />
          <div className="flex items-start gap-5">
            <OutputImage
              href={`${OUTPUT}/home-usecase-invoice.pdf`}
              src={`${THUMBNAIL}/home-usecase-invoice.webp`}
              alt="A4 invoice PDF for Hollow Oak Studio with selectable text"
              width={385}
              height={544}
              className="w-[40%] shrink-0 bg-white"
            />
            <div className="flex flex-1 flex-col gap-5 pt-6">
              <OutputImage
                href={`${OUTPUT}/home-usecase-ticket.pdf`}
                src={`${THUMBNAIL}/home-usecase-ticket.webp`}
                alt="HARBOR/26 Lisbon conference pass PDF with barcode stub"
                width={1322}
                height={544}
              />
              <OutputImage
                href={`${OUTPUT}/home-usecase-certificate.pdf`}
                src={`${THUMBNAIL}/home-usecase-certificate.webp`}
                alt="Course completion certificate PDF with seal"
                width={770}
                height={544}
              />
            </div>
          </div>
        </section>
      </Gallery>

      <section aria-label="Paint" className={cn("mx-auto max-w-[64rem] space-y-5", GUTTER)}>
        <h3 className={H3}>Paint</h3>
        <PaintBench scene={scene} />
      </section>
    </section>
  );
}

function PanelTitle({ title, formats }: { title: string; formats?: string }) {
  return (
    <div className="flex flex-wrap items-baseline justify-between gap-2">
      <h3 className={H3}>{title}</h3>
      {formats && <span className={VALUE}>{formats}</span>}
    </div>
  );
}

/** A rendered file's preview that opens the file itself. */
function OutputImage({
  href,
  src,
  alt,
  width,
  height,
  className,
}: {
  href: string;
  src: string;
  alt: string;
  width: number;
  height: number;
  className?: string;
}) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener"
      className={cn(
        "block overflow-hidden rounded-2xl border border-foreground/15 shadow-xl transition-transform duration-200 ease-out-strong hover:-translate-y-1 active:scale-[0.98]",
        className,
      )}
    >
      <img
        src={src}
        alt={alt}
        width={width}
        height={height}
        loading="lazy"
        decoding="async"
        className="h-auto w-full"
      />
    </a>
  );
}
