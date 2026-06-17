export const name = "500-stars";

export const width = 1200;
export const height = 675;

export const fonts = ["geist/Geist[wght].woff2"];

export const images = [{ src: "takumi.svg", path: "takumi.svg" }];

const StarIcon = ({
  size,
  opacity,
  top,
  left,
  rotate,
}: {
  size: number;
  opacity: number;
  top: string;
  left: string;
  rotate: string;
}) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="currentColor"
    style={{
      position: "absolute",
      top,
      left,
      opacity,
      transform: `rotate(${rotate})`,
      color: "#E3B341",
    }}
  >
    <title>Star</title>
    <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
  </svg>
);

export default function FiveHundredStars() {
  return (
    <div
      style={{
        backgroundColor: "#09090b",
        backgroundImage: `
          radial-gradient(circle at 50% 115%, rgba(227, 179, 65, 0.12) 0%, rgba(227, 179, 65, 0) 60%),
          radial-gradient(circle at 5% -5%, rgba(65, 120, 227, 0.05) 0%, rgba(65, 120, 227, 0) 50%)
        `,
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        position: "relative",
        fontFamily: "Geist, sans-serif",
        overflow: "hidden",
      }}
    >
      {/* Decorative Elements */}
      <StarIcon size={32} opacity={0.5} top="12%" left="18%" rotate="0deg" />
      <StarIcon size={24} opacity={0.4} top="28%" left="80%" rotate="0deg" />
      <StarIcon size={48} opacity={0.3} top="72%" left="10%" rotate="0deg" />
      <StarIcon size={28} opacity={0.45} top="65%" left="88%" rotate="0deg" />

      {/* Content Container */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          zIndex: 10,
        }}
      >
        {/* Logo */}
        <div style={{ marginBottom: "3rem" }}>
          <img
            src="takumi.svg"
            alt="Takumi Logo"
            style={{
              width: "6rem",
              height: "6rem",
            }}
          />
        </div>

        {/* Hero Text Section */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: "0.25rem",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "baseline",
            }}
          >
            <span
              style={{
                color: "white",
                fontSize: "14rem",
                fontWeight: 900,
                letterSpacing: "-0.04em",
                lineHeight: 0.8,
              }}
            >
              500
            </span>
          </div>

          <span
            style={{
              color: "#E3B341",
              fontSize: "3.25rem",
              fontWeight: 700,
              letterSpacing: "0.45em",
              marginLeft: "0.45em",
              textTransform: "uppercase",
              opacity: 0.95,
            }}
          >
            Stars
          </span>
        </div>
      </div>
    </div>
  );
}
