import { ArrowRight } from "lucide-react";
import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { ShowcaseMarquee } from "./showcase-marquee";

export function Showcase() {
  return (
    <section className="px-6 py-24 max-sm:py-12">
      <div className="max-w-275 mx-auto mb-10">
        <span className="inline-block text-xs font-semibold uppercase tracking-[0.12em] text-primary mb-3 px-3 py-1 rounded-full bg-primary/20">
          Showcase
        </span>
        <h2 className="font-display text-[clamp(2rem,4vw,3.2rem)] font-[750] tracking-tighter leading-tight mt-3">
          Built with Takumi
        </h2>
        <p className="text-[1.05rem] leading-relaxed text-muted-foreground max-w-130 mt-4">
          From OG images to dynamic cards, see what the community is building.
        </p>
      </div>
      <ShowcaseMarquee />
      <div className="max-w-275 mx-auto mt-8 text-center">
        <Button
          asChild
          variant="outline"
          className="rounded-full! border-border! bg-muted/50! backdrop-blur-sm! transition-all duration-300 hover:border-primary/40! hover:bg-muted! hover:-translate-y-0.5!"
        >
          <Link to="/showcase">
            View all projects <ArrowRight />
          </Link>
        </Button>
      </div>
    </section>
  );
}
