import { OG_IMAGE, SITE_NAME, SITE_URL } from "~/layout-config";

const HOME_URL = `${SITE_URL}/`;

// https://developers.google.com/search/docs/appearance/site-names
export function SiteJsonLd() {
  const data = {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "WebSite",
        "@id": `${HOME_URL}#website`,
        name: SITE_NAME,
        url: HOME_URL,
        publisher: { "@id": `${HOME_URL}#organization` },
      },
      {
        "@type": "Organization",
        "@id": `${HOME_URL}#organization`,
        name: SITE_NAME,
        url: HOME_URL,
        logo: `${SITE_URL}/logo.svg`,
        sameAs: ["https://github.com/kane50613/takumi", "https://x.com/kanewang_"],
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
