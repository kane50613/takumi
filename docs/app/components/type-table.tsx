import { cva } from "class-variance-authority";
import Link from "fumadocs-core/link";
import type { ReactNode } from "react";
import { cn } from "~/lib/utils";

interface ParameterNode {
  name: string;
  description: ReactNode;
}

export interface TypeNode {
  /**
   * Additional description of the field
   */
  description?: ReactNode;

  /**
   * type signature (short)
   */
  type: ReactNode;

  /**
   * type signature (full)
   */
  typeDescription?: ReactNode;

  /**
   * Optional `href` for the type
   */
  typeDescriptionLink?: string;

  default?: ReactNode;

  required?: boolean;
  deprecated?: boolean;

  parameters?: ParameterNode[];

  returns?: ReactNode;
}

const keyVariants = cva("text-fd-primary", {
  variants: {
    deprecated: {
      true: "line-through text-fd-primary/50",
    },
  },
});

export function TypeTable({ type }: { type: Record<string, TypeNode> }) {
  return (
    <div className="@container flex flex-col p-1 bg-fd-card text-fd-card-foreground rounded-2xl border my-6 text-sm overflow-hidden">
      <div className="flex font-medium items-center px-3 py-1 not-prose text-fd-muted-foreground">
        <p className="w-[25%]">Prop</p>
        <p className="w-[25%]">Type</p>
        <p className="@max-xl:hidden w-[50%]">Description</p>
      </div>
      {Object.entries(type).map(([key, value]) => (
        <Item key={key} name={key} item={value} />
      ))}
    </div>
  );
}

function Item({
  name,
  item: { required = false, deprecated, type, typeDescriptionLink, description },
}: {
  name: string;
  item: TypeNode;
}) {
  return (
    <div className="overflow-hidden rounded-xl relative flex flex-row items-center w-full group text-start px-3 py-2 not-prose hover:bg-fd-accent">
      <code
        className={cn(
          keyVariants({
            deprecated,
            className: "min-w-fit w-[25%] font-medium pe-2",
          }),
        )}
      >
        {name}
        {!required && "?"}
      </code>
      {typeDescriptionLink ? (
        <Link href={typeDescriptionLink} className="underline font-mono pe-2 w-[25%]">
          {type}
        </Link>
      ) : (
        <span className="font-mono pe-2 w-[25%]">{type}</span>
      )}
      <p className="@max-lg:hidden">{description}</p>
    </div>
  );
}
