import { Link } from "waku";
import { PROJECT_LINK_REL, showcaseProjects } from "~/data/showcase";
import { H2, LINE, LINK } from "./typography";

export function Showcase() {
  return (
    <section className="relative z-10 px-6 py-28 lg:px-[max(6vw,2rem)]">
      <div className="mb-10 space-y-4 lg:w-[26rem]">
        <h2 className={H2}>In production.</h2>
        <p className={LINE}>
          Dcard share cards, Fumadocs OG images, and the renderer behind Nuxt OG Image.{" "}
          <Link to="/showcase" className={LINK}>
            See all projects
          </Link>
        </p>
      </div>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
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
