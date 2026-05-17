import { showcaseFeatures } from "~/data/showcase";
import { FeatureCard } from "./feature-card";

export function Features() {
  return (
    <section className="px-6 py-24 max-sm:py-12">
      <div className="max-w-275 mx-auto">
        <div className="mb-14">
          <h2 className="font-display text-[clamp(2rem,4vw,3.2rem)] font-[750] tracking-tighter leading-tight mt-3">
            Still images are just the beginning. CSS animations, video pipelines, batch at native
            speed.
          </h2>
        </div>
        <div className="grid grid-cols-3 max-md:grid-cols-1 gap-px rounded-2xl overflow-hidden border border-border bg-border">
          {showcaseFeatures.map((feature, i) => (
            <FeatureCard key={feature.title} feature={feature} index={i} />
          ))}
          <div
            className="col-span-3 max-md:col-span-1 p-8 bg-background backdrop-blur-sm transition-colors duration-400 hover:bg-muted/50 animate-reveal-up"
            style={{ animationDelay: "560ms" }}
          >
            <div className="w-10 h-10 flex items-center justify-center rounded-lg bg-primary/20 text-primary mb-5">
              <svg
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                <line x1="8" y1="21" x2="16" y2="21" />
                <line x1="12" y1="17" x2="12" y2="21" />
              </svg>
            </div>
            <h3 className="font-display text-lg font-bold mb-2 tracking-tight">Runs Everywhere</h3>
            <p className="text-sm leading-relaxed text-muted-foreground">
              Node.js native, WebAssembly for Edge, or embed the Rust crate.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
