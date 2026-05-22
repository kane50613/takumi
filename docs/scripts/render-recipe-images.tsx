/**
 * Renders preview images referenced from docs/content/docs/recipes/*.mdx
 * into docs/public/recipes/<slug>.webp.
 *
 * Run with: cd docs && bun scripts/render-recipe-images.tsx
 */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createElement, type ReactElement } from "react";
import { Renderer } from "takumi-js/node";
import { fromJsx } from "takumi-js/helpers/jsx";
import { Code, GitBranch, Globe, Heart, Scale, Sparkles, Star, Zap } from "lucide-react";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");
const outDir = resolve(scriptDir, "../public/recipes");

const fontDir = join(repoRoot, "assets/fonts");
const fonts = await Promise.all(
  [
    "geist/Geist[wght].woff2",
    "geist/GeistMono[wght].woff2",
    "twemoji/TwemojiMozilla-colr.woff2",
  ].map((rel) => readFile(join(fontDir, rel))),
);

const persistentImages = [
  { src: "takumi.svg", data: await readFile(join(repoRoot, "assets/images/takumi.svg")) },
  { src: "logo.svg", data: await readFile(join(repoRoot, "docs/public/logo.svg")) },
  { src: "fuma.jpg", data: await readFile(join(repoRoot, "assets/images/fuma.jpg")) },
  { src: "large.jpg", data: await readFile(join(repoRoot, "assets/images/fumadocs-core-v16.jpg")) },
  {
    src: "product.jpg",
    data: await readFile(join(repoRoot, "assets/images/martin-martz-W0NRebXbsjM-unsplash.jpg")),
  },
];

const renderer = new Renderer({ fonts, persistentImages });

interface Preview {
  slug: string;
  width: number;
  height: number;
  jsx: ReactElement;
}

const OgImageJsx = (
  <div
    style={{
      backgroundColor: "#fcfcfc",
      backgroundImage: "radial-gradient(#e5e5e5 1px, transparent 1px)",
      backgroundSize: "32px 32px",
      width: "100%",
      height: "100%",
      fontFamily: "Geist, sans-serif",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      color: "#171717",
      position: "relative",
      padding: "4rem",
    }}
  >
    <img
      src="takumi.svg"
      alt=""
      style={{
        position: "absolute",
        width: "1200px",
        height: "1200px",
        opacity: 0.02,
        right: "-300px",
        top: "-300px",
        transform: "rotate(-15deg)",
      }}
    />
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "flex-start",
        width: "100%",
        maxWidth: "1000px",
        position: "relative",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "1.5rem", marginBottom: "2.5rem" }}>
        <img src="takumi.svg" alt="Takumi" style={{ width: "5.5rem", height: "5.5rem" }} />
        <h1
          style={{
            fontSize: "6.5rem",
            fontWeight: 800,
            margin: 0,
            letterSpacing: "-0.04em",
            lineHeight: 1,
            color: "#111111",
          }}
        >
          Takumi
        </h1>
      </div>
      <p
        style={{
          fontSize: "2.5rem",
          color: "#4a4a4a",
          maxWidth: "920px",
          margin: 0,
          marginBottom: "4rem",
          lineHeight: 1.35,
          letterSpacing: "-0.015em",
        }}
      >
        A Rust rendering engine for turning templates into images, with next/og-compatible APIs.
      </p>
      <div
        style={{
          display: "flex",
          gap: "2.5rem",
          alignItems: "center",
          color: "#555555",
          fontSize: "1.25rem",
          fontWeight: 600,
          letterSpacing: "0.06em",
          textTransform: "uppercase",
        }}
      >
        {[
          { Icon: Zap, label: "Native Speed" },
          { Icon: Globe, label: "Runs Everywhere" },
          { Icon: Sparkles, label: "Multiple Formats" },
        ].map(({ Icon, label }) => (
          <div key={label} style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
            {createElement(Icon, { size: 22, color: "#ff3535", strokeWidth: 2.5 })}
            <span>{label}</span>
          </div>
        ))}
      </div>
    </div>
  </div>
);

