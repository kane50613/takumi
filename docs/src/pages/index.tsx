import { HomeLayout } from "fumadocs-ui/layouts/home";
import { Outputs } from "~/components/home/outputs";
import { Showcase } from "~/components/home/showcase";
import { Stage } from "~/components/home/stage";
import { Seo, SiteJsonLd } from "~/components/seo";
import { baseOptions } from "~/layout-config";

const TITLE = "Takumi: Craft JSX and CSS into images, animations, SVG, and PDF";
const DESCRIPTION =
  "Swing the hammer on JSX, HTML, and CSS components. Takumi renders them in Rust and WebAssembly into images, animations, SVG, paged PDFs, and paint data without a browser.";

const HOME_THEME_CSS = `
@font-face {
  font-family: "Anybody";
  src: url("/fonts/Anybody.woff2") format("woff2");
  font-weight: 100 900;
  font-stretch: 50% 150%;
  font-display: swap;
}
@font-face {
  font-family: "Instrument Sans";
  src: url("/fonts/InstrumentSans.woff2") format("woff2");
  font-weight: 400 700;
  font-stretch: 75% 100%;
  font-display: swap;
}
:root:has([data-stage]) {
  --font-sans: "Instrument Sans", system-ui, sans-serif;
  --background: #faf5ee;
  --foreground: #18130f;
  --primary: #c92d14;
  --primary-foreground: #ffffff;
  --muted: #efe7dc;
  --muted-foreground: #5c5046;
  --anvil: #2a211b;
  --wire: #1d4ed8;
  --border: rgba(24, 19, 15, 0.12);
  --ring: rgba(201, 45, 20, 0.35);
  --color-fd-background: var(--background);
  --color-fd-foreground: var(--foreground);
  --color-fd-primary: var(--primary);
  font-family: var(--font-sans);
}
:root:has([data-stage]) {
  scroll-snap-type: y proximity;
  scroll-padding-top: 3.5rem;
}
:root:has([data-stage]) #nd-nav > nav {
  border-bottom-color: transparent;
}
.dark:has([data-stage]) {
  --background: #14100d;
  --foreground: #f7f0e6;
  --primary: #d62f15;
  --primary-foreground: #ffffff;
  --muted: #211a15;
  --muted-foreground: #b8aa9b;
  --anvil: #4a3d33;
  --wire: #5b8def;
  --border: rgba(247, 240, 230, 0.12);
  --ring: rgba(255, 85, 43, 0.4);
  --color-fd-background: var(--background);
  --color-fd-foreground: var(--foreground);
  --color-fd-primary: var(--primary);
  font-family: var(--font-sans);
}
`;

export default function Home() {
  return (
    <HomeLayout className="overflow-x-clip font-sans" {...baseOptions}>
      <Seo title={TITLE} description={DESCRIPTION} path="" />
      <SiteJsonLd />
      <style>{HOME_THEME_CSS}</style>

      <Stage />
      <Outputs />
      <Showcase />
    </HomeLayout>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
