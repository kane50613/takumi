---
packages:
  takumi-pdf:
    type: minor
---

### Print the page a link points at

A node classed `targetPageNumber` now renders the page number of the element the nearest enclosing `href` points at, which is what a table of contents needs. Counter styles apply the same way they do on `pageNumber`, and a fragment naming no element renders nothing.

Page numbers only exist once the document is paginated, so a document using the hook is paginated again with the numbers in place, up to three times, until they stop moving.
