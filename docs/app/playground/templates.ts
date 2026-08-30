import animatedShowcase from "./templates/animated-showcase?raw";
import accessible from "./templates/accessible?raw";
import analyticsChart from "./templates/analytics-chart?raw";
import articleCover from "./templates/article-cover?raw";
import gradientPoster from "./templates/gradient-poster?raw";
import invoice from "./templates/invoice?raw";
import multilingual from "./templates/multilingual?raw";
import receipt from "./templates/receipt?raw";
import report from "./templates/report?raw";
import svgText from "./templates/svg-text?raw";
import themeTokens from "./templates/theme-tokens?raw";
import twitterProfileCard from "./templates/twitter-profile-card?raw";
import watermark from "./templates/watermark?raw";
import welcome from "./templates/welcome?raw";
import type { OutputKind } from "./schema";

export type Template = {
  id: string;
  name: string;
  description: string;
  kind: OutputKind;
  code: string;
};

export const templates: Template[] = [
  {
    id: "welcome",
    name: "Welcome",
    description: "Edit the code, press Run",
    kind: "image",
    code: welcome,
  },
  {
    id: "multilingual",
    name: "Every script",
    description: "Eight writing systems, including right-to-left",
    kind: "image",
    code: multilingual,
  },
  {
    id: "twitter-profile-card",
    name: "Profile card",
    description: "One accent variable skins the border, handle and avatar ring",
    kind: "image",
    code: twitterProfileCard,
  },
  {
    id: "article-cover",
    name: "Article cover",
    description: "Blog header with a gradient background, themed by a brand scale",
    kind: "image",
    code: articleCover,
  },
  {
    id: "theme-tokens",
    name: "Theme tokens",
    description: "A Tailwind @theme block drives the colours, spacing, radius and display size",
    kind: "image",
    code: themeTokens,
  },
  {
    id: "gradient-poster",
    name: "Gradient poster",
    description: "Conic gradient, backdrop blur and a blend mode",
    kind: "image",
    code: gradientPoster,
  },
  {
    id: "svg-text",
    name: "SVG text",
    description: "An SVG source whose <text> and textPath draw from the registered fonts",
    kind: "image",
    code: svgText,
  },
  {
    id: "analytics-chart",
    name: "Analytics chart",
    description: "An ECharts SVG rendered off-DOM, passed straight to an img src",
    kind: "image",
    code: analyticsChart,
  },
  {
    id: "keyframe-animation",
    name: "Keyframe loop",
    description: "Staggered keyframes sampled into an animated WebP",
    kind: "animation",
    code: animatedShowcase,
  },
  {
    id: "invoice",
    name: "Invoice",
    description: "Bilingual A4 page in PDF/A-3b, with the invoice XML attached",
    kind: "pdf",
    code: invoice,
  },
  {
    id: "watermark",
    name: "Watermark",
    description: "A fixed box under the text, repeated on every page",
    kind: "pdf",
    code: watermark,
  },
  {
    id: "report",
    name: "Paged table",
    description: "An ECharts line chart above a long table whose header repeats on every page",
    kind: "pdf",
    code: report,
  },
  {
    id: "accessible",
    name: "Tagged pages",
    description: "PDF/A-4 with PDF/UA-2 tagging, bookmarks from the headings",
    kind: "pdf",
    code: accessible,
  },
  {
    id: "receipt",
    name: "Receipt",
    description: "One page sized to its content, like a receipt roll",
    kind: "pdf",
    code: receipt,
  },
];

export const defaultTemplate = templates[0].code;
