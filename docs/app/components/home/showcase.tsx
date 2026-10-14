import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { PROJECT_LINK_REL, showcaseProjects } from "~/data/showcase";
import { cn } from "~/lib/utils";
import { Rise } from "./rise";
import { GUTTER, H2, LINK, VALUE } from "./typography";

const FEATURED_URLS = new Set([
  "https://dcard.tw",
  "https://tanstack.com",
  "https://fumadocs.dev/",
  "https://lu.ma",
]);

const featured = showcaseProjects.filter((project) => FEATURED_URLS.has(project.url));

export function Showcase() {
  return (
    <section
      aria-labelledby="showcase-title"
      className={cn("space-y-[6svh] pt-[4svh] pb-[10svh]", GUTTER)}
    >
      <h2 id="showcase-title" className={H2}>
        Forged in production.
      </h2>

      <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
        {featured.map((project, index) => {
          const name = project.title ?? new URL(project.url).hostname;

          return (
            <Rise key={project.url} lag={index * 56}>
              <a
                href={project.url}
                target="_blank"
                rel={PROJECT_LINK_REL}
                className="group block overflow-hidden rounded-2xl border border-foreground/15 shadow-lg transition-transform duration-200 ease-out-strong hover:-translate-y-1 active:scale-[0.98]"
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
            </Rise>
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
            className="h-12 rounded-xl bg-primary px-7 text-base font-bold text-primary-foreground shadow-sm transition-[transform,background-color] duration-150 ease-out-strong hover:bg-primary/90 active:scale-[0.97]"
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
