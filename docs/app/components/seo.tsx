import { OG_IMAGE, SITE_NAME, SITE_URL } from "~/layout-config";

const HOME_URL = `${SITE_URL}/`;

const DESCRIPTION =
  "Takumi renders JSX, HTML, and node trees into images, SVG, and PDF from Rust, without a headless browser.";

const REPOSITORY = "https://github.com/kane50613/takumi";

// https://developers.google.com/search/docs/appearance/site-names
export function SiteJsonLd() {
  const data = {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "SoftwareApplication",
        "@id": `${HOME_URL}#software`,
        name: SITE_NAME,
        description: DESCRIPTION,
        url: HOME_URL,
        applicationCategory: "DeveloperApplication",
        applicationSubCategory: "Image and PDF rendering engine",
        operatingSystem: "Linux, macOS, Windows, Node.js, Bun, Deno, Cloudflare Workers, browsers",
        softwareRequirements: "Node.js 20+, or any WebAssembly runtime",
        license: [
          "https://opensource.org/licenses/MIT",
          "https://opensource.org/licenses/Apache-2.0",
        ],
        codeRepository: REPOSITORY,
        downloadUrl: "https://www.npmjs.com/package/takumi-js",
        image: `${SITE_URL}/logo.svg`,
        offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
        author: { "@id": `${HOME_URL}#organization` },
        publisher: { "@id": `${HOME_URL}#organization` },
      },
      {
        "@type": "WebSite",
        "@id": `${HOME_URL}#website`,
        name: SITE_NAME,
        description: DESCRIPTION,
        url: HOME_URL,
        publisher: { "@id": `${HOME_URL}#organization` },
      },
      {
        "@type": "Organization",
        "@id": `${HOME_URL}#organization`,
        name: SITE_NAME,
        description:
          "Takumi is the open source project behind the Takumi rendering engine, maintained by Kane Wang in Taiwan.",
        url: HOME_URL,
        logo: `${SITE_URL}/logo.svg`,
        email: "me@kane.tw",
        founder: { "@type": "Person", name: "Kane Wang", url: "https://github.com/kane50613" },
        address: {
          "@type": "PostalAddress",
          addressLocality: "Taipei",
          addressCountry: "TW",
        },
        contactPoint: [
          {
            "@type": "ContactPoint",
            contactType: "technical support",
            email: "me@kane.tw",
            url: `${REPOSITORY}/issues`,
            availableLanguage: ["English", "Chinese"],
          },
          {
            "@type": "ContactPoint",
            contactType: "security",
            email: "me@kane.tw",
            availableLanguage: ["English", "Chinese"],
          },
        ],
        sameAs: [REPOSITORY, "https://x.com/kanewang_"],
      },
    ],
  };

  return (
    <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(data) }} />
  );
}

export function Seo({
  title,
  description,
  path,
  image = OG_IMAGE,
}: {
  title: string;
  description: string | undefined;
  path: string;
  image?: string;
}) {
  const url = path ? `${SITE_URL}${path}` : HOME_URL;
  return (
    <>
      <title>{title}</title>
      <meta property="og:title" content={title} />
      <meta property="og:image" content={image} />
      <meta property="og:image:width" content="1200" />
      <meta property="og:image:height" content="630" />
      <meta property="og:image:alt" content={`${title} — ${SITE_NAME}`} />
      {description && (
        <>
          <meta name="description" content={description} />
          <meta property="og:description" content={description} />
        </>
      )}
      <meta property="og:url" content={url} />
      <meta name="twitter:image" content={image} />
      <meta name="twitter:url" content={url} />
      <link rel="canonical" href={url} />
    </>
  );
}
