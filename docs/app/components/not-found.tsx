import { NOT_FOUND_LINKS } from "~/not-found-links";

export function NotFound() {
  return (
    <div className="flex flex-col px-8 justify-center flex-1 items-center gap-4 py-24 text-center">
      <h1 className="text-6xl font-bold text-fd-muted-foreground">404</h1>
      <h2 className="text-2xl font-semibold">Page not found</h2>
      <p className="text-fd-muted-foreground max-w-md">
        No page exists at this URL. It may have been renamed or removed. Try one of these instead.
      </p>
      <ul className="mt-4 flex flex-col gap-2 text-left text-sm">
        {NOT_FOUND_LINKS.map((link) => (
          <li key={link.href}>
            <a
              href={link.href}
              className="font-medium text-fd-primary underline underline-offset-4 decoration-fd-primary/40 hover:decoration-fd-primary"
            >
              {link.label}
            </a>
            <span className="text-fd-muted-foreground"> · {link.note}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
