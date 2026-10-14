import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { PROJECT_LINK_REL, showcaseProjects } from "~/data/showcase";
import { cn } from "~/lib/utils";
import { Reveal } from "./reveal";
import { GUTTER, H2, LINK, PRIMARY_BUTTON } from "./typography";

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
      className={cn("space-y-[6svh] pt-[8svh] pb-[10svh]", GUTTER)}
    >
      <h2 id="showcase-title" className={H2}>
        In production.
      </h2>

      <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
        {featured.map((project, index) => {
          const name = project.title ?? new URL(project.url).hostname;

          return (
            <Reveal key={project.url} delay={index * 0.12}>
              <a
                href={project.url}
                target="_blank"
                rel={PROJECT_LINK_REL}
                className="block overflow-hidden rounded-lg border border-foreground/12 transition-[border-color,transform] duration-150 ease-out-strong hover:border-foreground/30 active:scale-[0.99]"
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
            </Reveal>
          );
        })}
      </div>

      <div className="flex flex-col items-start justify-between gap-8 pt-[12svh] lg:flex-row lg:items-end">
        <img
          src="/sticker.svg"
          alt="Takumi"
          width={530}
          height={210}
          loading="lazy"
          decoding="async"
          className="h-20 w-auto sm:h-24"
        />

        <div className="flex flex-wrap items-center gap-x-6 gap-y-4">
          <Button asChild size="lg" className={PRIMARY_BUTTON}>
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
