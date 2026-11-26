---
packages:
  takumi-core: minor
---

# Support Tailwind's `@container`, `@sm:`, `rtl:`, `ltr:`, `dark:`, and `print:`

The `tw` prop now reads these Tailwind v4 utilities and variants, as Tailwind compiles them:

- `@container` sets `container-type`, and `@container/name` also sets `container-name`.
- `@sm:`, `@min-md:`, `@max-lg:`, and `@min-[400px]:` apply while the nearest inline-size container meets the width, and `@sm/name:` asks the container with that name.
- `rtl:` and `ltr:` follow the element's `dir`, including `dir="auto"`.
- `portrait:`, `landscape:`, `dark:`, and `print:` follow the render's viewport, color scheme, and output.

```tsx
<div tw="@container">
  <div tw="flex flex-col @md:flex-row rtl:text-right">…</div>
</div>
```

A named width such as `@md:` uses Tailwind's default `--container-md`, which a stylesheet's `--container-*` does not change.
