import type { ReactNode } from "react";
import { HomeLayout } from "fumadocs-ui/layouts/home";
import { Seo, SiteJsonLd } from "~/components/seo";
import { baseOptions } from "~/layout-config";

export function ProsePage({
  title,
  heading,
  description,
  path,
  children,
}: {
  title: string;
  heading: string;
  description: string;
  path: string;
  children: ReactNode;
}) {
  return (
    <HomeLayout className="overflow-x-hidden" {...baseOptions}>
      <Seo title={title} description={description} path={path} />
      <SiteJsonLd />

      <div className="max-w-3xl mx-auto px-6 py-24 max-sm:py-16">
        <h1 className="font-[540] text-[clamp(2.25rem,4vw,3.5rem)] leading-[1.06] tracking-tight mb-4">
          {heading}
        </h1>
        <p className="text-muted-foreground leading-relaxed mb-10">{description}</p>
        <div className="flex flex-col gap-6 leading-relaxed [&_h2]:font-[540] [&_h2]:text-2xl [&_h2]:tracking-tight [&_a]:text-primary [&_a]:underline [&_a]:underline-offset-4">
          {children}
        </div>
      </div>
    </HomeLayout>
  );
}
