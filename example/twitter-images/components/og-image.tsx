import { readFile } from "node:fs/promises";
import { Box, Brain, Globe, type LucideIcon, Zap } from "lucide-react";
import { createElement } from "react";

const accentColor = "#ff3535";
const cardBg = "rgba(255, 255, 255, 0.03)";
const borderColor = "rgba(255, 255, 255, 0.08)";

const spacing = {
  outer: "3rem",
  card: "2.5rem",
  cardCompact: "2.25rem 2.5rem",
  large: "2rem",
  gap: "1.5rem",
  compact: "0.75rem",
};

export const persistentImages = [
  {
    src: "takumi.svg",
    data: await readFile("../../assets/images/takumi.svg"),
  },
];

export const name = "og-image";

export const width = 1280;
export const height = 640;

export const fonts = ["geist/Geist[wght].woff2"];

export default function OgImage() {
  return (
    <div
      style={{
        backgroundColor: "#09090b",
        width: "100%",
        height: "100%",
        fontFamily: "Geist, sans-serif",
        display: "flex",
        flexDirection: "column",
        padding: spacing.outer,
        color: "white",
        gap: spacing.gap,
        position: "relative",
      }}
    >
      {/* Decorative Gradients */}
      <div
        style={{
          position: "absolute",
          top: -200,
          right: -200,
          width: 800,
          height: 800,
          backgroundImage: "radial-gradient(circle, rgba(255, 53, 53, 0.08) 0%, transparent 70%)",
        }}
      />
      <div
        style={{
          position: "absolute",
          bottom: -200,
          left: -200,
          width: 600,
          height: 600,
          backgroundImage: "radial-gradient(circle, rgba(255, 53, 53, 0.05) 0%, transparent 70%)",
        }}
      />

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1.15fr 1fr",
          gap: spacing.gap,
          flexGrow: 1,
        }}
      >
        {/* Hero Card */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            backgroundColor: cardBg,
            border: `1px solid ${borderColor}`,
            borderRadius: "2.5rem",
            padding: spacing.card,
            gap: "3rem",
            position: "relative",
            justifyContent: "space-between",
          }}
        >
          <img
            src={persistentImages[0]?.src}
            alt="Takumi logo"
            style={{
              width: "5rem",
              height: "5rem",
              flexShrink: 0,
            }}
          />
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: spacing.compact,
            }}
          >
            <h1
              style={{
                fontSize: "5rem",
                fontWeight: 800,
                margin: 0,
                letterSpacing: "-0.04em",
                lineHeight: 1,
                whiteSpace: "nowrap",
              }}
            >
              Takumi
            </h1>
            <p
              style={{
                fontSize: "2rem",
                color: "rgba(255, 255, 255, 0.5)",
                fontWeight: 500,
                maxWidth: "420px",
                margin: 0,
                lineHeight: 1.4,
              }}
            >
              Turn JSX into production-ready PNG, GIF, Video fast.
            </p>
          </div>

          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: spacing.compact,
              backgroundColor: "rgba(255, 53, 53, 0.1)",
              padding: "0.75rem 1.25rem",
              borderRadius: "100px",
              border: "1px solid rgba(255, 53, 53, 0.2)",
              alignSelf: "flex-start",
            }}
          >
            <div
              style={{
                width: 8,
                height: 8,
                borderRadius: "50%",
                backgroundColor: accentColor,
              }}
            />
            <span
              style={{
                fontSize: "1.125rem",
                fontWeight: 600,
                color: accentColor,
                letterSpacing: "0.02em",
              }}
            >
              Built for Developers
            </span>
          </div>
        </div>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 1fr",
            gridTemplateRows: "1fr 1fr",
            gap: spacing.gap,
          }}
        >
          <Feature
            title="Direct Rendering"
            description="No SVG conversion step needed."
            icon={Box}
          />
          <Feature
            title="Runs at Native Speed"
            description="Rust engine for Node and WASM."
            icon={Zap}
          />
          <Feature title="Runs Everywhere" description="Node.js, Browser, and Edge." icon={Globe} />
          <Feature
            title="Output to Any Format"
            description="WebP, PNG, JPEG, and GIF."
            icon={Brain}
          />
        </div>
      </div>
    </div>
  );
}

function Feature({
  title,
  description,
  icon,
}: {
  title: string;
  description: string;
  icon: LucideIcon;
}) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        backgroundColor: cardBg,
        border: `1px solid ${borderColor}`,
        borderRadius: "2rem",
        padding: spacing.cardCompact,
        gap: spacing.compact,
        justifyContent: "space-between",
        textWrap: "balance",
      }}
    >
      <div
        style={{
          width: "3rem",
          height: "3rem",
          backgroundColor: "rgba(255, 53, 53, 0.15)",
          borderRadius: "1rem",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: accentColor,
          flexShrink: 0,
        }}
      >
        {createElement(icon, {
          size: 24,
          strokeWidth: 2,
        })}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "0.45rem" }}>
        <span
          style={{
            fontSize: "1.35rem",
            fontWeight: 700,
          }}
        >
          {title}
        </span>
        <span
          style={{
            fontSize: "1.125rem",
            color: "rgba(255, 255, 255, 0.7)",
            fontWeight: 500,
            lineHeight: 1.3,
          }}
        >
          {description}
        </span>
      </div>
    </div>
  );
}
