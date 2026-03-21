import { Link } from "react-router";
import { Button } from "~/components/ui/button";

export function CTA({ highlightedHtml }: { highlightedHtml: string }) {
  return (
    <section className="px-6 py-24 pb-32 max-sm:py-12 max-sm:pb-20">
      <div className="relative max-w-[900px] mx-auto rounded-3xl border border-border overflow-hidden">
        <div className="absolute -top-1/2 -left-[20%] w-[140%] h-[200%] bg-[radial-gradient(ellipse_at_50%_0%,rgba(var(--primary),0.08),transparent_60%)] pointer-events-none" />

        <div className="relative px-12 py-16 max-sm:px-6 max-sm:py-10 text-center">
          <h2 className="font-display text-[clamp(2rem,4vw,3rem)] font-[750] tracking-tighter leading-tight mb-4">
            Start rendering
            <br />
            <span className="bg-linear-to-br from-primary to-[#ffa944] bg-clip-text text-transparent">
              in minutes.
            </span>
          </h2>
          <p className="text-[1.05rem] text-muted-foreground max-w-[420px] mx-auto mb-8 leading-relaxed">
            Install the package, write your first component, and generate your image. It's that
            simple.
          </p>
          <div
            suppressHydrationWarning
            className="inline-block px-6 py-2.5 rounded-lg bg-muted border border-border font-mono text-sm text-foreground mb-8 select-all [&_pre]:bg-transparent! [&_pre]:m-0! [&_pre]:p-0! [&_code]:bg-transparent!"
            dangerouslySetInnerHTML={{ __html: highlightedHtml }}
          />
          <div className="flex gap-3 justify-center flex-wrap">
            <Button
              asChild
              size="lg"
              className="rounded-full! bg-primary! text-white! border-none! px-8! font-semibold! transition-all duration-300 hover:-translate-y-0.5! hover:shadow-[0_8px_30px_rgba(255,53,53,0.3)]!"
            >
              <Link to="/docs">Quick Start</Link>
            </Button>
            <Button
              asChild
              size="lg"
              variant="outline"
              className="rounded-full! border-border! bg-muted/50! backdrop-blur-sm! transition-all duration-300 hover:border-primary/40! hover:bg-muted! hover:-translate-y-0.5!"
            >
              <a
                href="https://github.com/kane50613/takumi"
                target="_blank"
                rel="noopener noreferrer"
              >
                Star on GitHub
              </a>
            </Button>
          </div>
        </div>
      </div>
    </section>
  );
}
