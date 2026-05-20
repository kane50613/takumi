---
"takumi": minor
---

Accept `:is()` / `:where()` selectors, and keep rules containing unsupported pseudo-classes/elements (e.g. `:hover`, `::before`) instead of dropping them — they parse but never match, so sibling selectors in the same list (e.g. `a, a:hover`) continue to apply
