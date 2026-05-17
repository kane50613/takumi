import type { showcaseFeatures } from "~/data/showcase";

export interface FeatureCardProps {
  feature: (typeof showcaseFeatures)[number];
  index: number;
}

export function FeatureCard({ feature, index }: FeatureCardProps) {
  return (
    <div
      className={`p-8 bg-background backdrop-blur-sm transition-colors duration-400 hover:bg-muted/50 animate-reveal-up ${
        index === 0 ? "bg-linear-to-br from-primary/10 to-transparent" : ""
      }`}
      style={{ animationDelay: `${200 + index * 120}ms` }}
    >
      <div className="w-10 h-10 flex items-center justify-center rounded-lg bg-primary/20 text-primary mb-5">
        <feature.icon className="w-5 h-5" />
      </div>
      <h3 className="font-display text-lg font-bold mb-2 tracking-tight">{feature.title}</h3>
      <p className="text-sm leading-relaxed text-muted-foreground">{feature.description}</p>
    </div>
  );
}
