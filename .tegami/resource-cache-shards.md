---
packages:
  "takumi": minor
---

# Keep decoded photos in the resource cache

The default `cacheMaxBytes` is now 64 MiB, and one decoded image can use the whole budget. The cache used to split its budget into many small shards and never kept an image larger than one shard, so most photos were decoded again on every render.
