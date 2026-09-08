import { HomeLayout } from "fumadocs-ui/layouts/home";
import { LazyPlayground } from "~/components/playground/lazy-playground";
import { Seo } from "~/components/seo";
import { baseOptions } from "~/layout-config";

const TITLE = "Playground · Takumi";
const DESCRIPTION =
  "Try Takumi in your browser. Edit JSX templates and preview images, SVG, animations, and PDF documents with WebAssembly.";

export default function Playground() {
  return (
    <HomeLayout {...baseOptions}>
      <Seo title={TITLE} description={DESCRIPTION} path="/playground" />
      <LazyPlayground />
    </HomeLayout>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
