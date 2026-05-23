---
"takumi": minor
---

Align Tailwind utilities with v4 spec: V4 OKLCH-derived sRGB palette, composite `shadow-{sm|md|lg|xl}` and `text-shadow-{sm|md|lg}`, `from-N%/via-N%/to-N%` gradient stops, bare `rounded`, `font-{number}`, `line-clamp-none`, `bg-auto`, `bg-repeat-{round,space}`, `bg-conic`, `filter-none`/`backdrop-filter-none`, `grid-cols-none`, `col-auto`/`row-auto`, `shadow-{2xs,xs}`, `text-shadow-none`, `inset-shadow-none`. Fix `ms`/`me`/`ps`/`pe` (logical sides mapped to wrong axis), `-bg-color` silent positive, `-col-start-N` silent positive. Add `Length::from_spacing` constructor; demote `TW_VAR_SPACING` to `pub(crate)`.
