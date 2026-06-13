import { getPageTreePeers, type Root as PageTreeRoot } from "fumadocs-core/page-tree";
import * as Twoslash from "fumadocs-twoslash/ui";
import { Card, Cards } from "fumadocs-ui/components/card";
import { Step, Steps } from "fumadocs-ui/components/steps";
import { Tab, Tabs } from "fumadocs-ui/components/tabs";
import defaultMdxComponents, { createRelativeLink } from "fumadocs-ui/mdx";
import { DocsBody, DocsDescription, DocsPage, DocsTitle } from "fumadocs-ui/page";
import { ArrowBigRight, BookOpen, FileCode2, Hand, Shovel, ToyBrick, Wrench } from "lucide-react";
import type { PageProps } from "waku/router";
import { unstable_notFound } from "waku/router/server";
import { Accordion, Accordions } from "~/components/accordion";
import { Mermaid } from "~/components/mdx/mermaid";
import { Seo } from "~/components/seo";
import { TypeTable } from "~/components/type-table";
import { Video } from "~/components/video";
import { source } from "~/source";

const components = {
  ...defaultMdxComponents,
  ...Twoslash,
  Hand,
  BookOpen,
  FileCode2,
  Wrench,
  Shovel,
  ToyBrick,
  ArrowBigRight,
  Mermaid,
  Step,
  Steps,
  Cards,
  Card,
  DocsCategory,
  Tabs,
  Tab,
  Accordion,
  Accordions,
  TypeTable,
  Video,
};

export default function Page({ slugs = [] }: PageProps<"/docs/[...slugs]">) {
  const page = source.getPage(slugs);

  if (!page) {
    unstable_notFound();
  }

  const MDX = page.data.body;
  const title = `${page.data.title} — Takumi`;
  const og = ["https://takumi.kane.tw/og", "docs", ...slugs, "image.webp"].join("/");
  const tree = source.getPageTree() as PageTreeRoot;

  return (
    <DocsPage
      toc={page.data.toc}
      tableOfContent={{
        style: "clerk",
      }}
      lastUpdate={page.data.lastModified}
      editOnGithub={{
        owner: "kane50613",
        repo: "takumi",
        sha: "master",
        path: `/docs/content/docs/${page.path}?plain=1`,
      }}
    >
      <Seo title={title} description={page.data.description} path={page.url} image={og} />
      <DocsTitle>{page.data.title}</DocsTitle>
      <DocsDescription>{page.data.description}</DocsDescription>
      <DocsBody>
        <MDX
          components={{
            ...components,
            a: createRelativeLink(source, page),
          }}
        />
        {page.data.index ? <DocsCategory tree={tree} url={page.url} /> : null}
      </DocsBody>
    </DocsPage>
  );
}

function DocsCategory({ tree, url }: { tree: PageTreeRoot; url: string }) {
  return (
    <Cards>
      {getPageTreePeers(tree, url).map((peer) => (
        <Card key={peer.url} title={peer.name} href={peer.url}>
          {peer.description}
        </Card>
      ))}
    </Cards>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
    staticPaths: source.getPages().map((page) => page.slugs ?? []),
  };
}
