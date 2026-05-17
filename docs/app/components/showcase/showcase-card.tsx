"use client";

import { Link2Icon } from "lucide-react";
import { useMemo } from "react";
import type { Project } from "~/data/showcase";
import { GithubIcon } from "./github-icon";

export interface ShowcaseCardProps {
  project: Project;
}

export function ShowcaseCard({ project }: ShowcaseCardProps) {
  const title = useMemo(() => {
    if (!project.title) {
      const { hostname, pathname } = new URL(project.url);

      if (hostname === "github.com") {
        const [owner, repo] = pathname.split("/").filter(Boolean);

        return `${owner}/${repo}`;
      }

      return hostname;
    }

    return project.title;
  }, [project.title, project.url]);

  const icon = useMemo(() => {
    if (project.url.includes("github.com")) {
      return <GithubIcon size={16} />;
    }

    return <Link2Icon size={16} />;
  }, [project.url]);

  return (
    <a
      href={project.url}
      target="_blank"
      rel="noopener noreferrer"
      className="flex flex-col rounded-xl overflow-hidden group bg-zinc-950/20 border border-border/40 shadow-sm transition-all duration-500 hover:border-primary/30 hover:shadow-[0_0_30px_-5px_--theme(--color-primary/0.15)] relative"
    >
      <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-500 bg-[radial-gradient(circle_at_50%_0%,--theme(--color-primary/0.05)_0%,transparent_50%)] pointer-events-none z-10" />

      <div className="relative aspect-1200/630 overflow-hidden bg-zinc-950/40 border-b border-border/40">
        <img
          src={project.image}
          alt=""
          className="absolute inset-0 w-full h-full object-cover blur-[30px] scale-125 opacity-30 transition-opacity duration-500 group-hover:opacity-50 select-none pointer-events-none"
          width={project.width}
          height={project.height}
          loading="lazy"
          decoding="async"
        />
        <img
          src={project.image}
          alt={title}
          className="relative w-full h-full object-contain transition-all duration-700 ease-out group-hover:scale-[1.03]"
          width={project.width}
          height={project.height}
          loading="lazy"
          decoding="async"
        />
      </div>
      <div className="px-3 py-2.5 flex items-center justify-between bg-zinc-950/40 backdrop-blur-sm z-20">
        <div className="flex items-center gap-2 text-muted-foreground group-hover:text-foreground transition-colors duration-300">
          <div className="flex items-center justify-center p-1 rounded-md bg-foreground/5 text-foreground/70 group-hover:text-primary group-hover:bg-primary/10 transition-colors duration-300">
            {icon}
          </div>
          <span className="text-sm font-medium tracking-tight truncate">{title}</span>
        </div>
      </div>
    </a>
  );
}
