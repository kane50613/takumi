import { HomeLayout } from "fumadocs-ui/layouts/home";
import { Live } from "~/components/home/live";
import { Outputs } from "~/components/home/outputs";
import { Showcase } from "~/components/home/showcase";
import { Stage } from "~/components/home/stage";
import { Seo, SiteJsonLd } from "~/components/seo";
import { baseOptions } from "~/layout-config";

const TITLE = "Takumi: Open Graph images and PDFs from JSX";
const DESCRIPTION =
  "Generate Open Graph images, social cards, and PDF documents from JSX, HTML, and CSS. Render with Rust and WebAssembly without a headless browser.";

export default function Home() {
  return (
    <HomeLayout className="overflow-x-clip" {...baseOptions}>
      <Seo title={TITLE} description={DESCRIPTION} path="" />
      <SiteJsonLd />

      <Stage />
      <Outputs />
      <Showcase />
      <Live />
    </HomeLayout>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