const XCardJsx = (
  <div
    style={{
      display: "flex",
      backgroundColor: "black",
      width: "100%",
      height: "100%",
      flexDirection: "column",
      padding: "3rem",
      paddingBottom: 0,
    }}
  >
    <div style={{ display: "flex", marginBottom: "2rem", gap: "2rem", alignItems: "center" }}>
      <img
        src="fuma.jpg"
        alt="Fuma Nama"
        style={{ width: 120, height: 120, borderRadius: "50%" }}
      />
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          fontSize: "3rem",
          flexGrow: 1,
          gap: "0.5rem",
        }}
      >
        <span style={{ color: "white", fontWeight: 700 }}>Fuma Nama</span>
        <span style={{ color: "gray", fontWeight: 300 }}>@fuma_nama</span>
      </div>
      <img src="takumi.svg" alt="Takumi" style={{ width: 64, height: 64 }} />
    </div>
    <span
      style={{
        display: "flex",
        fontSize: "4rem",
        color: "white",
        fontWeight: 300,
        marginBottom: "1rem",
      }}
    >
      My favourite part of the year
    </span>
    <div style={{ display: "flex", width: "100%", flexGrow: 1 }}>
      <img
        src="large.jpg"
        alt="content"
        style={{ width: "100%", borderRadius: "2rem", border: "2px solid dimgray" }}
      />
    </div>
    <div
      style={{
        display: "flex",
        position: "absolute",
        width: "100%",
        height: "50%",
        bottom: 0,
        backgroundImage: "linear-gradient(to top, black, transparent)",
      }}
    />
  </div>
);

const ProductCardJsx = (
  <div
    style={{
      display: "flex",
      width: "100%",
      height: "100%",
      backgroundColor: "#f1f5f9",
      padding: "40px",
      alignItems: "center",
      justifyContent: "center",
      fontFamily: "Geist",
    }}
  >
    <div
      style={{
        display: "flex",
        width: "100%",
        height: "100%",
        backgroundColor: "white",
        borderRadius: "32px",
        overflow: "hidden",
        boxShadow: "0 20px 50px rgba(0,0,0,0.08)",
      }}
    >
      <div
        style={{
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          backgroundImage: "linear-gradient(to bottom, #f8fafc, #f1f5f9)",
          padding: "40px",
        }}
      >
        <img
          src="product.jpg"
          alt="product"
          style={{ width: "100%", height: "100%", borderRadius: "24px", objectFit: "cover" }}
        />
      </div>
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          padding: "50px 60px",
          justifyContent: "center",
        }}
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <span
            style={{
              fontSize: 22,
              color: "#2563eb",
              fontWeight: 700,
              textTransform: "uppercase",
              letterSpacing: "0.1em",
            }}
          >
            Northwind
          </span>
          <span
            style={{
              fontSize: 60,
              fontWeight: 900,
              color: "#0f172a",
              lineHeight: 1.1,
              letterSpacing: "-0.02em",
            }}
          >
            Field Camera
          </span>
        </div>
        <span
          style={{
            fontSize: 28,
            color: "#475569",
            lineHeight: 1.5,
            marginTop: "24px",
            marginBottom: "32px",
          }}
        >
          Pocket-sized, weather-sealed, and just enough automation to stay out of your way.
        </span>
        <div style={{ display: "flex", fontSize: 52, fontWeight: 800, color: "#2563eb" }}>$349</div>
      </div>
    </div>
  </div>
);

const BlogCardJsx = (
  <div
    style={{
      display: "flex",
      flexDirection: "column",
      width: "100%",
      height: "100%",
      color: "white",
      backgroundImage: "linear-gradient(135deg, #1a1a1a 0%, #000 100%)",
      padding: "60px",
      justifyContent: "space-between",
      fontFamily: "Geist",
    }}
  >
    <div style={{ display: "flex" }}>
      <div
        style={{
          display: "flex",
          backgroundColor: "#3b82f6",
          padding: "8px 24px",
          borderRadius: "9999px",
          fontSize: 24,
          fontWeight: 600,
        }}
      >
        Engineering
      </div>
    </div>
    <h1
      style={{
        fontSize: 80,
        fontWeight: 800,
        lineHeight: 1.1,
        margin: 0,
        textShadow: "0 4px 12px rgba(0,0,0,0.5)",
      }}
    >
      How we render OG images without a browser
    </h1>
    <div style={{ display: "flex", alignItems: "center", gap: "24px" }}>
      <div
        style={{
          display: "flex",
          width: 80,
          height: 80,
          borderRadius: "50%",
          overflow: "hidden",
          border: "4px solid rgba(255,255,255,0.1)",
        }}
      >
        <img src="fuma.jpg" alt="" style={{ width: 80, height: 80 }} />
      </div>
      <div style={{ display: "flex", flexDirection: "column" }}>
        <span style={{ fontSize: 32, fontWeight: 600 }}>Kane Wang</span>
        <span style={{ fontSize: 24, color: "#a1a1aa" }}>May 22, 2026</span>
      </div>
    </div>
  </div>
);

