import animatedShowcase from "./templates/animated-showcase?raw";
import articleCover from "./templates/article-cover?raw";
import gradientPoster from "./templates/gradient-poster?raw";
import invoice from "./templates/invoice?raw";
import receipt from "./templates/receipt?raw";
import report from "./templates/report?raw";
import twitterProfileCard from "./templates/twitter-profile-card?raw";
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
    description: "Edit the code, watch the image re-render",
    kind: "image",
    code: welcome,
  },
  {
    id: "twitter-profile-card",
    name: "Profile card",
    description: "Avatar, handle and stats in a 1200 × 630 card",
    kind: "image",
    code: twitterProfileCard,
  },
  {
    id: "article-cover",
    name: "Article cover",
    description: "Blog header with a gradient background and a byline",
    kind: "image",
    code: articleCover,
  },
  {
    id: "gradient-poster",
    name: "Gradient poster",
    description: "Layered gradients and blend modes",
    kind: "image",
    code: gradientPoster,
  },
  {
    id: "keyframe-animation",
    name: "Keyframe loop",
    description: "CSS keyframes sampled into an animated WebP",
    kind: "animation",
    code: animatedShowcase,
  },
  {
    id: "invoice",
    name: "Invoice",
    description: "A4 page in PDF/A-3b, with the invoice XML attached",
    kind: "pdf",
    code: invoice,
  },
  {
    id: "report",
    name: "Report",
    description: "Multi-page flow with a repeating footer and bookmarks",
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
