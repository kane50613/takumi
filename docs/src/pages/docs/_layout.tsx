import type { Root as PageTreeRoot } from "fumadocs-core/page-tree";
import { DocsLayout } from "fumadocs-ui/layouts/docs";
import { Shovel, Sparkles } from "lucide-react";
import type { ReactNode } from "react";
import { baseOptions } from "~/layout-config";
import { source } from "~/source";

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <DocsLayout
      {...baseOptions}
      links={[
        {
          icon: <Shovel />,
          text: "Try in Playground",
          url: "/playground",
        },
        {
          icon: <Sparkles />,
          text: "Showcase",
          url: "/showcase",
        },
      ]}
      tree={source.getPageTree() as PageTreeRoot}
    >
      {children}
    </DocsLayout>
  );
}
