---
packages:
  takumi-pdf:
    type: minor
---

### Fill a page counter in the content

A `pageNumber` or `totalPages` hook outside a band stayed empty, and nothing said why. It now takes the page its box lands on, and a hook laid out inline takes the page of the box that holds it. Numbering the content lays the document out a second time, since the page a hook sits on is only known once the content is cut into pages. A document without such a hook pays nothing.
