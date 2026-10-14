import { cn } from "~/lib/utils";
import { scene } from "./card";
import { Gallery } from "./gallery";
import { PaintBench } from "./paint-bench";
import { GUTTER, H2, H3, VALUE } from "./typography";

const OUTPUT = "/images/twitter-images";
const THUMBNAIL = "/images/home";
const PANEL = "grid w-full max-w-[76rem] items-center gap-x-12 gap-y-6 lg:grid-cols-[15rem_1fr]";

/** The outputs the anvil strike forges, wiped in one by one, then its draw steps replayed. */
export function Outputs() {
  return (
    <section id="outputs" aria-labelledby="outputs-title" className="relative pt-[12svh] pb-[8svh]">
      <h2 id="outputs-title" className={cn(H2, GUTTER)}>
        One component, every format.
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
              className="absolute -bottom-2 right-0 w-[34%] ring-8 ring-background"
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

      <section
        aria-label="Paint"
        className={cn("mx-auto max-w-[64rem] space-y-6 pt-[8svh]", GUTTER)}
      >
        <h3 className={H3}>Paint</h3>
        <PaintBench scene={scene} />
      </section>
    </section>
  );
}

function PanelTitle({ title, formats }: { title: string; formats?: string }) {
  return (
    <div className="space-y-2">
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
        "block overflow-hidden rounded-lg border border-foreground/12 transition-[border-color,transform] duration-150 ease-out-strong hover:border-foreground/30 active:scale-[0.99]",
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
