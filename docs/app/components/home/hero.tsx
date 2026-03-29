import { Link } from "react-router";
import { Button } from "~/components/ui/button";
import { AnimatedOrb } from "./animated-orb";

export function Hero() {
  return (
    <section className="relative min-h-[70dvh] max-sm:min-h-auto flex flex-col items-center justify-center px-6 py-16 overflow-hidden">
      <AnimatedOrb />

      <div className="relative text-center max-w-200 z-10">
        <h1 className="font-display text-[clamp(2.8rem,7vw,5.5rem)] font-[750] leading-[1.05] tracking-tighter mb-6 animate-reveal-up [animation-delay:100ms]">
          <span className="block">Render React</span>
          <span className="block">components into</span>
          <span className="block bg-linear-to-br from-primary to-[#ffa944] bg-clip-text text-transparent pb-2">
            images, animations.
          </span>
        </h1>
        <p className="text-[clamp(1rem,2vw,1.2rem)] leading-relaxed text-muted-foreground max-w-160 mx-auto mb-10 animate-reveal-up [animation-delay:200ms]">
          From JSX to images, animations, and video frames at native speed. Supports rich CSS
          layout, WOFF2 fonts, and complex text scripts.
        </p>

        <div className="flex gap-3 justify-center flex-wrap animate-reveal-up [animation-delay:300ms]">
          <Button
            asChild
            size="lg"
            className="rounded-full! bg-primary! text-white! border-none! px-8! font-semibold! transition-all duration-300 hover:-translate-y-0.5! hover:shadow-[0_8px_30px_rgba(255,53,53,0.3)]!"
          >
            <Link to="/docs">Get Started</Link>
          </Button>
          <Button
            asChild
            size="lg"
            variant="outline"
            className="rounded-full! border-border! bg-muted/50! backdrop-blur-sm! transition-all duration-300 hover:border-primary/40! hover:bg-muted! hover:-translate-y-0.5!"
          >
            <Link to="/playground">Open Playground</Link>
          </Button>
        </div>
      </div>
    </section>
  );
}