const demoSymbolRows = [
  { name: "Functions", kind: "section" as const },
  { name: "render", kind: "symbol" as const },
  { name: "loadFont", kind: "symbol" as const },
  { name: "renderAnimation", kind: "symbol" as const },
  { name: "Classes", kind: "section" as const },
  { name: "Renderer", kind: "symbol" as const },
  { name: "Interfaces", kind: "section" as const },
  { name: "RenderOptions", kind: "symbol" as const },
  { name: "Types", kind: "section" as const },
  { name: "CSSProperties", kind: "symbol" as const },
];

const PackageOgJsx = (
  <div
    style={{
      position: "relative",
      overflow: "hidden",
      width: "100%",
      height: "100%",
      backgroundColor: "#020617",
      color: "#e2e8f0",
      fontFamily: "Geist",
      display: "flex",
      flexDirection: "column",
      justifyContent: "center",
    }}
  >
    <div
      style={{
        position: "absolute",
        top: -40,
        left: 48,
        width: 700,
        height: 700,
        borderRadius: "50%",
        filter: "blur(64px)",
        backgroundColor: "rgba(226, 232, 240, 0.03)",
      }}
    />
    <div style={{ padding: "3.75rem", display: "flex", flexDirection: "column", gap: "3rem" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
        <img src="logo.svg" width={60} height={60} alt="logo" />
        <h1
          style={{
            fontSize: "2.75rem",
            margin: 0,
            letterSpacing: "-0.04em",
            fontFamily: "Geist Mono",
          }}
        >
          takumi.kane.tw
        </h1>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
        <div style={{ fontSize: "2.5rem", opacity: 0.5, fontFamily: "Geist Mono" }}>takumi-js</div>
        <div
          style={{
            fontSize: "4.75rem",
            letterSpacing: "-0.05em",
            fontFamily: "Geist Mono",
            lineHeight: 1,
          }}
        >
          helpers
        </div>
        <div
          style={{
            fontSize: "2.25rem",
            opacity: 0.7,
            paddingTop: "0.75rem",
            fontFamily: "Geist Mono",
          }}
        >
          v0.7.0
        </div>
      </div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "1.25rem",
          fontSize: "2rem",
          color: "rgba(226, 232, 240, 0.7)",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <GitBranch width={28} height={28} />
          <span>
            kane50613<span style={{ opacity: 0.5 }}>/</span>takumi
          </span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <Star width={28} height={28} fill="white" />
          <span>3.0K</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <Heart width={28} height={28} fill="white" />
          <span>841</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <Scale width={28} height={28} />
          <span>MIT</span>
        </div>
      </div>
    </div>
    <div
      style={{
        position: "absolute",
        right: "2rem",
        top: "2rem",
        bottom: "2rem",
        width: 340,
        display: "flex",
        flexDirection: "column",
        gap: 2,
        opacity: 0.3,
        fontSize: "1.25rem",
        fontFamily: "Geist Mono",
        color: "rgba(226, 232, 240, 0.95)",
      }}
    >
      {demoSymbolRows.map((row, i) => (
        <div
          key={i}
          style={{
            display: "flex",
            alignItems: "center",
            paddingLeft: row.kind === "symbol" ? "20px" : 0,
          }}
        >
          <Code width={18} height={18} />
          <span
            style={{
              marginLeft: "0.5rem",
              fontSize: row.kind === "section" ? "1.05rem" : "1.2rem",
            }}
          >
            {row.name}
          </span>
        </div>
      ))}
    </div>
  </div>
);

const SpinnerJsx = (
  <div tw="flex w-full h-full items-center justify-center" style={{ backgroundColor: "#0f172a" }}>
    <div
      tw="rounded-full"
      style={{
        width: 96,
        height: 96,
        border: "8px solid #334155",
        borderTopColor: "#6366f1",
      }}
    />
  </div>
);

const VideoJsx = (
  <div
    style={{
      width: "100%",
      height: "100%",
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      justifyContent: "center",
      gap: "2rem",
      backgroundColor: "#0b1020",
      backgroundImage:
        "radial-gradient(circle at 30% 30%, rgba(99,102,241,0.25), transparent 60%), radial-gradient(circle at 70% 70%, rgba(236,72,153,0.18), transparent 55%)",
      fontFamily: "Geist",
      color: "white",
    }}
  >
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        width: 160,
        height: 160,
        borderRadius: "50%",
        backgroundColor: "rgba(255,255,255,0.08)",
        border: "2px solid rgba(255,255,255,0.2)",
      }}
    >
      <div
        style={{
          width: 0,
          height: 0,
          borderTop: "32px solid transparent",
          borderBottom: "32px solid transparent",
          borderLeft: "52px solid white",
          marginLeft: 12,
        }}
      />
    </div>
    <div
      style={{
        fontSize: 60,
        fontWeight: 700,
        letterSpacing: "-0.03em",
        fontFamily: "Geist Mono",
      }}
    >
      render → ffmpeg
    </div>
    <div
      style={{
        display: "flex",
        gap: 8,
        fontFamily: "Geist Mono",
        fontSize: 24,
        opacity: 0.7,
      }}
    >
      <span>120 frames</span>
      <span>·</span>
      <span>1200×630</span>
      <span>·</span>
      <span>30 fps</span>
    </div>
  </div>
);

