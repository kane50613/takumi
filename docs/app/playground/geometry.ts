const PX_PER_MM = 96 / 25.4;
const PAGE_SIZES = {
  a4: { width: 210 * PX_PER_MM, height: 297 * PX_PER_MM },
  letter: { width: 8.5 * 96, height: 11 * 96 },
};
const DEFAULT_PAGE_MARGIN = 48;
const DEFAULT_IMAGE_SIZE = { width: 1200, height: 630 };

type PdfOptions = NonNullable<PlaygroundOptions["pdf"]>;

/**
 * What the browser preview pane and the status bar need: the box to lay the
 * HTML out in, and a name for it. A paged PDF has no height, since the pane
 * shows one continuous flow rather than pages.
 */
export type OutputGeometry = {
  width: number;
  height?: number;
  /** CSS `padding` shorthand mirroring the PDF page margin. */
  padding?: string;
  label: string;
};

function marginPadding(margin: PdfOptions["margin"]): string {
  if (margin === undefined) return `${DEFAULT_PAGE_MARGIN}px`;
  if (typeof margin === "number") return `${margin}px`;

  const { top = 0, right = 0, bottom = 0, left = 0 } = margin;
  return `${top}px ${right}px ${bottom}px ${left}px`;
}

function pdfGeometry(pdf: PdfOptions): OutputGeometry {
  if (pdf.viewport) {
    const { width, height } = pdf.viewport;
    return { width, height, label: `${width} × ${height ?? "auto"}` };
  }

  const size = typeof pdf.size === "object" ? pdf.size : PAGE_SIZES[pdf.size ?? "a4"];
  const preset = typeof pdf.size === "object" ? undefined : (pdf.size ?? "a4");
  const width = Math.round(pdf.landscape ? size.height : size.width);
  const height = Math.round(pdf.landscape ? size.width : size.height);
  const name = preset ? preset.toUpperCase() : `${width} × ${height}`;

  return {
    width,
    label: pdf.landscape ? `${name} landscape` : name,
    padding: marginPadding(pdf.margin),
  };
}

export function outputGeometry(options: PlaygroundOptions): OutputGeometry {
  if (options.pdf) return pdfGeometry(options.pdf);

  const { width = DEFAULT_IMAGE_SIZE.width, height = DEFAULT_IMAGE_SIZE.height } = options;

  return { width, height, label: `${width} × ${height}` };
}
