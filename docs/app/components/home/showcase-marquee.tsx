import { useMemo } from "react";
import { showcaseProjects } from "../../data/showcase";

export function ShowcaseMarquee() {
  const projects = useMemo(() => [...showcaseProjects, ...showcaseProjects], []);

  return (
    <div className="overflow-hidden mask-[linear-gradient(to_right,transparent,black_10%,black_90%,transparent)] py-4">
      <div className="flex gap-5 animate-marquee w-max">
        {projects.map((project, i) => (
          <a
            key={`${project.url}-${i}`}
            href={project.url}
            target="_blank"
            rel="noopener noreferrer"
            className="shrink-0 w-80 max-sm:w-60 rounded-xl overflow-hidden border border-white/6 bg-white/3 transition-all duration-400 hover:border-primary/40 hover:scale-[1.03] hover:shadow-[0_12px_40px_rgba(255,53,53,0.1)]"
          >
            <img
              src={project.image}
              alt={project.title || "Showcase project"}
              className="w-full h-full block object-cover"
              width={project.width}
              height={project.height}
              loading="lazy"
              decoding="async"
            />
          </a>
        ))}
      </div>
    </div>
  );
}
