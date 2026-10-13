import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { PROJECT_LINK_REL, showcaseProjects } from "~/data/showcase";
import { cn } from "~/lib/utils";
import { GUTTER, H2, LEAD, LINK, VALUE } from "./typography";

const FEATURED_URLS = new Set([
  "https://dcard.tw",
  "https://tanstack.com",
  "https://fumadocs.dev/",
  "https://lu.ma",
]);

const featured = showcaseProjects.filter((project) => FEATURED_URLS.has(project.url));

const OFFSETS = [
  "lg:w-[30%] lg:mt-0",
  "lg:w-[26%] lg:mt-14",
  "lg:w-[26%] lg:mt-4",
  "lg:w-[28%] lg:mt-20",
] as const;

export function Showcase() {
  return (
    <section
      aria-labelledby="showcase-title"
      className={cn("space-y-[6svh] pt-[4svh] pb-[10svh]", GUTTER)}
    >
      <div className="max-w-[48rem] space-y-3">
        <h2 id="showcase-title" className={H2}>
          Forged in production.
        </h2>
        <p className={LEAD}>
          Dcard, TanStack, Fumadocs, Luma, and Nuxt OG Image render in Node.js, Bun, WebAssembly,
          and Rust.
        </p>
      </div>

      {/* Asymmetric staggered spread of production share images */}
      <div className="flex flex-col gap-6 sm:flex-row sm:flex-wrap lg:flex-nowrap lg:items-start lg:justify-between">
        {featured.map((project, index) => {
          const name = project.title ?? new URL(project.url).hostname;

          return (
            <a
              key={project.url}
              href={project.url}
              target="_blank"
              rel={PROJECT_LINK_REL}
              className={cn(
                "group block w-full overflow-hidden rounded-2xl border border-foreground/15 shadow-lg transition-transform duration-200 ease-out hover:-translate-y-1 active:scale-[0.98] sm:w-[calc(50%-0.75rem)]",
                OFFSETS[index],
              )}
            >
              <img
                src={project.image}
                alt={`Production Open Graph image rendered by ${name}`}
                width={project.width}
                height={project.height}
                loading="lazy"
                decoding="async"
                className="aspect-40/21 h-auto w-full object-cover"
              />
            </a>
          );
        })}
      </div>

      {/* Closing Workshop Stamp: Sticker Wordmark + Playground & Docs Actions */}
      <div className="flex flex-col items-start justify-between gap-8 pt-[4svh] lg:flex-row lg:items-end">
        <div className="space-y-4">
          <img
            src="/sticker.svg"
            alt="Takumi"
            width={530}
            height={210}
            loading="lazy"
            decoding="async"
            className="h-20 w-auto sm:h-24"
          />
          <p className={VALUE}>Node.js · Bun · WebAssembly · Cloudflare Workers · Rust</p>
        </div>

        <div className="flex flex-wrap items-center gap-x-6 gap-y-4">
          <Button
            asChild
            size="lg"
            className="h-12 rounded-xl bg-primary px-7 text-base font-bold text-primary-foreground shadow-sm transition-transform duration-150 ease-out hover:bg-primary/90 active:scale-[0.97]"
          >
            <Link to="/playground">Try in playground</Link>
          </Button>
          <Link to="/docs" className={cn(LINK, "text-lg")}>
            Read documentation
          </Link>
          <Link to="/showcase" className={cn(LINK, "text-lg")}>
            View showcase
          </Link>
        </div>
      </div>
    </section>
  );
}