const GalleryJsx = (
  <div
    style={{
      width: "100%",
      height: "100%",
      display: "flex",
      flexDirection: "column",
      backgroundColor: "#0f172a",
      color: "white",
      fontFamily: "Geist",
      padding: 60,
      gap: 32,
    }}
  >
    <div
      style={{
        fontSize: 48,
        fontWeight: 800,
        letterSpacing: "-0.03em",
      }}
    >
      Template Gallery
    </div>
    <div style={{ display: "flex", flexWrap: "wrap", gap: 20 }}>
      {[
        "og-image",
        "x-post-image",
        "github-social-preview",
        "package-og-image",
        "prisma-og-image",
        "500-stars",
        "text-fit",
        "v1",
        "product-card",
        "blog-post",
      ].map((name) => (
        <div
          key={name}
          style={{
            display: "flex",
            padding: "16px 28px",
            borderRadius: 14,
            backgroundColor: "rgba(255,255,255,0.06)",
            border: "1px solid rgba(255,255,255,0.1)",
            fontFamily: "Geist Mono",
            fontSize: 28,
          }}
        >
          {name}
        </div>
      ))}
    </div>
  </div>
);

const previews: Preview[] = [
  { slug: "og-image", width: 1280, height: 640, jsx: OgImageJsx },
  { slug: "x-card", width: 1200, height: 630, jsx: XCardJsx },
  { slug: "product-card", width: 1200, height: 630, jsx: ProductCardJsx },
  { slug: "blog-card", width: 1200, height: 630, jsx: BlogCardJsx },
  { slug: "github-preview", width: 1280, height: 640, jsx: OgImageJsx },
  { slug: "package-og", width: 1200, height: 630, jsx: PackageOgJsx },
  { slug: "spinner", width: 400, height: 400, jsx: SpinnerJsx },
  { slug: "video", width: 1200, height: 630, jsx: VideoJsx },
  { slug: "template-gallery", width: 1200, height: 630, jsx: GalleryJsx },
];

await mkdir(outDir, { recursive: true });

for (const p of previews) {
  const { node, stylesheets } = await fromJsx(p.jsx);
  const buf = await renderer.render(node, {
    width: p.width,
    height: p.height,
    format: "webp",
    stylesheets,
  });
  const dest = join(outDir, `${p.slug}.webp`);
  await writeFile(dest, buf);
  console.log(`wrote ${dest} (${buf.length} bytes)`);
}
