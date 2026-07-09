import { Analytics } from "@vercel/analytics/react";
import type { ReactNode } from "react";
import { Provider } from "../components/provider";
import "../../app/app.css";

export default function RootElement({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:image:width" content="1200" />
        <meta name="twitter:image:height" content="630" />
        <meta name="twitter:creator" content="@kanewang_" />
        <meta name="twitter:site" content="@kanewang_" />
        <meta property="og:site_name" content="Takumi" />
        <meta property="og:type" content="website" />
        <link rel="icon" type="image/svg+xml" href="/logo.svg" />
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
        <link
          rel="stylesheet"
          href="https://fonts.googleapis.com/css2?family=Noto+Serif:ital,wght@0,400..800;1,400..800&family=Geist+Mono:wght@400&display=swap"
        />
      </head>
      <body className="flex flex-col min-h-screen">
        <Provider>{children}</Provider>
        <Analytics />
      </body>
    </html>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
