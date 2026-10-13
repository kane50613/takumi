import { HomeLayout } from "fumadocs-ui/layouts/home";
import { CTA } from "~/components/home/cta";
import { Ecosystem } from "~/components/home/ecosystem";
import { Showcase } from "~/components/home/showcase";
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

      <Ecosystem />
      <Showcase />
      <CTA />
    </HomeLayout>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
