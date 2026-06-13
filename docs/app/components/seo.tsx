import { OG_IMAGE, SITE_URL } from "~/layout-config";

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
  const url = `${SITE_URL}${path}`;
  return (
    <>
      <title>{title}</title>
      <meta name="description" content={description} />
      <meta property="og:title" content={title} />
      <meta property="og:description" content={description} />
      <meta property="og:image" content={image} />
      <meta property="og:url" content={url} />
      <meta name="twitter:image" content={image} />
      <meta name="twitter:url" content={url} />
      <link rel="canonical" href={url} />
    </>
  );
}
