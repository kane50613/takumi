import { readFile } from "node:fs/promises";
import { Box, Brain, Globe, type LucideIcon, Zap } from "lucide-react";
import { createElement } from "react";

const secondaryForeground = "rgba(241, 245, 249, 0.75)";
const primaryForeground = "#F1F5F9";
const borderColor = "rgba(215, 29, 54, 0.5)";
const accentColor = "#ff3535";

export const persistentImages = [
  {
    src: "takumi.svg",
    data: await readFile("../../assets/images/takumi.svg"),
  },
];

export const name = "og-image";

export const width = 1280;
export const height = 640;

export const fonts = [
  "plus-jakarta-sans/PlusJakartaSans-VariableFont_wght.woff2",
];

export default function OgImage() {
  return (
    <div
      style={{
        backgroundImage:
          "linear-gradient(135deg, #100806 0%, #1b0a08 35%, #2a0c0a 65%, #360e0c 100%)",
        width: "100%",
        height: "100%",
        fontFamily: "Plus Jakarta Sans",
        display: "grid",
        gridTemplateColumns: "3fr 5fr",
      }}
    >
      <div
        style={{
          paddingLeft: "4rem",
          display: "flex",
          flexDirection: "column",
          borderRightWidth: 1,
          borderColor,
          borderStyle: "solid",
          justifyContent: "center",
          color: primaryForeground,
          gap: "2rem",
        }}
      >
        <img
          src={persistentImages[0]?.src}
          alt="Takumi logo"
          style={{
            width: "6rem",
          }}
        />
        <span style={{ fontSize: "4rem", fontWeight: 800 }}>Takumi</span>
        <span
          style={{
            fontSize: "2rem",
            fontWeight: 600,
            color: secondaryForeground,
            lineHeight: 1.5,
          }}
        >
          Render your React components to images.
        </span>
      </div>

      <div style={{ display: "flex", flexDirection: "column" }}>
        <div
          style={{
            display: "flex",
            justifyContent: "center",
            alignItems: "center",
            padding: "1rem",
            borderBottomWidth: 1,
            borderColor,
            borderStyle: "solid",
          }}
        >
          <span
            style={{
              color: accentColor,
              fontWeight: 800,
              fontSize: "1.75rem",
              padding: "0.5rem 1rem",
            }}
          >
            Build for Developers.
          </span>
        </div>
        <Features />
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            padding: "0.75rem",
          }}
        >
          <span
            style={{
              color: secondaryForeground,
              fontWeight: 600,
              fontSize: "1rem",
            }}
          >
            This image is rendered with Takumi.
          </span>
        </div>
      </div>
    </div>
  );
}

function Features() {
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(2, 1fr)",
        gridTemplateRows: "repeat(2, 1fr)",
        flexGrow: 1,
      }}
    >
      <Feature
        title="All in One"
        description="React code to image file in a single library."
        icon={Box}
        borderBottom
        borderRight
      />
      <Feature
        title="Speed is the Priority"
        description="Focus on speed to get your images ready."
        icon={Zap}
        borderBottom
      />
      <Feature
        title="Runs Everywhere"
        description="In the browser, Node.js, and Edge Runtime."
        icon={Globe}
        borderRight
        borderBottom
      />
      <Feature
        title="LLM Friendly"
        description="Documentation is ready for AI to use."
        icon={Brain}
        borderBottom
      />
    </div>
  );
}

function Feature({
  title,
  description,
  icon,
  borderBottom = false,
  borderRight = false,
}: {
  title: string;
  description: string;
  icon: LucideIcon;
  borderBottom?: boolean;
  borderRight?: boolean;
}) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        borderBottomWidth: Number(borderBottom),
        borderRightWidth: Number(borderRight),
        borderColor,
        borderStyle: "solid",
        padding: "2rem",
        gap: "1rem",
        justifyContent: "center",
      }}
    >
      <div
        style={{
          display: "flex",
          gap: "0.5rem",
          color: secondaryForeground,
          alignItems: "center",
        }}
      >
        {createElement(icon, {
          color: accentColor,
          strokeWidth: 2.5,
          width: 24,
          height: 24,
        })}
        <span style={{ fontSize: "1.5rem", fontWeight: 600 }}>{title}</span>
      </div>
      <span
        style={{
          fontSize: "1.75rem",
          color: primaryForeground,
          fontWeight: 600,
          lineHeight: 1.5,
        }}
      >
        {description}
      </span>
    </div>
  );
}
