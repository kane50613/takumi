import { readFile } from "node:fs/promises";
import { Zap, Globe, Sparkles } from "lucide-react";

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
        src={persistentImages[0]?.src}
        alt=""
        style={{
          position: "absolute",
          width: "1200px",
          height: "1200px",
          opacity: 0.02,
          right: "-300px",
          top: "-300px",
          transform: "rotate(-15deg)",
          pointerEvents: "none",
        }}
      />

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
          alignItems: "flex-start",
          width: "100%",
          maxWidth: "1000px",
          position: "relative",
          zIndex: 1,
        }}
      >
        <div
          style={{ display: "flex", alignItems: "center", gap: "1.5rem", marginBottom: "2.5rem" }}
        >
          <img
            src={persistentImages[0]?.src}
            alt="Takumi"
            style={{
              width: "5.5rem",
              height: "5.5rem",
            }}
          />
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
            fontWeight: 400,
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
          <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
            <Zap size={22} color="#ff3535" strokeWidth={2.5} />
            <span>Native Speed</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
            <Globe size={22} color="#ff3535" strokeWidth={2.5} />
            <span>Runs Everywhere</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
            <Sparkles size={22} color="#ff3535" strokeWidth={2.5} />
            <span>Multiple Formats</span>
          </div>
        </div>
      </div>
    </div>
  );
}
