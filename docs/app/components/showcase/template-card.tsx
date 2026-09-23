import { ArrowRight } from "lucide-react";
import type { Template } from "~/data/showcase";

export function TemplateCard({ item }: { item: Template }) {
  return (
    <a href={item.href} className="group flex flex-col space-y-4">
      <div className="relative aspect-1200/630 overflow-hidden rounded-xl bg-zinc-950/50 border border-border/40 shadow-sm transition-all duration-500 group-hover:border-primary/30 group-hover:shadow-[0_0_30px_-5px_--theme(--color-primary/0.2)]">
        <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-500 bg-[radial-gradient(circle_at_50%_0%,--theme(--color-primary/0.1)_0%,transparent_70%)] pointer-events-none z-10" />

        <img
          src={item.image}
          alt=""
          className="absolute inset-0 w-full h-full object-cover blur-2xl scale-125 opacity-40 transition-opacity duration-500 group-hover:opacity-60 select-none pointer-events-none"
        />
        <img
          src={item.image}
          alt={`${item.title} layout example`}
          width={1200}
          height={630}
          className="relative w-full h-full object-contain transition-all duration-700 ease-out group-hover:scale-[1.03]"
        />
      </div>
      <div className="flex items-center justify-between px-1">
        <h3 className="font-semibold text-lg text-foreground/90 group-hover:text-foreground transition-colors">
          {item.title} Template
        </h3>
        <div className="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary opacity-0 -translate-x-2 transition-all duration-300 group-hover:opacity-100 group-hover:translate-x-0">
          <ArrowRight className="w-4 h-4" />
        </div>
      </div>
    </a>
  );
}
