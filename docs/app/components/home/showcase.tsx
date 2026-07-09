import { Link } from "waku";
import { showcaseProjects } from "~/data/showcase";

function User({ href, children }: { href: string; children: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="text-foreground underline underline-offset-4 decoration-border hover:decoration-primary"
    >
      {children}
    </a>
  );
}

export function Showcase() {
  return (
    <section className="px-6 py-24 max-sm:py-14">
      <div className="max-w-275 mx-auto">
        <h2 className="font-[540] text-[clamp(2.25rem,4vw,3.5rem)] leading-[1.06] tracking-tight mb-4">
          In production.
        </h2>
        <p className="text-muted-foreground leading-relaxed max-w-150 mb-10">
          <User href="https://dcard.tw">Dcard</User> renders post share images with it,{" "}
          <User href="https://fumadocs.dev">Fumadocs</User> generates its docs OG images, and{" "}
          <User href="https://nuxtseo.com/docs/og-image/renderers/takumi">Nuxt OG Image</User> ships
          it as a built-in renderer.
        </p>

        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
          {showcaseProjects.slice(0, 8).map((project) => (
            <a
              key={project.url}
              href={project.url}
              target="_blank"
              rel="noopener noreferrer"
              className="border border-border hover:border-primary"
            >
              <img
                src={project.image}
                alt={`OG image from ${new URL(project.url).hostname}`}
                width={project.width}
                height={project.height}
                loading="lazy"
                className="w-full h-auto aspect-40/21 object-cover"
              />
            </a>
          ))}
        </div>

        <p className="mt-8">
          <Link
            to="/showcase"
            className="text-sm underline underline-offset-4 decoration-border hover:decoration-primary"
          >
            View all projects →
          </Link>
        </p>
      </div>
    </section>
  );
}
