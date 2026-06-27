import { Link } from "waku";
import { Button } from "~/components/ui/button";

const OUTPUT_BASE = "/images/twitter-images";

const PRINTS = [
  {
    src: `${OUTPUT_BASE}/og-image@2x.webp`,
    alt: "Takumi OG image rendered by Takumi",
    className: "top-0 left-0 -rotate-2",
  },
  {
    src: `${OUTPUT_BASE}/x-post-image@2x.webp`,
    alt: "X post image rendered by Takumi",
    className: "top-[29%] right-0 rotate-[1.5deg]",
  },
  {
    src: `${OUTPUT_BASE}/prisma-og-image@2x.webp`,
    alt: "Prisma-style OG image rendered by Takumi",
    className: "top-[58%] left-[5%] -rotate-1",
  },
] as const;

function PrintSpread() {
  return (
    <div className="max-md:hidden w-105 shrink-0 max-lg:mx-auto animate-reveal-up [animation-delay:300ms]">
      <div className="relative aspect-10/11">
        {PRINTS.map(({ src, alt, className }) => (
          <img
            key={src}
            src={src}
            alt={alt}
            width={2400}
            height={1260}
            className={`absolute w-[86%] h-auto border border-border bg-background shadow-[0_2px_6px_rgba(0,0,0,0.12),0_18px_44px_-14px_rgba(0,0,0,0.4)] ${className}`}
          />
        ))}
      </div>
    </div>
  );
}

export function Hero() {
  return (
    <section className="px-6 pt-20 pb-24 max-sm:pt-12 max-sm:pb-14">
      <div className="max-w-275 mx-auto flex max-lg:flex-col lg:items-center justify-between gap-14">
        <div className="max-w-160">
          <h1 className="font-display font-[540] text-[clamp(3rem,5.2vw,4.75rem)] leading-[1.04] tracking-tight text-balance mb-8 animate-reveal-up">
            Render JSX to images.
            <br />
            <em className="text-primary">Skip the browser.</em>
          </h1>
          <p className="text-[clamp(1rem,2vw,1.125rem)] leading-relaxed text-muted-foreground mb-10 animate-reveal-up [animation-delay:100ms]">
            Takumi parses CSS, lays out the tree, shapes text, and encodes pixels in a single Rust
            binary. Headless Chromium spends 300&nbsp;MB and a cold start on an OG card. Takumi
            spends a function call.
          </p>
          <div className="flex items-center gap-6 flex-wrap animate-reveal-up [animation-delay:200ms]">
            <Button asChild size="lg" className="px-7 font-semibold">
              <Link to="/docs">Get started</Link>
            </Button>
            <Link
              to="/playground"
              className="text-sm underline underline-offset-4 decoration-border hover:decoration-primary"
            >
              Open the playground
            </Link>
            <code className="inline-flex h-10 items-center px-4 border border-dashed border-border font-mono text-sm text-muted-foreground select-all">
              bun i takumi-js
            </code>
          </div>
        </div>

        <PrintSpread />
      </div>
    </section>
  );
}
