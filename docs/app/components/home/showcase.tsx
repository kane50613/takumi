import { Link } from "waku";
import { cn } from "~/lib/utils";
import { PROJECT_LINK_REL, showcaseProjects } from "~/data/showcase";
import { BODY, GUTTER, H2, LINK } from "./typography";

export function Showcase() {
  return (
    <section aria-labelledby="showcase-title" className={cn("py-[8svh]", GUTTER)}>
      <div className="mb-10 max-w-[52rem] space-y-4">
        <h2 id="showcase-title" className={H2}>
          In production.
        </h2>
        <p className={BODY}>
          Dcard share cards, Fumadocs OG images, and the renderer behind Nuxt OG Image.{" "}
          <Link to="/showcase" className={LINK}>
            See all projects
          </Link>
        </p>
      </div>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {showcaseProjects.slice(0, 4).map((project) => (
          <a
            key={project.url}
            href={project.url}
            target="_blank"
            rel={PROJECT_LINK_REL}
            className="overflow-hidden rounded-md border transition-[border-color] duration-150 ease-out hover:border-foreground/40"
          >
            <img
              src={project.image}
              alt={`OG image from ${new URL(project.url).hostname}`}
              width={project.width}
              height={project.height}
              loading="lazy"
              decoding="async"
              className="aspect-40/21 h-auto w-full object-cover"
            />
          </a>
        ))}
      </div>
    </section>
  );
}
