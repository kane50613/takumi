"use client";

import { useStaticSearch } from "fumadocs-core/search/client/orama-static";
import {
  SearchDialog,
  SearchDialogClose,
  SearchDialogContent,
  SearchDialogHeader,
  SearchDialogIcon,
  SearchDialogInput,
  SearchDialogList,
  SearchDialogOverlay,
} from "fumadocs-ui/components/dialog/search";
import type { StaticOptions } from "fumadocs-core/search/client/orama-static";
import type { SharedProps } from "fumadocs-ui/contexts/search";

export type SearchResultsProps = SharedProps & Pick<StaticOptions, "from" | "locale">;

// Mirrors the dialog of oramaSearchPlugin(), which the package does not export.
export default function SearchResults({ from, locale, ...props }: SearchResultsProps) {
  return (
    <SearchDialog {...props} {...useStaticSearch({ from, locale })}>
      <SearchDialogOverlay />
      <SearchDialogContent>
        <SearchDialogHeader>
          <SearchDialogIcon />
          <SearchDialogInput />
          <SearchDialogClose />
        </SearchDialogHeader>
        <SearchDialogList />
      </SearchDialogContent>
    </SearchDialog>
  );
}
