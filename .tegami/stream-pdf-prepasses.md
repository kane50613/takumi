---
packages:
  takumi-pdf: patch
---

# Stream PDF documents with page counters, page-number bands or internal links

`render` now draws a page at a time for documents whose header, footer or content shows `pageNumber`, `totalPages` or `targetPageNumber`, or links to an element with `href="#id"`. Passes that only lay the pages out count them and find the targets first, so the output is unchanged and peak memory drops, at the cost of laying the content out once more. A counter that is still moving after three passes lays the document out to its end before the first page is drawn.
