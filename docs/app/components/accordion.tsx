import type { AccordionSingleProps } from "@radix-ui/react-accordion";
import * as AccordionPrimitive from "@radix-ui/react-accordion";
import { ChevronRight } from "lucide-react";
import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "~/lib/utils";

export const Accordions = ({
  type = "single",
  className,
  ...props
}: Omit<AccordionSingleProps, "value" | "onValueChange">) => (
  <AccordionPrimitive.Root
    type={type}
    className={cn(
      "divide-y divide-fd-border overflow-hidden rounded-lg border bg-fd-card",
      className,
    )}
    {...props}
  />
);

export const Accordion = ({
  title,
  className,
  id,
  value = String(title),
  children,
  ...props
}: Omit<ComponentPropsWithoutRef<typeof AccordionPrimitive.Item>, "value" | "title"> & {
  title: string | ReactNode;
  value?: string;
}) => {
  return (
    <AccordionPrimitive.Item value={value} className={cn("scroll-m-24", className)} {...props}>
      <AccordionPrimitive.Header
        id={id}
        data-accordion-value={value}
        className="not-prose flex flex-row items-center text-fd-card-foreground font-medium has-focus-visible:bg-fd-accent"
      >
        <AccordionPrimitive.Trigger className="group flex flex-1 items-center gap-2 px-3 py-2.5 text-start focus-visible:outline-none">
          <ChevronRight className="size-4 shrink-0 text-fd-muted-foreground transition-transform duration-200 group-data-[state=open]:rotate-90" />
          {title}
        </AccordionPrimitive.Trigger>
      </AccordionPrimitive.Header>
      <AccordionPrimitive.Content className="overflow-hidden data-[state=closed]:animate-fd-accordion-up data-[state=open]:animate-fd-accordion-down">
        <div className="px-4 pb-2 text-[15px] prose-no-margin">{children}</div>
      </AccordionPrimitive.Content>
    </AccordionPrimitive.Item>
  );
};
