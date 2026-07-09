"use client";

import { HomeLayout } from "fumadocs-ui/layouts/home";
import JSConfetti from "js-confetti";
import { ArrowRight, Heart } from "lucide-react";
import { useCallback, useRef } from "react";
import { GithubIcon } from "~/components/showcase/github-icon";
import { ShowcaseCard } from "~/components/showcase/showcase-card";
import { TemplateCard } from "~/components/showcase/template-card";
import { Seo } from "~/components/seo";
import { baseOptions } from "~/layout-config";
import { showcaseProjects, showcaseTemplates } from "~/data/showcase";

const TITLE = "Showcase · Takumi";
const DESCRIPTION =
  "Discover how developers are using Takumi to power their dynamic image generation.";

export default function Showcase() {
  const confettiRef = useRef<JSConfetti | null>(null);

  const onConfetti = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
    if (!confettiRef.current) {
      confettiRef.current = new JSConfetti();
    }

    const rect = e.currentTarget.getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;

    confettiRef.current.addConfettiAtPosition({
      emojis: ["❤️", "🪓", "🎨", "✨"],
      emojiSize: 40,
      confettiNumber: 30,
      confettiDispatchPosition: { x, y },
    });
  }, []);

  return (
    <HomeLayout className="overflow-x-hidden" {...baseOptions}>
      <Seo title={TITLE} description={DESCRIPTION} path="/showcase" />

      <div className="fixed inset-0 pointer-events-none z-[-1] overflow-hidden">
        <div className="absolute top-[-20%] left-[-10%] w-[60%] h-[60%] opacity-[0.03] bg-[radial-gradient(circle_at_center,var(--color-primary)_0%,transparent_70%)] blur-[100px]" />
        <div className="absolute bottom-[-20%] right-[-10%] w-[60%] h-[60%] opacity-[0.03] bg-[radial-gradient(circle_at_center,var(--color-primary)_0%,transparent_70%)] blur-[100px]" />
      </div>

      <div className="max-w-300 mx-auto px-6 py-24 max-sm:py-16">
        <div className="flex flex-col items-center text-center mb-24 max-w-3xl mx-auto">
          <div className="mb-8 relative group">
            <div className="absolute -inset-4 bg-primary/20 blur-2xl rounded-full animate-pulse duration-700 opacity-50 group-hover:opacity-100 transition-opacity" />
            <button
              type="button"
              onClick={onConfetti}
              className="relative flex items-center justify-center w-20 h-20 transition-transform active:scale-95 cursor-pointer outline-none bg-background rounded-full border border-border/50 shadow-[0_0_30px_-5px_--theme(--color-primary/0.3)] backdrop-blur-sm group-hover:border-primary/40 group-hover:shadow-[0_0_40px_-5px_--theme(--color-primary/0.5)] z-10"
              aria-label="Celebrate"
            >
              <Heart className="w-8 h-8 text-primary fill-primary/20 group-hover:fill-primary transition-all duration-300" />
            </button>
          </div>

          <h1 className="text-[clamp(2.5rem,6vw,4.5rem)] font-[750] tracking-tighter leading-[1.1] mb-6">
            Crafted with <span className="text-primary">Takumi</span>
          </h1>
          <p className="text-[1.1rem] md:text-[1.25rem] leading-relaxed text-muted-foreground text-pretty">
            Explore a curated collection of production applications and open-source projects
            leveraging Takumi's high-performance image engine.
          </p>
        </div>

        <section className="mb-32">
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 auto-rows-fr">
            {showcaseProjects.map((project) => (
              <ShowcaseCard key={project.url} project={project} />
            ))}
          </div>
        </section>

        <section className="mb-32">
          <div className="mb-12 border-b border-border/50 pb-8 flex flex-col md:flex-row md:items-end justify-between gap-6">
            <div>
              <span className="inline-block text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground mb-3">
                Starter Kits
              </span>
              <h2 className="text-[clamp(2rem,4vw,2.5rem)] font-[750] tracking-tighter leading-tight">
                Ready-to-use Templates
              </h2>
            </div>
            <p className="text-muted-foreground max-w-sm md:text-right">
              Kickstart your generation with our pre-built, responsive canvas layouts.
            </p>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-8">
            {showcaseTemplates.map((item) => (
              <TemplateCard key={item.title} item={item} />
            ))}
          </div>
        </section>

        <section className="relative overflow-hidden rounded-3xl border border-border/40 bg-zinc-950/50 dark:bg-zinc-900/20 backdrop-blur-xl">
          <div className="absolute inset-0 bg-[linear-gradient(to_right,--theme(--color-zinc-800/0.1)_1px,transparent_1px),linear-gradient(to_bottom,--theme(--color-zinc-800/0.1)_1px,transparent_1px)] bg-size-[40px_40px] opacity-20 pointer-events-none" />
          <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-150 h-100 bg-primary/10 blur-[100px] rounded-full pointer-events-none" />

          <div className="relative px-8 py-20 md:py-24 flex flex-col items-center text-center z-10">
            <div className="text-primary/80 mb-6">
              <GithubIcon size={48} />
            </div>
            <h2 className="text-3xl md:text-4xl font-[750] tracking-tighter mb-4 text-foreground">
              Feature your creation
            </h2>
            <p className="text-muted-foreground text-lg mb-10 max-w-125">
              Forged something exceptional? Submit your project to the showcase and share your craft
              with the community.
            </p>

            <a
              href="https://github.com/kane50613/takumi/edit/master/docs/app/data/showcase.ts"
              target="_blank"
              rel="noreferrer"
              className="group inline-flex items-center gap-3 px-8 py-4 bg-primary text-primary-foreground font-semibold rounded-full hover:bg-primary/90 transition-all shadow-[0_0_20px_-5px_--theme(--color-primary/0.4)] hover:shadow-[0_0_30px_-5px_--theme(--color-primary/0.6)] hover:-translate-y-0.5 active:translate-y-0 active:scale-95"
            >
              Submit Pull Request
              <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1" />
            </a>
          </div>
        </section>
      </div>
    </HomeLayout>
  );
}
