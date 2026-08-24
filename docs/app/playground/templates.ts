import animatedShowcase from "./templates/animated-showcase?raw";
import articleCover from "./templates/article-cover?raw";
import gradientPoster from "./templates/gradient-poster?raw";
import invoice from "./templates/invoice?raw";
import multilingual from "./templates/multilingual?raw";
import receipt from "./templates/receipt?raw";
import report from "./templates/report?raw";
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
    description: "Edit the code, press Run — the palette lives in options.variables",
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
    id: "gradient-poster",
    name: "Gradient poster",
    description: "Conic gradient, backdrop blur and a blend mode",
    kind: "image",
    code: gradientPoster,
  },
  {
    id: "keyframe-animation",
    name: "Keyframe loop",
    description: "Staggered Japanese text sampled into an animated WebP",
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
    name: "Report",
    description: "Four scripts across pages, with a counted footer and bookmarks",
    kind: "pdf",
    code: report,
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
