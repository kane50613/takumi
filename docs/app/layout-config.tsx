import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

export const SITE_URL = "https://takumi.kane.tw";

// docs/public/images/twitter-images symlinks example/twitter-images/output.
export const OG_IMAGE = `${SITE_URL}/images/twitter-images/og-image.png`;

export const SITE_NAME = "Takumi";

export const baseOptions: BaseLayoutProps = {
  githubUrl: "https://github.com/kane50613/takumi",
  nav: {
    title: <img src="/sticker.svg" alt="Takumi" className="h-8 w-fit" height={210} width={530} />,
  },
  links: [
    {
      text: "Documentation",
      url: "/docs",
      active: "nested-url",
    },
    {
      text: "Playground",
      url: "/playground",
    },
    {
      text: "Showcase",
      url: "/showcase",
    },
    {
      text: "For LLMs",
      type: "menu",
      items: [
        {
          text: "llms.txt",
          url: "/llms.txt",
          description: "Outline of the documentation",
          external: true,
        },
        {
          text: "llms-full.txt",
          url: "/llms-full.txt",
          description: "Full text of the documentation",
          external: true,
        },
      ],
    },
  ],
};
