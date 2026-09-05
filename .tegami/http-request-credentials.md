---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Keep credentials off HTTP requests

Remove Authorization and Cookie headers before HTTP requests, including same-origin redirects. Keep credentials on same-origin HTTPS requests.
