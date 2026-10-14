import { cn } from "~/lib/utils";
import { scene } from "./card";
import { Gallery } from "./gallery";
import { PaintBench } from "./paint-bench";
import { BODY, GUTTER, H2, H3, VALUE } from "./typography";

const OUTPUT = "/images/twitter-images";
const PANEL = "grid w-full max-w-[76rem] items-center gap-x-12 gap-y-6 lg:grid-cols-[19rem_1fr]";

/** The outputs the anvil strike forges, wiped in one by one, then its draw steps replayed. */
export function Outputs() {
  return (
    <section id="outputs" aria-labelledby="outputs-title" className="relative pt-[12svh] pb-[8svh]">
      <h2 id="outputs-title" className={cn(H2, GUTTER)}>
        One component, every format.
      </h2>

      <Gallery>
        <section aria-label="Images" className={PANEL}>
          <PanelTitle
            title="Images"
            line="Open Graph cards, blog covers, and quote cards at any size."
            formats={["PNG", "WebP", "JPEG"]}
          />
          <div className="relative pb-12">
            <OutputImage
              href={`${OUTPUT}/home-usecase-blog-cover.png`}
              src={`${OUTPUT}/home-usecase-blog-cover@2x.webp`}
              alt="Blog cover with a lighthouse at night rendered as PNG"
              width={1037}
              height={544}
              className="w-[82%]"
            />
            <OutputImage
              href={`${OUTPUT}/home-usecase-quote-card.webp`}
              src={`${OUTPUT}/home-usecase-quote-card.svg`}
              alt="Square quote card rendered as WebP"
              width={544}
              height={544}
              className="absolute -bottom-2 right-0 w-[34%] ring-8 ring-background"
            />
          </div>
        </section>

        <section aria-label="Animations" className={PANEL}>
          <PanelTitle
            title="Animations"
            line="CSS keyframes, played frame by frame into one file."
            formats={["Animated WebP", "APNG", "GIF"]}
          />
          <OutputImage
            href={`${OUTPUT}/home-usecase-now-playing.webp`}
            src={`${OUTPUT}/home-usecase-now-playing.webp`}
            alt="Animated now-playing card with moving CSS keyframe equalizer bars"
            width={960}
            height={360}
          />
        </section>

        <section aria-label="SVG" className={PANEL}>
          <PanelTitle
            title="SVG"
            line="Vector banners and README art that stay sharp at any scale."
          />
          <OutputImage
            href={`${OUTPUT}/home-usecase-repo-banner.svg`}
            src={`${OUTPUT}/home-usecase-repo-banner.svg`}
            alt="Repository banner rendered as scalable vector SVG"
            width={1280}
            height={440}
          />
        </section>

        <section aria-label="PDF" className={PANEL}>
          <PanelTitle
            title="PDF"
            line="Invoices, tickets, and certificates as paged PDFs with selectable text."
          />
          <div className="flex items-start gap-5">
            <OutputImage
              href={`${OUTPUT}/home-usecase-invoice.pdf`}
              src={`${OUTPUT}/home-usecase-invoice.svg`}
              alt="A4 invoice PDF for Hollow Oak Studio with selectable text"
              width={385}
              height={544}
              className="w-[40%] shrink-0 bg-white"
            />
            <div className="flex flex-1 flex-col gap-5 pt-6">
              <OutputImage
                href={`${OUTPUT}/home-usecase-ticket.pdf`}
                src={`${OUTPUT}/home-usecase-ticket.svg`}
                alt="HARBOR/26 Lisbon conference pass PDF with barcode stub"
                width={1322}
                height={544}
              />
              <OutputImage
                href={`${OUTPUT}/home-usecase-certificate.pdf`}
                src={`${OUTPUT}/home-usecase-certificate.svg`}
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
        <PanelTitle
          title="Paint"
          line="The draw steps behind the card above, replayed in paint order."
        />
        <PaintBench scene={scene} />
      </section>
    </section>
  );
}

function PanelTitle({
  title,
  line,
  formats = [],
}: {
  title: string;
  line: string;
  formats?: string[];
}) {
  return (
    <div className="space-y-3">
      <h3 className={H3}>{title}</h3>
      <p className={BODY}>{line}</p>
      {formats.length > 0 && (
        <ul className={cn(VALUE, "flex flex-wrap gap-x-4 gap-y-1")}>
          {formats.map((format) => (
            <li key={format} className="whitespace-nowrap">
              {format}
            </li>
          ))}
        </ul>
      )}
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
        "block overflow-hidden rounded-lg border border-foreground/12 bg-background transition-[border-color,transform] duration-150 ease-out-strong hover:border-foreground/30 active:scale-[0.99]",
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
