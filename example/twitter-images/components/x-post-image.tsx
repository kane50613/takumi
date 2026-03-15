import { readFile } from "node:fs/promises";

export const name = "x-post-image";

export const width = 1200;
export const height = 630;

export const fonts = [];

export const persistentImages = [
  {
    src: "takumi.svg",
    data: await readFile("../../assets/images/takumi.svg"),
  },
  {
    src: "fuma.jpg",
    data: await readFile("../../assets/images/fuma.jpg"),
  },
  {
    src: "large.jpg",
    data: await readFile("../../assets/images/fumadocs-core-v16.jpg"),
  },
];

// https://x.com/kanewang_/status/1976314376102740338
export default function XPostImage() {
  return (
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
      <div
        style={{
          display: "flex",
          marginBottom: "2rem",
          gap: "2rem",
          alignItems: "center",
        }}
      >
        <img
          src="fuma.jpg"
          alt="Fuma Nama"
          style={{
            width: 120,
            height: 120,
            borderRadius: "50%",
          }}
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
          <span
            style={{
              color: "white",
              fontWeight: 700,
            }}
          >
            Fuma Nama
          </span>
          <span
            style={{
              marginTop: 0,
              color: "gray",
              fontWeight: 300,
            }}
          >
            @fuma_nama
          </span>
        </div>
        <img
          src="takumi.svg"
          alt="Takumi"
          style={{
            width: 64,
            height: 64,
          }}
        />
      </div>
      <span
        style={{
          display: "flex",
          lineClamp: 1,
          textOverflow: "ellipsis",
          fontSize: "4rem",
          color: "white",
          fontWeight: 300,
          marginBottom: "1rem",
        }}
      >
        My favourite part of the year
      </span>
      <div
        style={{
          display: "flex",
          width: "100%",
          flexGrow: 1,
        }}
      >
        <img
          src="large.jpg"
          style={{
            width: "100%",
            borderRadius: "2rem",
            border: "2px solid dimgray",
          }}
          alt="content"
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
}
