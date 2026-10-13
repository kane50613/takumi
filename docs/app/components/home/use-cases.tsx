import { cn } from "~/lib/utils";

const OUTPUT = "/images/twitter-images";
const THUMBNAIL = "/images/home";

type UseCase = {
  name: string;
  format: string;
  /** The rendered file the tile opens. */
  file: string;
  image: string;
  width: number;
  height: number;
  alt: string;
  /** Moving, so it hides when the reader asks for reduced motion. */
  animated?: boolean;
};

const rows: UseCase[][] = [
  [
    {
      name: "Blog cover",
      format: "PNG",
      file: `${OUTPUT}/home-usecase-blog-cover.png`,
      image: `${THUMBNAIL}/home-usecase-blog-cover.webp`,
      width: 1037,
      height: 544,
      alt: "A blog cover with a serif headline and a lighthouse at night.",
    },
    {
      name: "Conference pass",
      format: "PDF",
      file: `${OUTPUT}/home-usecase-ticket.pdf`,
      image: `${THUMBNAIL}/home-usecase-ticket.webp`,
      width: 1322,
      height: 544,
      alt: "A conference pass for HARBOR/26 in Lisbon with a barcode stub.",
    },
    {
      name: "Invoice",
      format: "PDF",
      file: `${OUTPUT}/home-usecase-invoice.pdf`,
      image: `${THUMBNAIL}/home-usecase-invoice.webp`,
      width: 385,
      height: 544,
      alt: "An A4 invoice totaling €12,472.20.",
    },
    {
      name: "Chat digest",
      format: "PNG",
      file: `${OUTPUT}/home-usecase-chat-digest.png`,
      image: `${THUMBNAIL}/home-usecase-chat-digest.webp`,
      width: 968,
      height: 544,
      alt: "A weekly help-desk digest with a bar chart of messages per day.",
    },
  ],
  [
    {
      name: "Now playing",
      format: "Animated WebP",
      file: `${OUTPUT}/home-usecase-now-playing.webp`,
      image: `${OUTPUT}/home-usecase-now-playing.webp`,
      width: 960,
      height: 360,
      alt: "A now-playing card with album art and a moving equalizer.",
      animated: true,
    },
    {
      name: "Repo banner",
      format: "SVG",
      file: `${OUTPUT}/home-usecase-repo-banner.svg`,
      image: `${OUTPUT}/home-usecase-repo-banner.svg`,
      width: 1280,
      height: 440,
      alt: "A repository banner for a job queue with status badges.",
    },
    {
      name: "Certificate",
      format: "PDF",
      file: `${OUTPUT}/home-usecase-certificate.pdf`,
      image: `${THUMBNAIL}/home-usecase-certificate.webp`,
      width: 770,
      height: 544,
      alt: "A course certificate with a gold frame and a seal.",
    },
    {
      name: "Quote card",
      format: "WebP",
      file: `${OUTPUT}/home-usecase-quote-card.webp`,
      image: `${THUMBNAIL}/home-usecase-quote-card.webp`,
      width: 544,
      height: 544,
      alt: "A proverb in vertical Chinese with English, Japanese, and Korean lines.",
    },
  ],
];

/** Real renders from the examples, two staggered rows that run past the right edge. */
export function UseCaseWall({ className }: { className?: string }) {
  return (
    <div className={cn("space-y-4", className)}>
      {rows.map((row, index) => (
        <ul
          key={row[0]?.name}
          aria-label={index === 0 ? "Examples" : undefined}
          className={cn("flex w-max gap-4", index === 1 && "ml-[7vw]")}
        >
          {row.map((useCase) => (
            <li key={useCase.name} className={cn(useCase.animated && "motion-reduce:hidden")}>
              <a
                href={useCase.file}
                target="_blank"
                rel="noopener"
                className="group relative block overflow-hidden rounded-md border outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
              >
                <img
                  src={useCase.image}
                  alt={useCase.alt}
                  width={useCase.width}
                  height={useCase.height}
                  decoding="async"
                  fetchPriority="low"
                  className="h-[clamp(6.5rem,19svh,12rem)] w-auto transition-transform duration-300 ease-out group-hover:scale-[1.02]"
                />
                <span className="absolute bottom-2 left-2 rounded-sm bg-background/90 px-2 py-0.5 text-base leading-snug whitespace-nowrap">
                  {useCase.name} <span className="text-muted-foreground">· {useCase.format}</span>
                </span>
              </a>
            </li>
          ))}
        </ul>
      ))}
    </div>
  );
}
