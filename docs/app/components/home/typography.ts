/** The homepage's text styles. Reading text stays at 18px or larger, and values at 16px. */
export const DISPLAY =
  "font-display font-[860] [font-stretch:112%] text-[clamp(2.5rem,4.6vw,5rem)] leading-[0.9] tracking-[-0.02em]";
/** A strike's one-word title, set larger and wider than the opening headline. */
export const STEP_WORD = "text-[clamp(3.5rem,12vw,11.5rem)] [font-stretch:125%] leading-[0.86]";
export const H2 =
  "font-display font-[840] [font-stretch:118%] text-[clamp(2rem,4vw,3.5rem)] leading-[0.98] tracking-[-0.02em] text-balance";
export const H3 = "font-display text-[1.625rem] font-[780] [font-stretch:112%] tracking-tight";
export const LEAD =
  "font-sans text-[clamp(1.125rem,1.45vw,1.375rem)] leading-[1.45] text-foreground/80 text-pretty";
export const VALUE =
  "font-label text-base [font-stretch:87.5%] leading-snug text-muted-foreground tabular-nums";
export const LINK =
  "font-sans font-semibold underline underline-offset-4 decoration-foreground/30 transition-[text-decoration-color] duration-150 ease-out-strong hover:decoration-foreground";
export const PRIMARY_BUTTON =
  "raised raised-primary h-12 rounded-xl bg-primary px-6 text-base font-semibold text-primary-foreground transition-[transform,box-shadow,background-color] duration-150 ease-out-strong hover:bg-[color-mix(in_oklab,var(--primary),white_8%)] active:scale-[0.97]";
/** The page's side margin, shared so every section lines up on one edge. */
export const GUTTER = "px-[max(5vw,1.25rem)]";
