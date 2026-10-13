import { Link } from "waku";
import { PROJECT_LINK_REL, showcaseProjects } from "~/data/showcase";
import { H2, LEAD, LINK, Section, TEXT_COLUMN, VISUAL_COLUMN } from "./section";

function User({ href, children }: { href: string; children: string }) {
  return (
    <a href={href} target="_blank" rel="noopener noreferrer" className={`${LINK} text-foreground`}>
      {children}
    </a>
  );
}

export function Showcase() {
  return (
    <Section>
      <div className={`${TEXT_COLUMN} space-y-6`}>
        <h2 className={H2}>In production.</h2>
        <p className={LEAD}>
          <User href="https://dcard.tw">Dcard</User> share cards.{" "}
          <User href="https://fumadocs.dev">Fumadocs</User> OG images. Built into{" "}
          <User href="https://nuxtseo.com/docs/og-image/renderers/takumi">Nuxt OG Image</User>.
        </p>
        <Link to="/showcase" className={`${LINK} text-sm`}>
          View all projects
        </Link>
      </div>
      <div className={`${VISUAL_COLUMN} grid grid-cols-2 gap-4`}>
        {showcaseProjects.slice(0, 6).map((project) => (
          <a
            key={project.url}
            href={project.url}
            target="_blank"
            rel={PROJECT_LINK_REL}
            className="overflow-hidden rounded-lg border transition-[border-color] duration-150 ease-out hover:border-foreground/30"
          >
            <img
              src={project.image}
              alt={`OG image from ${new URL(project.url).hostname}`}
              width={project.width}
              height={project.height}
              loading="lazy"
              decoding="async"
              className="w-full h-auto aspect-40/21 object-cover"
            />
          </a>
        ))}
      </div>
    </Section>
  );
}
