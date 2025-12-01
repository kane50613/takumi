import type { ReactNode } from "react";

export default function DocsTemplateV1({
  title,
  description,
  icon,
  primaryColor,
  primaryTextColor,
  site,
}: {
  title: ReactNode;
  description: ReactNode;
  icon: ReactNode;
  primaryColor: string;
  primaryTextColor: string;
  site: ReactNode;
}) {
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        backgroundColor: "#09090b",
        position: "relative",
        display: "flex",
      }}
    >
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          width: "100%",
          height: "100%",
          backgroundImage: `radial-gradient(circle at 0% 0%, ${primaryColor}, transparent)`,
          opacity: 0.8,
        }}
      />

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          width: "100%",
          height: "100%",
          padding: "4rem",
          color: "white",
          position: "relative",
          flex: 1,
          justifyContent: "space-between",
        }}
      >
        <span
          style={{
            fontSize: 84,
            fontWeight: 700,
            lineHeight: 1.1,
            letterSpacing: "-0.03em",
            color: "white",
            textOverflow: "ellipsis",
            lineClamp: 2,
          }}
        >
          {title}
        </span>
        <span
          style={{
            fontSize: 48,
            color: "rgba(161, 161, 170, 1)",
            fontWeight: 400,
            lineHeight: 1.4,
            lineClamp: 2,
            textOverflow: "ellipsis",
            maxWidth: "90%",
          }}
        >
          {description}
        </span>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "1rem",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: primaryTextColor,
            }}
          >
            {icon}
          </div>
          <span
            style={{
              fontSize: 40,
              fontWeight: 600,
              letterSpacing: "-0.02em",
              color: "white",
              opacity: 0.9,
            }}
          >
            {site}
          </span>
        </div>
      </div>
    </div>
  );
}
