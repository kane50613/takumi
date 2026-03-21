import { getPageTreePeers, type Root as PageTreeRoot } from "fumadocs-core/page-tree";
import * as Twoslash from "fumadocs-twoslash/ui";
import { Card, Cards } from "fumadocs-ui/components/card";
import { Tab, Tabs } from "fumadocs-ui/components/tabs";
import { DocsLayout } from "fumadocs-ui/layouts/docs";
import defaultMdxComponents from "fumadocs-ui/mdx";
import { DocsBody, DocsDescription, DocsPage, DocsTitle } from "fumadocs-ui/page";
import { ArrowBigRight, BookOpen, FileCode2, Hand, Shovel, Sparkles, Wrench } from "lucide-react";
import { redirect } from "react-router";
import { Accordion, Accordions } from "~/components/accordion";
import { TypeTable } from "~/components/type-table";
import { Video } from "~/components/video";
import { baseOptions } from "~/layout-config";
import { source } from "~/source";
import browserCollections from "../../.source/browser";
import type { Route } from "./+types/page";

const components = {
  ...defaultMdxComponents,
  ...Twoslash,
  Hand,
  BookOpen,
  FileCode2,
  Wrench,
  ArrowBigRight,
  Shovel,
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

export function loader({ params }: Route.LoaderArgs) {
  const slugs = params["*"].split("/").filter((v) => v.length > 0);
  const page = source.getPage(slugs);

  if (!page) throw redirect("/docs");

  return {
    path: page.path,
    url: page.url,
    tree: source.pageTree,
    lastModified: page.data.lastModified,
    slugs,
  };
}

const clientLoader = browserCollections.docs.createClientLoader({
  component(
    { default: Mdx, toc, frontmatter },
    {
      tree,
      url,
      slugs,
      path,
      lastModified,
    }: {
      tree: PageTreeRoot;
      url: string;
      slugs: string[];
      path: string;
      lastModified: Date | undefined;
    },
  ) {
    const title = `${frontmatter.title} - Takumi`;

    const og = ["https://takumi.kane.tw/og", "docs", ...slugs, "image.webp"].join("/");

    return (
      <DocsPage
        toc={toc}
        tableOfContent={{
          style: "clerk",
        }}
        lastUpdate={lastModified}
        editOnGithub={{
          owner: "kane50613",
          repo: "takumi",
          sha: "master",
          path: `/docs/content/docs/${path}?plain=1`,
        }}
      >
        <title>{title}</title>
        <meta name="description" content={frontmatter.description} />
        <meta property="og:title" content={title} />
        <meta property="og:description" content={frontmatter.description} />
        <meta property="og:image" content={og} />
        <meta name="twitter:image" content={og} />
        <DocsTitle>{frontmatter.title}</DocsTitle>
        <DocsDescription>{frontmatter.description}</DocsDescription>
        <DocsBody>
          <Mdx components={components} />
          {frontmatter.index ? <DocsCategory tree={tree} url={url} /> : null}
        </DocsBody>
      </DocsPage>
    );
  },
});

export default function Page(props: Route.ComponentProps) {
  const { tree, path, url, slugs, lastModified } = props.loaderData;

  const Content = clientLoader.getComponent(path);

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
      tree={tree as PageTreeRoot}
    >
      <Content
        tree={tree as PageTreeRoot}
        url={url}
        slugs={slugs}
        path={path}
        lastModified={lastModified}
      />
    </DocsLayout>
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
