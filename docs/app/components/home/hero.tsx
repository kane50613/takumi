import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { PixelField } from "./pixel-field";

export function Hero() {
  return (
    <section className="px-6 pt-20 pb-24 max-sm:pt-12 max-sm:pb-14">
      <div className="max-w-275 mx-auto">
        <div className="flex max-lg:flex-col lg:items-end justify-between gap-10 mb-12">
          <h1 className="font-[540] text-[clamp(3rem,5.2vw,4.75rem)] leading-[1.04] tracking-tight text-balance animate-reveal-up">
            JSX to images and PDFs.
            <br />
            <em className="text-primary">Skip the browser.</em>
          </h1>
          <div className="max-w-90 animate-reveal-up [animation-delay:100ms]">
            <p className="text-[clamp(1rem,2vw,1.125rem)] leading-relaxed text-muted-foreground mb-6">
              Create OG images, social cards, and PDFs from JSX or HTML, styled with CSS and
              Tailwind.
            </p>
            <div className="flex items-center gap-6 flex-wrap">
              <Button asChild size="lg" className="px-7 font-semibold">
                <Link to="/docs">Get started</Link>
              </Button>
              <Link
                to="/playground"
                className="text-sm underline underline-offset-4 decoration-border hover:decoration-primary"
              >
                Open the playground
              </Link>
            </div>
          </div>
        </div>

        <figure className="relative aspect-[2.1] max-md:aspect-[1.3] overflow-hidden rounded-3xl border border-border bg-[#0d0c0b] animate-reveal-up [animation-delay:200ms]">
          <PixelField />
          <figcaption className="absolute inset-x-5 bottom-4 flex justify-between font-mono text-xs text-white/50">
            <span>1200 × 630 · rendered by Takumi</span>
            <code className="select-all">bun i takumi-js</code>
          </figcaption>
        </figure>
      </div>
    </section>
  );
}
